//! Current Website game-settings.ts browser document. The platform owner chooses
//! the storage key and performs I/O; this codec preserves Source's strict document
//! admission, historical migrations and normalized camelCase serialization.

use serde::Serialize;
use serde::ser::SerializeMap;
use serde_json::{Map, Value};
use solomon_sim::settings::{
    GameControlAction, GameControlBindings, GameSettings, valid_game_binding_code,
};

pub const STORAGE_KEY: &str = "solomon-dark-game-settings-v1";

const KEYS: [&str; 20] = [
    "cameraFovPercent",
    "castSecondariesAtMouse",
    "complexLighting",
    "complexShadows",
    "controls",
    "enableActivityMessages",
    "enableCheats",
    "enableGlobalChat",
    "enableOnlineFeatures",
    "enableSharedHub",
    "enhancedEffects",
    "lightQualityPercent",
    "musicVolumePercent",
    "multipleShadows",
    "reducedScreenFlashes",
    "reverseHotbarCycle",
    "soundVolumePercent",
    "submitRunsToServer",
    "uiScalePercent",
    "zoomEffects",
];
const ONLINE_KEYS: [&str; 5] = [
    "enableActivityMessages",
    "enableGlobalChat",
    "enableOnlineFeatures",
    "enableSharedHub",
    "submitRunsToServer",
];

/// Source rejects the whole malformed document rather than applying a partial
/// settings record. Missing storage is handled by the caller as the same default.
#[must_use]
pub fn parse(serialized: &str) -> GameSettings {
    serde_json::from_str(serialized).ok().and_then(parse_value).unwrap_or_default()
}

fn parse_value(value: Value) -> Option<GameSettings> {
    let mut source = value.as_object()?.clone();
    if source.len() == 1 && source.contains_key("enableCheats") {
        return Some(GameSettings {
            enable_cheats: source["enableCheats"] == Value::Bool(true),
            ..GameSettings::default()
        });
    }
    source.entry("enhancedEffects").or_insert(Value::Bool(true));
    source.entry("reverseHotbarCycle").or_insert(Value::Bool(false));
    let pre_online: Vec<_> = KEYS
        .into_iter()
        .filter(|key| *key != "reducedScreenFlashes" && !ONLINE_KEYS.contains(key))
        .collect();
    if same_keys(&source, &pre_online) {
        for key in ONLINE_KEYS {
            source.insert(key.into(), Value::Bool(true));
        }
    }
    let pre_flashes: Vec<_> =
        KEYS.into_iter().filter(|key| *key != "reducedScreenFlashes").collect();
    if same_keys(&source, &pre_flashes) {
        source.insert("reducedScreenFlashes".into(), Value::Bool(false));
    }
    if !same_keys(&source, &KEYS) {
        return None;
    }
    Some(GameSettings {
        camera_fov_percent: integer(&source, "cameraFovPercent", 75, 125)?,
        ui_scale_percent: integer(&source, "uiScalePercent", 75, 150)?,
        music_volume_percent: integer(&source, "musicVolumePercent", 0, 100)?,
        sound_volume_percent: integer(&source, "soundVolumePercent", 0, 100)?,
        light_quality_percent: integer(&source, "lightQualityPercent", 24, 100)?,
        cast_secondaries_at_mouse: boolean(&source, "castSecondariesAtMouse")?,
        complex_lighting: boolean(&source, "complexLighting")?,
        complex_shadows: boolean(&source, "complexShadows")?,
        multiple_shadows: boolean(&source, "multipleShadows")?,
        reduced_screen_flashes: boolean(&source, "reducedScreenFlashes")?,
        zoom_effects: boolean(&source, "zoomEffects")?,
        enhanced_effects: boolean(&source, "enhancedEffects")?,
        reverse_hotbar_cycle: boolean(&source, "reverseHotbarCycle")?,
        enable_activity_messages: boolean(&source, "enableActivityMessages")?,
        enable_cheats: boolean(&source, "enableCheats")?,
        enable_global_chat: boolean(&source, "enableGlobalChat")?,
        enable_online_features: boolean(&source, "enableOnlineFeatures")?,
        enable_shared_hub: boolean(&source, "enableSharedHub")?,
        submit_runs_to_server: boolean(&source, "submitRunsToServer")?,
        controls: parse_controls(source.get("controls")?)?,
    })
}

fn same_keys(source: &Map<String, Value>, expected: &[&str]) -> bool {
    source.len() == expected.len() && expected.iter().all(|key| source.contains_key(*key))
}

fn boolean(source: &Map<String, Value>, key: &str) -> Option<bool> {
    source.get(key)?.as_bool()
}

