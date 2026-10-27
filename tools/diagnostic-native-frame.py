"""Private observation-only trace for the intermittent Linux blank game frame."""
from pathlib import Path

p = Path('crates/web/src/game.rs')
s = p.read_text()
needle = '    fn capture_ui_snapshot(&mut self) {\n'
assert s.count(needle) == 1
s = s.replace(needle, needle + '''        thread_local! {
            static FRAME_TRACE: std::cell::RefCell<(String, f64)> =
                const { std::cell::RefCell::new((String::new(), -10000.0)) };
        }
        let summary = format!("{} {} {} {:?}", self.has_game, self.menu.surface_name(),
            self.menu.covers_world(), self.gpu_ui.diagnostics().widgets.iter()
                .map(|widget| widget.id.as_str()).collect::<Vec<_>>());
        let now = crate::platform::monotonic_ms();
        FRAME_TRACE.with(|trace| {
            let mut trace = trace.borrow_mut();
            if trace.0 != summary || now - trace.1 >= 1000.0 {
                eprintln!("NATIVE_FRAME_TRACE {}", serde_json::json!({
                    "time": now, "hasGame": self.has_game,
                    "surface": self.menu.surface_name(), "coversWorld": self.menu.covers_world(),
                    "blocking": self.menu.is_blocking(), "ready": self.ui_ready,
                    "playing": self.native.as_ref().is_some_and(NativeGame::is_playing),
                    "inspecting": self.native.as_ref().is_some_and(NativeGame::is_inspecting),
                    "leaving": self.native.as_ref().is_some_and(NativeGame::is_leaving),
                    "worldLoading": self.world_loading, "ticks": self.session.ticks,
                    "width": self.surface.logical_width(), "height": self.surface.logical_height(),
                    "diagnostics": self.gpu_ui.diagnostics(), "runtimeError": crate::runtime_ui::error(),
                }));
                *trace = (summary, now);
            }
        });
''')
p.write_text(s)
p = Path('crates/desktop/src/app.rs')
s = p.read_text()
needle = '        if matches!(event, WindowEvent::CloseRequested) {\n'
assert s.count(needle) == 1
s = s.replace(needle, '''        if matches!(event, WindowEvent::Focused(_) | WindowEvent::Occluded(_) | WindowEvent::Resized(_)) {
            eprintln!("NATIVE_WINDOW_TRACE {} {:?}", solomon_web::platform::monotonic_ms(), event);
        }
''' + needle)
p.write_text(s)
p = Path('tools/smoke-desktop-runtime.py')
s = p.read_text()
assert s.count('    check_offline()\n') == 1
s = s.replace('    check_offline()\n', '')
p.write_text(s)
