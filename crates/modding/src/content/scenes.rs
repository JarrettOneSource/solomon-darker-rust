//! Scenes, shops, Boasts and UI definitions from `mod-content-catalog.ts`.

use std::collections::BTreeSet;

use solomon_sim::hub::interaction::BoastFailureProducer;
use solomon_sim::modding::LuaValue;
use solomon_sim::modding::content::*;
use solomon_sim::modding::definition::{WebLuaAssetKind, WebLuaContentKind, WebLuaRuleDefinition};

use crate::assets::PreparedModAssetCatalog;

use super::empty_array;
use super::fields::*;

pub(super) fn boneyard(
    common: PreparedModContentEntry,
) -> Result<PreparedModBoneyardDefinition, String> {
    let source = common
        .fields
        .get("source")
        .and_then(LuaValue::as_str)
        .filter(|source| {
            source.starts_with("levels/")
                && source.ends_with(".boneyard")
                && source.len() > "levels/.boneyard".len()
                && !source.contains(['\n', '\r', '\u{2028}', '\u{2029}'])
        })
        .ok_or_else(|| format!("{}:{} Boneyard source is invalid", common.mod_id, common.key))?;
    if common.art.get("layout").is_none_or(|binding| {
        binding.asset_kind != WebLuaAssetKind::Boneyard || binding.path != source
    }) {
        return Err(format!(
            "{}:{} Boneyard art.layout must declare its source file",
            common.mod_id, common.key
        ));
    }
    let source = source.to_owned();
    let empty = empty_array();
    let roster = common
        .fields
        .get("roster")
        .unwrap_or(&empty)
        .as_array()
        .filter(|roster| valid_roster(roster))
        .ok_or_else(|| format!("{}:{} Boneyard roster is invalid", common.mod_id, common.key))?
        .to_vec();
    let wave_values = common
        .fields
        .get("waves")
        .unwrap_or(&empty)
        .as_array()
        .filter(|waves| waves.len() <= 256 && waves.iter().all(|wave| wave.as_object().is_some()))
        .ok_or_else(|| format!("{}:{} Boneyard waves are invalid", common.mod_id, common.key))?;
    let trigger_values = common
        .fields
        .get("triggers")
        .unwrap_or(&empty)
        .as_array()
        .filter(|values| {
            values.len() <= 64 && values.iter().all(|value| is_token(value, "rule-definition"))
        })
        .ok_or_else(|| format!("{}:{} Boneyard triggers are invalid", common.mod_id, common.key))?;
    let triggers: Vec<WebLuaRuleDefinition> =
        trigger_values.iter().map(decode).collect::<Result<_, _>>()?;
    let environment =
        optional_object(common.fields.get("environment"), &format!("{}.environment", common.key))?;
    exact_keys(&environment, &["mode"], &format!("{}.environment", common.key))?;
    if let Some(mode) = environment.get("mode") {
        integer(Some(mode), 0, 2, &format!("{}.environment.mode", common.key))?;
    }
    let anchors =
        optional_object(common.fields.get("anchors"), &format!("{}.anchors", common.key))?;
    exact_keys(&anchors, &["entry"], &format!("{}.anchors", common.key))?;
    if let Some(entry) = anchors.get("entry") {
        point(Some(entry), &format!("{}.anchors.entry", common.key))?;
    }
    let mut waves = Vec::new();
    for (index, value) in wave_values.iter().enumerate() {
        let field = format!("{}.waves[{index}]", common.key);
        let wave = object(Some(value), &field)?;
        exact_keys(wave, &["ordinal", "roster", "wave"], &field)?;
        if wave.get("wave").is_none() && wave.get("ordinal").is_none() {
            return Err(format!("{}:{} wave requires wave or ordinal", common.mod_id, common.key));
        }
        if wave
            .get("roster")
            .and_then(LuaValue::as_array)
            .is_none_or(|roster| !valid_roster(roster))
        {
            return Err(format!("{}:{} wave roster is invalid", common.mod_id, common.key));
        }
        waves.push(wave.clone());
    }
    Ok(PreparedModBoneyardDefinition {
        common,
        anchors,
        environment,
        roster,
        source,
        triggers,
        waves,
    })
}