fn integer(source: &Map<String, Value>, key: &str, minimum: u16, maximum: u16) -> Option<u16> {
    let number = source.get(key)?.as_f64()?;
    if number.fract() != 0.0 || number < f64::from(minimum) || number > f64::from(maximum) {
        return None;
    }
    Some(number as u16)
}

fn parse_controls(value: &Value) -> Option<GameControlBindings> {
    let source = value.as_object()?;
    let missing_cheats = !source.contains_key("openCheats");
    let missing_hotbar = !source.contains_key("cycleHotbar");
    let expected =
        GameControlAction::ALL.len() - usize::from(missing_cheats) - usize::from(missing_hotbar);
    if source.len() != expected {
        return None;
    }
    for action in GameControlAction::ALL {
        let Some(value) = source.get(action.as_str()) else {
            if matches!(action, GameControlAction::OpenCheats | GameControlAction::CycleHotbar) {
                continue;
            }
            return None;
        };
        if !value.as_str().is_some_and(valid_game_binding_code) {
            return None;
        }
    }
    let code = |key: &str| source.get(key)?.as_str().map(str::to_owned);
    let mut controls = GameControlBindings {
        move_up: code("moveUp")?,
        move_down: code("moveDown")?,
        move_left: code("moveLeft")?,
        move_right: code("moveRight")?,
        open_menu: code("openMenu")?,
        open_inventory: code("openInventory")?,
        open_skills: code("openSkills")?,
        open_chat: code("openChat")?,
        open_cheats: code("openCheats").unwrap_or_default(),
        cycle_hotbar: code("cycleHotbar").unwrap_or_default(),
        belt: [
            code("belt1")?,
            code("belt2")?,
            code("belt3")?,
            code("belt4")?,
            code("belt5")?,
            code("belt6")?,
            code("belt7")?,
            code("belt8")?,
        ],
    };
    controls.migrate_missing_bindings().then_some(controls)
}

/// Source setGameSettings writes all current fields in this declaration order.
/// Numeric values are integers in the Rust model, so normalization only clamps;
/// captured binding strings still use the canonical UTF-16 admission check.
pub fn serialize(settings: &GameSettings) -> Result<String, String> {
    let normalized = NormalizedSettings {
        camera_fov_percent: settings.camera_fov_percent.clamp(75, 125),
        cast_secondaries_at_mouse: settings.cast_secondaries_at_mouse,
        complex_lighting: settings.complex_lighting,
        complex_shadows: settings.complex_shadows,
        controls: NormalizedControls(&settings.controls),
        enable_activity_messages: settings.enable_activity_messages,
        enable_cheats: settings.enable_cheats,
        enable_global_chat: settings.enable_global_chat,
        enable_online_features: settings.enable_online_features,
        enable_shared_hub: settings.enable_shared_hub,
        enhanced_effects: settings.enhanced_effects,
        light_quality_percent: settings.light_quality_percent.clamp(24, 100),
        music_volume_percent: settings.music_volume_percent.min(100),
        multiple_shadows: settings.multiple_shadows,
        reduced_screen_flashes: settings.reduced_screen_flashes,
        reverse_hotbar_cycle: settings.reverse_hotbar_cycle,
        sound_volume_percent: settings.sound_volume_percent.min(100),
        submit_runs_to_server: settings.submit_runs_to_server,
        ui_scale_percent: settings.ui_scale_percent.clamp(75, 150),
        zoom_effects: settings.zoom_effects,
    };
    serde_json::to_string(&normalized).map_err(|error| error.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NormalizedSettings<'a> {
    camera_fov_percent: u16,
    cast_secondaries_at_mouse: bool,
    complex_lighting: bool,
    complex_shadows: bool,
    controls: NormalizedControls<'a>,
    enable_activity_messages: bool,
    enable_cheats: bool,
    enable_global_chat: bool,
    enable_online_features: bool,
    enable_shared_hub: bool,
    enhanced_effects: bool,
    light_quality_percent: u16,
    music_volume_percent: u16,
    multiple_shadows: bool,
    reduced_screen_flashes: bool,
    reverse_hotbar_cycle: bool,
    sound_volume_percent: u16,
    submit_runs_to_server: bool,
    ui_scale_percent: u16,
    zoom_effects: bool,
}

struct NormalizedControls<'a>(&'a GameControlBindings);

impl Serialize for NormalizedControls<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let defaults = GameControlBindings::default();
        let mut map = serializer.serialize_map(Some(GameControlAction::ALL.len()))?;
        for action in GameControlAction::ALL {
            let code = self.0.code(action);
            // Source normalizes each captured string here without swapping or
            // deduplicating it. The stricter read path owns document admission.
            let normalized =
                if valid_game_binding_code(code) { code } else { defaults.code(action) };
            map.serialize_entry(action.as_str(), normalized)?;
        }
        map.end()
    }
}
