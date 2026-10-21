//! One gamepad read while a menu owns input. Website `input/gamepad-menu-navigation.ts`
//! defines edge buttons, direction repeat, spatial focus and neutral handoffs.

use solomon_sim::Vec2;
use solomon_sim::input::{self, MenuDirection, MenuGamepadButtons, MenuGamepadState};
use wasm_bindgen::JsCast;
use web_sys::{
    Document, Gamepad, GamepadButton, GamepadMappingType, HtmlElement, HtmlInputElement,
};

pub struct MenuNavigation {
    previous: MenuGamepadState,
    next_repeat_at: f64,
    scope: String,
    require_neutral: bool,
}

impl MenuNavigation {
    pub fn new() -> Self {
        Self {
            previous: MenuGamepadState::default(),
            next_repeat_at: 0.0,
            scope: String::new(),
            require_neutral: false,
        }
    }

    pub fn release(&mut self) {
        self.previous = MenuGamepadState::default();
        self.next_repeat_at = 0.0;
        self.scope.clear();
        self.require_neutral = true;
    }

    /// Returns a back edge; confirmation uses the focused button's ordinary click event.
    pub fn update(
        &mut self,
        root: &HtmlElement,
        document: &Document,
        scope: &str,
        now: f64,
    ) -> Result<bool, String> {
        let current = read_browser_gamepad()?;
        if scope != self.scope {
            self.scope = scope.to_string();
            self.next_repeat_at = 0.0;
            self.require_neutral |= self.previous.active() && current.active();
        }
        if self.require_neutral {
            if !current.active() {
                self.require_neutral = false;
                self.previous = current;
            }
            return Ok(false);
        }
        let targets = focusable(root)?;
        if targets.is_empty() {
            self.previous = current;
            return Ok(false);
        }
        let active = document.active_element();
        let active_index = targets.iter().position(|target| {
            active.as_ref().is_some_and(|active| active.is_same_node(Some(target)))
        });
        let initial =
            if active_index.is_none() { preferred_target(root, &targets)? } else { active_index };
        if current.confirm
            && !self.previous.confirm
            && let Some(index) = initial
        {
            targets[index].focus().map_err(|_| "initial menu focus")?;
            targets[index].click();
        }
        if current.next && !self.previous.next {
            if active_index.is_some() {
                linear_focus(&targets, active_index, 1)?;
            } else if let Some(index) = initial {
                targets[index].focus().map_err(|_| "initial menu focus")?;
            }
        }
        if current.previous && !self.previous.previous {
            if active_index.is_some() {
                linear_focus(&targets, active_index, -1)?;
            } else if let Some(index) = initial {
                targets[index].focus().map_err(|_| "initial menu focus")?;
            }
        }
        let (move_focus, next_repeat_at) = input::advance_menu_direction_repeat(
            self.previous.direction,
            current.direction,
            now,
            self.next_repeat_at,
        );
        self.next_repeat_at = next_repeat_at;
        if move_focus && let Some(direction) = current.direction {
            if active_index.is_some() {
                directional_focus(&targets, active_index, direction)?;
            } else if let Some(index) = initial {
                targets[index].focus().map_err(|_| "initial menu focus")?;
            }
        }
        let back = current.back && !self.previous.back;
        self.previous = current;
        Ok(back)
    }
}

fn preferred_target(root: &HtmlElement, targets: &[HtmlElement]) -> Result<Option<usize>, String> {
    let preferred = root
        .query_selector_all("[data-game-default-focus='true']")
        .map_err(|_| "preferred menu focus selector")?;
    for index in 0..preferred.length() {
        let Some(candidate) = preferred.item(index) else {
            continue;
        };
        if let Some(index) = targets.iter().position(|target| target.is_same_node(Some(&candidate)))
        {
            return Ok(Some(index));
        }
    }
    // Hidden preferred controls own the first input once their native reveal completes.
    if preferred.length() > 0 {
        return Ok(None);
    }
    Ok((!targets.is_empty()).then_some(0))
}