fn valid_roster(values: &[LuaValue]) -> bool {
    values.len() <= 64
        && values
            .iter()
            .all(|value| value.as_str().is_some() || is_token(value, "resolved-content-reference"))
}

pub(super) fn boast(
    common: PreparedModContentEntry,
    assets: &PreparedModAssetCatalog,
) -> Result<PreparedModBoastDefinition, String> {
    let stock = common.fields.get("stock_icon");
    let binding = common.art.get("icon");
    if stock.is_none() == binding.is_none() {
        return Err(format!(
            "{}:{} Boast requires exactly one icon source",
            common.mod_id, common.key
        ));
    }
    let icon = match binding {
        None => {
            let style = integer(stock, 0, 7, &format!("{}.stock_icon", common.key))? as usize;
            PreparedModBoastIcon::Stock { record: 90 + style, style }
        }
        Some(binding) => {
            if binding.asset_kind != WebLuaAssetKind::Sprite {
                return Err(format!(
                    "{}:{} Boast art.icon must be a sprite",
                    common.mod_id, common.key
                ));
            }
            let asset = assets.image(&common.mod_id, &binding.key)?;
            if asset.asset_kind != WebLuaAssetKind::Sprite || asset.frames.is_empty() {
                return Err(format!(
                    "{}:{} Boast art.icon has no sprite frame",
                    common.mod_id, common.key
                ));
            }
            let frame = &asset.frames[0];
            if frame.logical_width > 128.0 || frame.logical_height > 128.0 {
                return Err(format!(
                    "{}:{} Boast art.icon exceeds 128 logical pixels",
                    common.mod_id, common.key
                ));
            }
            PreparedModBoastIcon::Mod {
                frame: *frame,
                image_height: asset.height,
                image_path: asset.identity.path.clone(),
                image_width: asset.width,
            }
        }
    };
    let mut failure_producers = Vec::new();
    if let Some(value) = common.fields.get("fail_on") {
        for producer in text_array(Some(value), &format!("{}.fail_on", common.key))? {
            failure_producers.push(match producer.as_str() {
                "magical-equipment" => BoastFailureProducer::MagicalEquipment,
                "mana-underflow" => BoastFailureProducer::ManaUnderflow,
                "potion-use" => BoastFailureProducer::PotionUse,
                "secondary-cast" => BoastFailureProducer::SecondaryCast,
                _ => {
                    return Err(format!(
                        "{}:{} has an unsupported Boast failure producer",
                        common.mod_id, common.key
                    ));
                }
            });
        }
    }
    let instruction = required_text(
        common.fields.get("instruction"),
        1_024,
        &format!("{}.instruction", common.key),
    )?;
    let label = common.name.clone();
    let random_skill_choices = match common.fields.get("random_skill_choices") {
        None => false,
        Some(value) => boolean(Some(value), &format!("{}.random_skill_choices", common.key))?,
    };
    let response =
        required_text(common.fields.get("response"), 1_024, &format!("{}.response", common.key))?;
    let score_multiplier = number_or(
        common.fields.get("score_multiplier"),
        1.1_f32 as f64,
        1.0,
        10.0,
        &format!("{}.score_multiplier", common.key),
    )?;
    let selection = ModBoastSelection::new(common.content_id.clone(), common.mod_id.clone())?;
    let statement =
        required_text(common.fields.get("statement"), 1_024, &format!("{}.statement", common.key))?;
    let success_wave = integer_or(
        common.fields.get("success_wave"),
        30,
        1,
        10_000,
        &format!("{}.success_wave", common.key),
    )? as u32;
    Ok(PreparedModBoastDefinition {
        common,
        failure_producers,
        icon,
        instruction,
        label,
        random_skill_choices,
        response,
        score_multiplier,
        selection,
        statement,
        success_wave,
    })
}

