//! Game preferences from Website `game-settings.ts`. Storage belongs to the host.

#[must_use]
pub fn camera_zoom_for_fov(native_zoom: f64, fov_percent: u16) -> f64 {
    native_zoom / (f64::from(fov_percent.clamp(75, 125)) / 100.0)
}

#[must_use]
pub fn game_light_quality(settings: &GameSettings) -> f64 {
    crate::float::fround(0.25 * f64::from(settings.light_quality_percent.clamp(24, 100)) / 100.0)
}

/// Human-readable control names shared by settings and Tutorial instructions.
#[must_use]
pub fn game_binding_label(code: &str) -> String {
    match code {
        "Mouse0" => return "Left Mouse".to_owned(),
        "Mouse1" => return "Middle Mouse".to_owned(),
        "Mouse2" => return "Right Mouse".to_owned(),
        _ => {}
    }
    if let Some(letter) = code.strip_prefix("Key")
        && letter.len() == 1
    {
        return letter.to_owned();
    }
    if let Some(digit) = code.strip_prefix("Digit")
        && digit.len() == 1
    {
        return digit.to_owned();
    }
    if let Some(key) = code.strip_prefix("Numpad") {
        return format!("Numpad {key}");
    }
    if let Some(direction) = code.strip_prefix("Arrow") {
        return direction.to_owned();
    }
    let mut label = String::new();
    let mut previous = None;
    for character in code.chars() {
        if character.is_ascii_uppercase()
            && previous.is_some_and(|value: char| value.is_ascii_lowercase())
        {
            label.push(' ');
        }
        label.push(character);
        previous = Some(character);
    }
    label
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameControlAction {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    OpenMenu,
    OpenInventory,
    OpenSkills,
    OpenChat,
    OpenCheats,
    Belt(usize),
    CycleHotbar,
}

impl GameControlAction {
    pub const ALL: [Self; 18] = [
        Self::MoveUp,
        Self::MoveDown,
        Self::MoveLeft,
        Self::MoveRight,
        Self::OpenMenu,
        Self::OpenInventory,
        Self::OpenSkills,
        Self::OpenChat,
        Self::OpenCheats,
        Self::Belt(0),
        Self::Belt(1),
        Self::Belt(2),
        Self::Belt(3),
        Self::Belt(4),
        Self::Belt(5),
        Self::Belt(6),
        Self::Belt(7),
        Self::CycleHotbar,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MoveUp => "moveUp",
            Self::MoveDown => "moveDown",
            Self::MoveLeft => "moveLeft",
            Self::MoveRight => "moveRight",
            Self::OpenMenu => "openMenu",
            Self::OpenInventory => "openInventory",
            Self::OpenSkills => "openSkills",
            Self::OpenChat => "openChat",
            Self::OpenCheats => "openCheats",
            Self::CycleHotbar => "cycleHotbar",
            Self::Belt(slot) => {
                ["belt1", "belt2", "belt3", "belt4", "belt5", "belt6", "belt7", "belt8"][slot]
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct GameControlBindings {
    pub move_up: String,
    pub move_down: String,
    pub move_left: String,
    pub move_right: String,
    pub open_menu: String,
    pub open_inventory: String,
    pub open_skills: String,
    #[cfg_attr(feature = "persistence", serde(default = "default_open_chat_binding"))]
    pub open_chat: String,
    // Empty marks an older stored controls object; the browser edge runs the
    // source migration below before applying it, so occupied keys are preserved.
    #[cfg_attr(
        feature = "persistence",
        serde(default, deserialize_with = "deserialize_migrated_binding")
    )]
    pub open_cheats: String,
    #[cfg_attr(
        feature = "persistence",
        serde(default, deserialize_with = "deserialize_migrated_binding")
    )]
    pub cycle_hotbar: String,
    pub belt: [String; 8],
}

fn default_open_chat_binding() -> String {
    "KeyT".to_owned()
}

#[cfg(feature = "persistence")]
fn deserialize_migrated_binding<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let code = <String as serde::Deserialize>::deserialize(deserializer)?;
    if !valid_game_binding_code(&code) {
        return Err(serde::de::Error::custom("invalid saved game binding"));
    }
    Ok(code)
}

impl Default for GameControlBindings {
    fn default() -> Self {
        Self {
            move_up: "KeyW".to_string(),
            move_down: "KeyS".to_string(),
            move_left: "KeyA".to_string(),
            move_right: "KeyD".to_string(),
            open_menu: "Escape".to_string(),
            open_inventory: "KeyI".to_string(),
            open_skills: "KeyK".to_string(),
            open_chat: default_open_chat_binding(),
            open_cheats: "Backquote".to_owned(),
            cycle_hotbar: "KeyR".to_owned(),
            belt: ["Mouse2", "Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7"]
                .map(str::to_string),
        }
    }
}

impl GameControlBindings {
    #[must_use]
    pub fn code(&self, action: GameControlAction) -> &str {
        match action {
            GameControlAction::MoveUp => &self.move_up,
            GameControlAction::MoveDown => &self.move_down,
            GameControlAction::MoveLeft => &self.move_left,
            GameControlAction::MoveRight => &self.move_right,
            GameControlAction::OpenMenu => &self.open_menu,
            GameControlAction::OpenInventory => &self.open_inventory,
            GameControlAction::OpenSkills => &self.open_skills,
            GameControlAction::OpenChat => &self.open_chat,
            GameControlAction::OpenCheats => &self.open_cheats,
            GameControlAction::CycleHotbar => &self.cycle_hotbar,
            GameControlAction::Belt(slot) => &self.belt[slot],
        }
    }

