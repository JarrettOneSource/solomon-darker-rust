//! Source insertion-order callback/timer snapshots and per-tick work admission.

use super::*;
use crate::definition::values::{LuaTableEntries, lua_payload, lua_table_entries};
use values::{nonnegative_integer, require_text};

impl WebLuaRuntime {
    pub(super) fn run_due_timers(&mut self, lua: &Lua) -> mlua::Result<()> {
        // Source snapshots the map's references. A timer cancelled by an earlier
        // callback still runs once if it was already in this tick's snapshot.
        let timers = self.state.borrow().timers.clone();
        for timer in timers {
            if timer.borrow().due_tick > self.state.borrow().current_tick {
                continue;
            }
            if !self.admit_callback() {
                return Ok(());
            }
            let (callback, handle) = {
                let timer = timer.borrow();
                (timer.callback.clone(), timer.handle)
            };
            let active = authority(lua)?;
            if !self.call_callback(lua, &callback, &format!("timer {handle}"), active, None) {
                self.state.borrow_mut().timers.retain(|entry| entry.borrow().handle != handle);
                continue;
            }
            let tick = self.state.borrow().current_tick;
            let mut timer = timer.borrow_mut();
            if let Some(interval) = timer.interval_ticks {
                timer.due_tick = tick + interval;
            } else if let Some(step) = timer.sequence_steps.get(timer.sequence_index).cloned() {
                timer.sequence_index += 1;
                timer.callback = step.callback;
                timer.due_tick = tick + step.delay_ticks;
            } else {
                drop(timer);
                self.state.borrow_mut().timers.retain(|entry| entry.borrow().handle != handle);
            }
        }
        Ok(())
    }

    pub(super) fn dispatch_callbacks(
        &mut self,
        lua: &Lua,
        name: &str,
        payload: &LuaValue,
        player_id: Option<&str>,
    ) -> mlua::Result<()> {
        let callbacks: Vec<_> = self
            .state
            .borrow()
            .callbacks
            .iter()
            .filter(|entry| entry.name == name)
            .cloned()
            .collect();
        for callback in callbacks {
            if !self.admit_callback() {
                return Ok(());
            }
            let active = match player_id {
                Some(id) => Some(id.into()),
                None => authority(lua)?,
            };
            let owner = format!("{name} callback {}", callback.handle);
            if !self.call_callback(lua, &callback.callback, &owner, active, Some(payload)) {
                self.state.borrow_mut().callbacks.retain(|entry| entry.handle != callback.handle);
            }
        }
        Ok(())
    }

    fn admit_callback(&self) -> bool {
        let mut state = self.state.borrow_mut();
        if state.tick_work_ms > 4.0 {
            state.note_budget();
            return false;
        }
        if state.callback_invocations < 64 {
            return true;
        }
        if state.callback_warning_tick != Some(state.current_tick) {
            state.callback_warning_tick = Some(state.current_tick);
            let tick = state.current_tick;
            state.log(
                "warning",
                "lua.tick_callback_limit_reached",
                format!("Lua reached its callback limit at tick {tick}."),
            );
        }
        false
    }

    fn call_callback(
        &mut self,
        lua: &Lua,
        callback: &Function,
        owner: &str,
        player_id: Option<String>,
        payload: Option<&LuaValue>,
    ) -> bool {
        {
            let mut state = self.state.borrow_mut();
            state.callback_invocations += 1;
            state.active_player_id = player_id;
        }
        let started = self.now();
        self.deadline.set(Some(Instant::now() + Duration::from_millis(2)));
        let invoked = match payload {
            Some(payload) => lua_payload(lua, payload).and_then(|value| callback.call::<()>(value)),
            None => callback.call::<()>(()),
        };
        self.deadline.set(None);
        self.record_work(started);
        let mut state = self.state.borrow_mut();
        state.active_player_id = None;
        if let Err(error) = invoked {
            state.log(
                "warning",
                "lua.callback_failed",
                format!("{owner}: {}", bounded_error(&error)),
            );
            return false;
        }
        true
    }
}