pub(super) fn room(common: PreparedModContentEntry) -> Result<PreparedModRoomDefinition, String> {
    let geometry =
        optional_object(common.fields.get("geometry"), &format!("{}.geometry", common.key))?;
    exact_keys(
        &geometry,
        &["floor", "height", "kind", "walls", "width"],
        &format!("{}.geometry", common.key),
    )?;
    if geometry.get("kind").and_then(LuaValue::as_str) != Some("inline") {
        return Err(format!("{}:{} room geometry kind must be inline", common.mod_id, common.key));
    }
    number(geometry.get("width"), 64.0, 4_096.0, &format!("{}.geometry.width", common.key))?;
    number(geometry.get("height"), 64.0, 4_096.0, &format!("{}.geometry.height", common.key))?;
    let floor_valid = geometry.get("floor").and_then(LuaValue::as_str).is_some_and(|floor| {
        floor.len() == 7
            && floor.starts_with('#')
            && floor.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
    });
    if !floor_valid {
        return Err(format!(
            "{}:{} room floor must be a six-digit hex color",
            common.mod_id, common.key
        ));
    }
    if let Some(value) = geometry.get("walls") {
        let walls = value
            .as_array()
            .ok_or_else(|| format!("{}:{} room walls are invalid", common.mod_id, common.key))?;
        for (index, value) in walls.iter().enumerate() {
            let field = format!("{}.geometry.walls[{index}]", common.key);
            let wall = object(Some(value), &field)?;
            exact_keys(wall, &["color", "height", "width", "x", "y"], &field)?;
            for name in ["height", "width", "x", "y"] {
                let minimum = if matches!(name, "height" | "width") { 1.0 } else { -4_096.0 };
                number(wall.get(name), minimum, 4_096.0, &format!("{field}.{name}"))?;
            }
        }
    }
    if let Some(value) = common.fields.get("props") {
        let props = value
            .as_array()
            .ok_or_else(|| format!("{}:{} room props are invalid", common.mod_id, common.key))?;
        for (index, value) in props.iter().enumerate() {
            let field = format!("{}.props[{index}]", common.key);
            let prop = object(Some(value), &field)?;
            exact_keys(prop, &["color", "kind", "label", "radius", "x", "y"], &field)?;
            for name in ["radius", "x", "y"] {
                number(
                    prop.get(name),
                    if name == "radius" { 1.0 } else { -4_096.0 },
                    4_096.0,
                    &format!("{field}.{name}"),
                )?;
            }
        }
    }
    Ok(PreparedModRoomDefinition { common, geometry })
}

pub(super) fn scene(
    common: PreparedModContentEntry,
    room_ids: &BTreeSet<String>,
) -> Result<PreparedModSceneDefinition, String> {
    let rooms = reference_array(
        nullish(common.fields.get("rooms")).or(Some(&empty_array())),
        WebLuaContentKind::Room,
        &format!("{}.rooms", common.key),
    )?;
    if rooms.is_empty() || rooms.iter().any(|room| !room_ids.contains(&room.content_id)) {
        return Err(format!("{}:{} scene rooms are unavailable", common.mod_id, common.key));
    }
    if common.art.get("layout").is_none_or(|binding| binding.asset_kind != WebLuaAssetKind::Scene) {
        return Err(format!("{}:{} scene requires art.layout", common.mod_id, common.key));
    }
    Ok(PreparedModSceneDefinition { common, rooms })
}