    /// `rebindGameControl` swaps an occupied key into this action's old binding.
    pub fn rebind(&mut self, action: GameControlAction, code: &str) -> bool {
        if !valid_game_binding_code(code) || self.code(action) == code {
            return false;
        }
        let old = self.code(action).to_string();
        let displaced =
            GameControlAction::ALL.into_iter().find(|candidate| self.code(*candidate) == code);
        *self.code_mut(action) = code.to_string();
        if let Some(displaced) = displaced {
            *self.code_mut(displaced) = old;
        }
        true
    }

    /// Current `migrateGameControls`: preserve all occupied keys, then add missing
    /// console/hotbar bindings in source order. Other invalid controls reject the document.
    pub fn migrate_missing_bindings(&mut self) -> bool {
        let mut used = std::collections::BTreeSet::new();
        for action in GameControlAction::ALL {
            let code = self.code(action);
            if code.is_empty()
                && matches!(action, GameControlAction::OpenCheats | GameControlAction::CycleHotbar)
            {
                continue;
            }
            if !valid_game_binding_code(code) || !used.insert(code.to_owned()) {
                return false;
            }
        }
        for action in [GameControlAction::OpenCheats, GameControlAction::CycleHotbar] {
            if !self.code(action).is_empty() {
                continue;
            }
            let mut candidates = if action == GameControlAction::OpenCheats {
                vec!["Backquote".to_owned()]
            } else {
                vec!["KeyR".to_owned(), "KeyV".to_owned()]
            };
            for key in 1..=24 {
                candidates.push(format!("F{key}"));
            }
            let Some(code) = candidates.into_iter().find(|code| !used.contains(code)) else {
                return false;
            };
            used.insert(code.clone());
            *self.code_mut(action) = code;
        }
        true
    }

    fn code_mut(&mut self, action: GameControlAction) -> &mut String {
        match action {
            GameControlAction::MoveUp => &mut self.move_up,
            GameControlAction::MoveDown => &mut self.move_down,
            GameControlAction::MoveLeft => &mut self.move_left,
            GameControlAction::MoveRight => &mut self.move_right,
            GameControlAction::OpenMenu => &mut self.open_menu,
            GameControlAction::OpenInventory => &mut self.open_inventory,
            GameControlAction::OpenSkills => &mut self.open_skills,
            GameControlAction::OpenChat => &mut self.open_chat,
            GameControlAction::OpenCheats => &mut self.open_cheats,
            GameControlAction::CycleHotbar => &mut self.cycle_hotbar,
            GameControlAction::Belt(slot) => &mut self.belt[slot],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub struct GameSettings {
    pub camera_fov_percent: u16,
    pub ui_scale_percent: u16,
    pub music_volume_percent: u16,
    pub sound_volume_percent: u16,
    pub light_quality_percent: u16,
    pub cast_secondaries_at_mouse: bool,
    pub complex_lighting: bool,
    pub complex_shadows: bool,
    pub multiple_shadows: bool,
    pub reduced_screen_flashes: bool,
    pub zoom_effects: bool,
    #[cfg_attr(feature = "persistence", serde(default = "default_enabled"))]
    pub enhanced_effects: bool,
    #[cfg_attr(feature = "persistence", serde(default))]
    pub reverse_hotbar_cycle: bool,
    #[cfg_attr(feature = "persistence", serde(default = "default_enabled"))]
    pub enable_activity_messages: bool,
    #[cfg_attr(feature = "persistence", serde(default))]
    pub enable_cheats: bool,
    #[cfg_attr(feature = "persistence", serde(default = "default_enabled"))]
    pub enable_global_chat: bool,
    #[cfg_attr(feature = "persistence", serde(default = "default_enabled"))]
    pub enable_online_features: bool,
    #[cfg_attr(feature = "persistence", serde(default = "default_enabled"))]
    pub enable_shared_hub: bool,
    #[cfg_attr(feature = "persistence", serde(default = "default_enabled"))]
    pub submit_runs_to_server: bool,
    pub controls: GameControlBindings,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            camera_fov_percent: 100,
            ui_scale_percent: 100,
            music_volume_percent: 100,
            sound_volume_percent: 100,
            light_quality_percent: 100,
            cast_secondaries_at_mouse: true,
            complex_lighting: true,
            complex_shadows: true,
            multiple_shadows: true,
            reduced_screen_flashes: false,
            zoom_effects: true,
            enhanced_effects: true,
            reverse_hotbar_cycle: false,
            enable_activity_messages: true,
            enable_cheats: false,
            enable_global_chat: true,
            enable_online_features: true,
            enable_shared_hub: true,
            submit_runs_to_server: true,
            controls: GameControlBindings::default(),
        }
    }
}

#[cfg(feature = "persistence")]
fn default_enabled() -> bool {
    true
}

/// ECMAScript counts UTF-16 code units when checking a captured binding's length.
#[must_use]
pub fn valid_game_binding_code(code: &str) -> bool {
    let length = code.encode_utf16().count();
    length > 0 && length <= 64
}

pub use crate::social::GameOnlinePreferences;

#[must_use]
pub fn game_online_preferences(settings: &GameSettings) -> GameOnlinePreferences {
    let global_chat = settings.enable_online_features && settings.enable_global_chat;
    GameOnlinePreferences {
        activity_messages: global_chat && settings.enable_activity_messages,
        global_chat,
        submit_runs: settings.enable_online_features && settings.submit_runs_to_server,
    }
}

#[must_use]
pub fn game_shared_hub_enabled(settings: &GameSettings) -> bool {
    settings.enable_online_features && settings.enable_shared_hub
}
