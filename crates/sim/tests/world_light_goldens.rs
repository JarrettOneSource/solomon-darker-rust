//! The complete source manager/camera query with normalized entity-factory submissions.

use serde_json::Value;
use solomon_sim::Vec2;
use solomon_sim::boneyard::BoneyardBounds;
use solomon_sim::boneyard::lighting::NativeBoneyardLightSource;
use solomon_sim::boneyard::world_light::{
    BoneyardWorldLightQuery, WorldLightOwner, WorldLightPlayer, WorldLightScene,
    WorldMiscLightOwner, native_primary_view_bounds, native_region_point_gain,
};
use solomon_sim::painter::{PainterRegistration, WorldManagerLane};

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap()
}

fn point(value: &Value) -> Vec2 {
    Vec2::new(number(&value["x"]), number(&value["y"]))
}

fn bounds(value: &Value) -> BoneyardBounds {
    BoneyardBounds::new(
        number(&value["x"]),
        number(&value["y"]),
        number(&value["w"]),
        number(&value["h"]),
    )
}

fn registration(value: &Value) -> PainterRegistration {
    PainterRegistration {
        manager_lane: match value["managerLane"].as_str().unwrap() {
            "actor" => WorldManagerLane::Actor,
            "transient" => WorldManagerLane::Transient,
            value => panic!("unknown manager lane {value}"),
        },
        registration_ordinal: value["registrationOrdinal"].as_u64().unwrap() as usize,
    }
}

fn source(value: &Value) -> NativeBoneyardLightSource {
    NativeBoneyardLightSource {
        intensity: number(&value["intensity"]),
        position: point(&value["position"]),
        radius: number(&value["radius"]),
        raster_scale: value["rasterScale"].as_f64(),
        casts_directional_shadow: value["castsDirectionalShadow"].as_bool().unwrap(),
    }
}

fn owner(value: &Value) -> WorldLightOwner {
    WorldLightOwner {
        registration: registration(&value["registration"]),
        sources: value["sources"].as_array().unwrap().iter().map(source).collect(),
    }
}

#[test]
fn all_participant_cameras_collect_native_light_owners_in_exact_manager_order() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/world-light-goldens.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let world = &case["world"];
        let players: Vec<_> = case["players"]
            .as_array()
            .unwrap()
            .iter()
            .map(|player| WorldLightPlayer {
                id: player["id"].as_str().unwrap(),
                position: point(&player["position"]),
                heading_index: player["headingIndex"].as_u64().unwrap() as usize,
                weapon_pulse: number(&player["weaponPulse"]),
                overlay_effect_phase: number(&player["overlayEffectPhase"]),
                light_registration: if player["lightRegistration"].is_null() {
                    None
                } else {
                    Some(registration(&player["lightRegistration"]))
                },
                viewport: if player["viewport"].is_null() {
                    None
                } else {
                    Some(point(&player["viewport"]))
                },
            })
            .collect();
        let providers: Vec<_> =
            case["providerOwners"].as_array().unwrap().iter().map(owner).collect();
        let misc: Vec<_> = case["miscOwners"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| WorldMiscLightOwner {
                birth_tick: entry["birthTick"].as_i64().unwrap(),
                id: entry["id"].as_u64().unwrap() as usize,
                append_ordinal: entry["appendOrdinal"].as_u64().unwrap() as usize,
                owner: owner(&entry["owner"]),
            })
            .collect();
        let query = BoneyardWorldLightQuery::new(
            WorldLightScene {
                bounds: bounds(&world["bounds"]),
                camera_bounds: bounds(&world["arenaTransition"]["cameraBounds"]),
                lantern_position: Some(point(&world["lanternPosition"])),
                lantern_light_registration: Some(registration(&world["lanternLightRegistration"])),
            },
            &players,
            &providers,
            &misc,
            case["tick"].as_u64().unwrap(),
        );
        let expected = &case["expected"];
        let cameras: Vec<_> = expected["cameras"].as_array().unwrap().iter().map(bounds).collect();
        assert_eq!(query.cameras, cameras);
        let accepted: Vec<_> =
            expected["acceptedSources"].as_array().unwrap().iter().map(source).collect();
        assert_eq!(query.accepted_sources().copied().collect::<Vec<_>>(), accepted);
        for record in expected["queries"].as_array().unwrap() {
            let position = point(&record["position"]);
            assert_eq!(
                query.scalar_at(position),
                number(&record["scalar"]),
                "scalar at {position:?}"
            );
            assert_eq!(
                query.point_gain_at(position),
                number(&record["pointGain"]),
                "point gain at {position:?}"
            );
        }
    }
    for case in fixture["views"].as_array().unwrap() {
        assert_eq!(
            native_primary_view_bounds(
                bounds(&case["bounds"]),
                point(&case["focus"]),
                point(&case["viewport"]),
                number(&case["scale"]),
                number(&case["padding"])
            ),
            bounds(&case["expected"])
        );
    }
    for case in fixture["gains"].as_array().unwrap() {
        assert_eq!(
            native_region_point_gain(
                point(&case["position"]),
                point(&case["cameraCenter"]),
                number(&case["width"]),
                case["alternate"].as_bool().unwrap()
            ),
            number(&case["expected"])
        );
    }
}