fn read_browser_gamepad() -> Result<MenuGamepadState, String> {
    let window = web_sys::window().ok_or("no window for gamepad navigation")?;
    let navigator = window.navigator();
    let function = js_sys::Reflect::get(navigator.as_ref(), &"getGamepads".into())
        .map_err(|_| "cannot inspect menu gamepads")?;
    if !function.is_function() {
        return Ok(MenuGamepadState::default());
    }
    for value in navigator.get_gamepads().map_err(|_| "browser denied menu gamepad access")? {
        if value.is_null() || value.is_undefined() {
            continue;
        }
        let gamepad: Gamepad = value.dyn_into().map_err(|_| "invalid menu gamepad")?;
        if !gamepad.connected() || gamepad.mapping() != GamepadMappingType::Standard {
            continue;
        }
        let axes = gamepad.axes();
        let buttons = gamepad.buttons();
        let pressed = |index| {
            buttons
                .get(index)
                .dyn_into::<GamepadButton>()
                .is_ok_and(|button| button.pressed() || button.value() > 0.5)
        };
        let state = input::read_menu_gamepad(
            Vec2::new(axes.get(0).as_f64().unwrap_or(0.0), axes.get(1).as_f64().unwrap_or(0.0)),
            MenuGamepadButtons {
                back: pressed(1),
                confirm: pressed(0),
                next: pressed(5),
                previous: pressed(4),
                up: pressed(12),
                down: pressed(13),
                left: pressed(14),
                right: pressed(15),
            },
        );
        if state.active() {
            return Ok(state);
        }
    }
    Ok(MenuGamepadState::default())
}

fn focusable(root: &HtmlElement) -> Result<Vec<HtmlElement>, String> {
    let nodes = root
        .query_selector_all("button:not([disabled]), input:not([disabled]), select:not([disabled])")
        .map_err(|_| "menu focus selector")?;
    let mut targets = Vec::new();
    for index in 0..nodes.length() {
        let Some(node) = nodes.item(index) else {
            continue;
        };
        let element: HtmlElement = node.dyn_into().map_err(|_| "menu focus target is not HTML")?;
        if element.offset_width() > 0 && element.offset_height() > 0 {
            targets.push(element);
        }
    }
    Ok(targets)
}

fn linear_focus(
    targets: &[HtmlElement],
    active: Option<usize>,
    delta: isize,
) -> Result<(), String> {
    let index = if let Some(active) = active {
        (active as isize + delta).rem_euclid(targets.len() as isize) as usize
    } else if delta < 0 {
        targets.len() - 1
    } else {
        0
    };
    targets[index].focus().map_err(|_| "menu focus".to_string())
}

fn directional_focus(
    targets: &[HtmlElement],
    active: Option<usize>,
    direction: MenuDirection,
) -> Result<(), String> {
    let Some(active) = active else {
        return linear_focus(targets, None, 1);
    };
    if let Some(input) = targets[active].dyn_ref::<HtmlInputElement>()
        && input.type_() == "range"
        && matches!(direction, MenuDirection::Left | MenuDirection::Right)
    {
        let delta = if direction == MenuDirection::Left { -1.0 } else { 1.0 };
        let minimum = input.min().parse::<f64>().map_err(|_| "range minimum")?;
        let maximum = input.max().parse::<f64>().map_err(|_| "range maximum")?;
        input.set_value_as_number((input.value_as_number() + delta).clamp(minimum, maximum));
        let event = web_sys::Event::new("change").map_err(|_| "menu range change event")?;
        // This event is sent directly to the root because `Event::new` does not bubble.
        if let Some(root) =
            targets[active].closest(".game-menus").map_err(|_| "range menu owner")?
        {
            root.dispatch_event(&event).map_err(|_| "menu range change")?;
        }
        return Ok(());
    }
    let centers: Vec<_> = targets
        .iter()
        .map(|target| {
            let bounds = target.get_bounding_client_rect();
            Vec2::new(bounds.left() + bounds.width() / 2.0, bounds.top() + bounds.height() / 2.0)
        })
        .collect();
    if let Some(target) = input::choose_spatial_menu_target(active, &centers, direction) {
        return targets[target].focus().map_err(|_| "spatial menu focus".to_string());
    }
    linear_focus(
        targets,
        Some(active),
        if matches!(direction, MenuDirection::Left | MenuDirection::Up) { -1 } else { 1 },
    )
}