pub(super) fn scene_extension(
    common: PreparedModContentEntry,
) -> Result<PreparedModSceneExtensionDefinition, String> {
    let features = common.fields.get("features").and_then(LuaValue::as_array);
    if common.fields.get("scene").and_then(LuaValue::as_str) != Some("stock.boneyard")
        || features
            .is_none_or(|values| values.iter().any(|value| !is_token(value, "rule-definition")))
    {
        return Err(format!("{}:{} scene extension is invalid", common.mod_id, common.key));
    }
    let features = features
        .expect("feature rules were validated")
        .iter()
        .map(decode)
        .collect::<Result<_, _>>()?;
    Ok(PreparedModSceneExtensionDefinition { common, features, scene: "stock.boneyard".into() })
}

pub(super) fn shop(
    common: PreparedModContentEntry,
    item_ids: &BTreeSet<String>,
    wearable_ids: &BTreeSet<String>,
    pool_ids: &BTreeSet<String>,
) -> Result<PreparedModShopDefinition, String> {
    let values = common
        .fields
        .get("stock")
        .and_then(LuaValue::as_array)
        .filter(|stock| !stock.is_empty())
        .ok_or_else(|| format!("{}:{} shop requires stock", common.mod_id, common.key))?;
    let mut stock = Vec::new();
    for (index, value) in values.iter().enumerate() {
        let field = format!("{}.stock[{index}]", common.key);
        let row = object(Some(value), &field)?;
        exact_keys(row, &["item", "price", "quantity"], &field)?;
        let item = reference(row.get("item"), &format!("{field}.item"))?;
        if !item_ids.contains(&item.content_id)
            || !matches!(item.target_kind, WebLuaContentKind::Item | WebLuaContentKind::Potion)
        {
            return Err(format!("{}:{} shop item is unavailable", common.mod_id, common.key));
        }
        let quantity =
            integer_or(row.get("quantity"), 1, 1, 9_999, &format!("{field}.quantity"))? as u32;
        if wearable_ids.contains(&item.content_id) && quantity != 1 {
            return Err(format!(
                "{}:{} wearable shop stock quantity must be one",
                common.mod_id, common.key
            ));
        }
        let price = integer(row.get("price"), 0, 10_000_000, &format!("{field}.price"))?;
        stock.push(PreparedModShopStock { item, price, quantity });
    }
    let mut services = Vec::new();
    if let Some(value) = common.fields.get("services") {
        let values = value
            .as_array()
            .filter(|services| services.len() <= 32)
            .ok_or_else(|| format!("{}:{} shop services are invalid", common.mod_id, common.key))?;
        for (index, value) in values.iter().enumerate() {
            let field = format!("{}.services[{index}]", common.key);
            let service = object(Some(value), &field)?;
            exact_keys(service, &["pool", "price", "type"], &field)?;
            let pool = reference(service.get("pool"), &format!("{field}.pool"))?;
            if service.get("type").and_then(LuaValue::as_str) != Some("reforge")
                || pool.target_kind != WebLuaContentKind::AffixPool
                || !pool_ids.contains(&pool.content_id)
            {
                return Err(format!("{}:{} reforge service is invalid", common.mod_id, common.key));
            }
            let price = integer(service.get("price"), 0, 10_000_000, &format!("{field}.price"))?;
            services.push(PreparedModShopService { pool, price });
        }
    }
    let mount = common
        .fields
        .get("mount")
        .map(|value| optional_object(Some(value), &format!("{}.mount", common.key)))
        .transpose()?;
    if let Some(mount) = &mount {
        exact_keys(mount, &["radius", "scene", "x", "y"], &format!("{}.mount", common.key))?;
        if !matches!(
            mount.get("scene").and_then(LuaValue::as_str),
            Some("hub.courtyard" | "boneyard")
        ) {
            return Err(format!("{}:{} shop mount scene is invalid", common.mod_id, common.key));
        }
        number(mount.get("x"), -100_000.0, 100_000.0, &format!("{}.mount.x", common.key))?;
        number(mount.get("y"), -100_000.0, 100_000.0, &format!("{}.mount.y", common.key))?;
        if let Some(radius) = mount.get("radius") {
            number(Some(radius), 1.0, 2_048.0, &format!("{}.mount.radius", common.key))?;
        }
    }
    let npc = common
        .fields
        .get("npc")
        .map(|value| optional_object(Some(value), &format!("{}.npc", common.key)))
        .transpose()?;
    if let Some(npc) = &npc {
        exact_keys(npc, &["name"], &format!("{}.npc", common.key))?;
        optional_text(npc.get("name"), &common.name, 128, &format!("{}.npc.name", common.key))?;
    }
    let restock_ms = duration_ms(
        nullish(common.fields.get("restock")).or(Some(&LuaValue::Number(0.0))),
        &format!("{}.restock", common.key),
    )?;
    let stock_scope = match nullish(common.fields.get("stock_scope")).and_then(LuaValue::as_str) {
        None if nullish(common.fields.get("stock_scope")).is_none() => ModShopStockScope::Player,
        Some("player") => ModShopStockScope::Player,
        Some("party") => ModShopStockScope::Party,
        Some("session") => ModShopStockScope::Session,
        _ => return Err(format!("{}:{} shop stock scope is invalid", common.mod_id, common.key)),
    };
    Ok(PreparedModShopDefinition { common, mount, npc, restock_ms, services, stock, stock_scope })
}