impl RuntimeState {
    pub(super) fn add_callback(&mut self, name: &Value, callback: &Value) -> Result<(), String> {
        let name = require_text(name, "event name", 64)?;
        if !solomon_sim::modding::console::WEB_LUA_EVENT_NAMES.contains(&name.as_str()) {
            return Err(format!("unsupported event: {name}"));
        }
        let Value::Function(callback) = callback else {
            return Err("event callback must be a function".into());
        };
        if self.callbacks.len() >= 128 {
            return Err("Lua callback limit reached".into());
        }
        let handle = self.next_handle;
        self.next_handle += 1;
        self.callbacks.push(RegisteredCallback { name, handle, callback: callback.clone() });
        Ok(())
    }
    fn timer_callback_count(&self) -> usize {
        let mut count = 0;
        for timer in &self.timers {
            let timer = timer.borrow();
            count += 1 + timer.sequence_steps.len() - timer.sequence_index;
        }
        count
    }
    pub(super) fn add_timer(
        &mut self,
        delay: &Value,
        callback: &Value,
        repeating: bool,
    ) -> Result<u64, String> {
        if self.timer_callback_count() >= 256 {
            return Err("Lua timer limit reached".into());
        }
        let Value::Function(callback) = callback else {
            return Err("timer callback must be a function".into());
        };
        let milliseconds =
            nonnegative_integer(delay, if repeating { "timer interval" } else { "timer delay" })?;
        if milliseconds > 86_400_000 {
            return Err("Lua timer delay exceeds 24 hours".into());
        }
        if repeating && milliseconds < 1 {
            return Err("repeating Lua timer interval is below 1 ms".into());
        }
        let ticks = milliseconds.div_ceil(10).max(1);
        let handle = self.next_handle;
        self.next_handle += 1;
        self.timers.push(Rc::new(RefCell::new(LuaTimer {
            callback: callback.clone(),
            due_tick: self.current_tick + ticks,
            handle,
            interval_ticks: repeating.then_some(ticks),
            sequence_steps: Vec::new(),
            sequence_index: 0,
        })));
        Ok(handle)
    }
    pub(super) fn add_sequence(&mut self, value: &Value) -> Result<u64, String> {
        let entries = match value {
            Value::Table(table) => match lua_table_entries(table)? {
                LuaTableEntries::Array(entries) if entries.len() <= 64 => entries,
                _ => return Err("timer sequence requires between 1 and 64 steps".into()),
            },
            _ => return Err("timer sequence requires between 1 and 64 steps".into()),
        };
        let mut steps = Vec::new();
        let mut cumulative = 0;
        for (index, value) in entries.iter().enumerate() {
            let field = format!("timer sequence step {}", index + 1);
            let Value::Table(table) = value else {
                return Err(format!("{field} must be a table"));
            };
            if !matches!(lua_table_entries(table)?, LuaTableEntries::Object(_)) {
                return Err(format!("{field} must be a table"));
            }
            let callback = table.get::<Value>("callback").map_err(|error| error.to_string())?;
            let Value::Function(callback) = callback else {
                return Err(format!("{field} callback must be a function"));
            };
            let delay = table.get::<Value>("delay_ms").map_err(|error| error.to_string())?;
            let milliseconds = nonnegative_integer(&delay, &format!("{field} delay_ms"))?;
            cumulative += milliseconds;
            if cumulative > 86_400_000 {
                return Err("timer sequence cumulative delay exceeds 24 hours".into());
            }
            steps.push(TimerStep { callback, delay_ticks: milliseconds.div_ceil(10).max(1) });
        }
        if self.timer_callback_count() + steps.len() > 256 {
            return Err("Lua timer limit reached".into());
        }
        let first = steps.remove(0);
        let handle = self.next_handle;
        self.next_handle += 1;
        self.timers.push(Rc::new(RefCell::new(LuaTimer {
            callback: first.callback,
            due_tick: self.current_tick + first.delay_ticks,
            handle,
            interval_ticks: None,
            sequence_steps: steps,
            sequence_index: 0,
        })));
        Ok(handle)
    }
}