pub(super) fn ui(common: PreparedModContentEntry) -> Result<PreparedModUiDefinition, String> {
    let mount = common.fields.get("mount").and_then(LuaValue::as_str);
    let view = common.fields.get("view");
    if !matches!(
        mount,
        Some(
            "hud.bottom_left"
                | "hud.bottom_right"
                | "hud.overlay"
                | "hud.top_left"
                | "hud.top_right"
        )
    ) || view.is_none_or(|value| {
        !is_token(value, "rule-definition")
            || value.get("operation").and_then(LuaValue::as_str) != Some("prefab.minimap")
    }) {
        return Err(format!("{}:{} UI mount or view is invalid", common.mod_id, common.key));
    }
    let mount = mount.expect("UI mount was validated").to_owned();
    let view = decode(view.expect("UI view was validated"))?;
    let bindings =
        optional_object(common.fields.get("bindings"), &format!("{}.bindings", common.key))?;
    for (name, value) in bindings.iter() {
        let field = format!("{}.bindings.{name}", common.key);
        let row = object(Some(value), &field)?;
        exact_keys(row, &["state"], &field)?;
        if row.get("state").and_then(LuaValue::as_str).is_none_or(str::is_empty) {
            return Err(format!(
                "{}:{} UI binding {name} requires state",
                common.mod_id, common.key
            ));
        }
    }
    let visible =
        optional_object(common.fields.get("visible"), &format!("{}.visible", common.key))?;
    exact_keys(&visible, &["scenes", "state"], &format!("{}.visible", common.key))?;
    if let Some(value) = visible.get("scenes")
        && value.as_array().is_none_or(|scenes| {
            scenes.is_empty()
                || scenes
                    .iter()
                    .any(|scene| !matches!(scene.as_str(), Some("hub" | "boneyard" | "room")))
        })
    {
        return Err(format!("{}:{} UI visible.scenes is invalid", common.mod_id, common.key));
    }
    if let Some(value) = visible.get("state") {
        let field = format!("{}.visible.state", common.key);
        let condition = object(Some(value), &field)?;
        exact_keys(condition, &["equals", "state"], &field)?;
        if condition.get("state").and_then(LuaValue::as_str).is_none_or(str::is_empty) {
            return Err(format!(
                "{}:{} UI visible.state requires state",
                common.mod_id, common.key
            ));
        }
    }
    let accessible_name = optional_text(
        common.fields.get("accessible_name"),
        &common.name,
        128,
        &format!("{}.accessible_name", common.key),
    )?;
    let actions = match common.fields.get("actions") {
        None => Vec::new(),
        Some(value) => text_array(Some(value), &format!("{}.actions", common.key))?,
    };
    Ok(PreparedModUiDefinition { common, accessible_name, actions, bindings, mount, visible, view })
}
