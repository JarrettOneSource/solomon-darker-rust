use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::loot::messages::*;
use solomon_sim::boneyard::loot::presentation::*;
use solomon_sim::boneyard::loot::*;
use solomon_sim::economy::HubEconomyState;
use solomon_sim::hub::painter::{PainterLane, PainterLayer};
use solomon_sim::loot::*;
use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
use solomon_sim::profile::WizardElement;
use solomon_sim::spells::presentation::{SpellAtlas, SpellBlend};

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

fn actor(value: &Value) -> BoneyardLootActor {
    let item = value["itemNativeTypeId"].as_u64().map(|native_type_id| {
        let economy = HubEconomyState::new(1, WizardElement::Air);
        let mut item = economy.backpack[0].clone();
        item.native_type_id = native_type_id as u32;
        item.native_subtype = value["itemNativeSubtype"].as_u64().map(|value| value as u32);
        item
    });
    BoneyardLootActor {
        activation_delay_ticks: 0,
        age_ticks: 0,
        alpha: value["alpha"].as_f64().unwrap(),
        amount: 0,
        animation_phase: value["animationPhase"].as_f64().unwrap(),
        bonus_kind: value["bonusKind"].as_u64().and_then(|id| NativeBonusKind::from_id(id as u32)),
        bounce_height: value["bounceHeight"].as_f64().unwrap(),
        bounce_velocity: 0.0,
        frame_phase: value["framePhase"].as_f64().unwrap(),
        id: value["id"].as_u64().unwrap() as usize,
        item,
        kind: match value["kind"].as_str().unwrap() {
            "orb" => NativeLootDropKind::Orb,
            "gold" => NativeLootDropKind::Gold,
            "sack" => NativeLootDropKind::Sack,
            "bonus" => NativeLootDropKind::Bonus,
            kind => panic!("unknown loot kind {kind}"),
        },
        last_step_tick: None,
        lifetime_ticks_remaining: None,
        native_type_id: 0,
        orb_kind: Some(if value["orbKind"] == "health" {
            NativeOrbKind::Health
        } else {
            NativeOrbKind::Mana
        }),
        orb_value: value["orbValue"].as_f64().unwrap(),
        painter_registration: PainterRegistration {
            manager_lane: WorldManagerLane::Actor,
            registration_ordinal: value["painterRegistration"]["registrationOrdinal"]
                .as_u64()
                .unwrap() as usize,
        },
        position: point(&value["position"]),
        rotation_deg: value["rotationDeg"].as_f64().unwrap(),
        scatter_seed: value["scatterSeed"].as_u64().unwrap() as u32,
        scatter_active: value["scatterActive"].as_bool().unwrap(),
        scatter_progress: value["scatterProgress"].as_f64().unwrap(),
        settled_sound_emitted: false,
        source: NativeLootDropSource::Script,
        spawn_tick: 0,
        tier: value["tier"].as_u64().unwrap() as u32,
    }
}

fn goodie(value: &Value) -> BoneyardGoodieState {
    BoneyardGoodieState {
        active: value["active"].as_bool().unwrap(),
        activated_by_player_id: None,
        eid: "oracle".into(),
        exhausted: false,
        id: value["id"].as_u64().unwrap() as usize,
        phase: match value["phase"].as_u64().unwrap() {
            0 => BoneyardGoodiePhase::Intact,
            1 => BoneyardGoodiePhase::Opening,
            2 => BoneyardGoodiePhase::Broken,
            _ => unreachable!(),
        },
        position: point(&value["position"]),
        reward_seed: 0,
        scenery_registration_ordinal: value["sceneryRegistrationOrdinal"].as_u64().unwrap()
            as usize,
        subtype: 0,
        timer: value["timer"].as_u64().unwrap(),
    }
}

fn layer_snapshot(draw: &NativeLootVisualLayer) -> Value {
    let mut value = json!({ "alpha": draw.alpha, "atlas": match draw.atlas {
        SpellAtlas::BadGuys => "BadGuys", SpellAtlas::DeadHawg => "DeadHawg",
    }, "blendMode": match draw.blend { SpellBlend::Normal => "normal", SpellBlend::Add => "add" },
        "entry": draw.entry, "offset": { "x": draw.offset.x, "y": draw.offset.y },
        "role": draw.role, "rotationRadians": draw.rotation_radians,
        "scale": { "x": draw.scale.x, "y": draw.scale.y }, "tint": draw.tint });
    if draw.diffuse_color {
        value["textureColor"] = json!("diffuse");
    }
    value
}

fn plan_snapshot(plan: &NativeLootPresentationPlan) -> Value {
    json!({ "layers": plan.layers.iter().map(layer_snapshot).collect::<Vec<_>>(),
        "nextScatterSeed": plan.next_scatter_seed })
}

fn painter_snapshot(layer: PainterLayer, id: &str, family: &str) -> Value {
    json!({ "id": id, "queueFamily": family, "registration": {
        "managerLane": match layer.manager_lane {
            PainterLane::Actor => "actor", PainterLane::Scenery => "scenery", PainterLane::Transient => "transient",
        }, "registrationOrdinal": layer.registration_ordinal,
    }, "sortBias": layer.sort_bias, "worldY": layer.world_y })
}

fn assert_snapshot(actual: &Value, expected: &Value, path: &str, trig: bool) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            let actual = actual.as_f64().unwrap();
            let expected = expected.as_f64().unwrap();
            if trig
                && (path.contains(".offset.")
                    || path.contains(".scale.")
                    || path.ends_with(".alpha"))
            {
                // Only orb/gold sine and cosine arithmetic permits the libm tolerance.
                assert!((actual - expected).abs() <= 1e-12, "{path}: {actual} != {expected}");
            } else {
                assert_eq!(actual, expected, "{path}");
            }
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} length");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                assert_snapshot(actual, expected, &format!("{path}[{index}]"), trig);
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path} field count");
            for (key, expected) in expected {
                assert_snapshot(&actual[key], expected, &format!("{path}.{key}"), trig);
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn ground_loot_scatter_goodies_and_hit_copies_replay_the_source() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/loot-presentation-goldens.json"))
            .unwrap();
    for (index, case) in fixture["actors"].as_array().unwrap().iter().enumerate() {
        let actor = actor(&case["actor"]);
        let plan = native_loot_presentation_plan(&actor, actor.scatter_seed);
        let trig = matches!(actor.kind, NativeLootDropKind::Gold | NativeLootDropKind::Orb);
        assert_snapshot(&plan_snapshot(&plan), &case["plan"], &format!("actor:{index}"), trig);
        assert_snapshot(
            &painter_snapshot(
                native_loot_painter_layer(&actor),
                &format!("loot:{}", actor.id),
                "ordinary-dynamic",
            ),
            &case["painter"],
            &format!("painter:{index}"),
            false,
        );
    }
    for (index, case) in fixture["scatters"].as_array().unwrap().iter().enumerate() {
        let actor = actor(&case["actor"]);
        let mut seed = actor.scatter_seed;
        for (frame, expected) in case["plans"].as_array().unwrap().iter().enumerate() {
            let plan = native_loot_presentation_plan(&actor, seed);
            seed = plan.next_scatter_seed;
            assert_snapshot(
                &plan_snapshot(&plan),
                expected,
                &format!("scatter:{index}:{frame}"),
                true,
            );
        }
    }
    for (index, case) in fixture["goodies"].as_array().unwrap().iter().enumerate() {
        let goodie = goodie(&case["goodie"]);
        let draws = native_goodie_presentation_plan(
            &goodie,
            case["tick"].as_f64().unwrap(),
            case["hitAlpha"].as_f64().unwrap(),
            case["complexLighting"].as_bool().unwrap(),
        );
        assert_snapshot(
            &json!(draws.iter().map(layer_snapshot).collect::<Vec<_>>()),
            &case["layers"],
            &format!("goodie:{index}"),
            false,
        );
        assert_snapshot(
            &painter_snapshot(
                native_goodie_painter_layer(&goodie),
                &format!("goodie:{}", goodie.id),
                "scenery",
            ),
            &case["painter"],
            &format!("goodie-painter:{index}"),
            false,
        );
    }
}

#[test]
fn pickup_messages_coalesce_gold_and_preserve_source_stack_and_fade_timing() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/loot-presentation-goldens.json"))
            .unwrap();
    let mut presentation = NativeLootMessagePresentation::new(0);
    for (index, case) in fixture["journeys"].as_array().unwrap().iter().enumerate() {
        if let Some(event) = case["event"].as_object() {
            let event = BoneyardLootEvent {
                actor_id: 1,
                event_id: event["eventId"].as_u64().unwrap(),
                goodie_id: None,
                phase: None,
                playback_rate: None,
                player_id: Some("player".into()),
                position: Vec2::ZERO,
                sound: (event.get("sound").and_then(Value::as_str) == Some("pickup-coin"))
                    .then_some(BoneyardLootSound::PickupCoin),
                text: event.get("text").and_then(Value::as_str).map(String::from),
                tick: event["tick"].as_u64().unwrap(),
                event_type: BoneyardLootEventType::LootPickup,
            };
            assert_eq!(
                presentation.consume(&event),
                case["accepted"].as_bool().unwrap(),
                "event:{index}"
            );
        } else {
            let sample = presentation.sample(case["tick"].as_u64().unwrap());
            let sample = sample.iter().map(|visual| json!({ "eventId": visual.message.event_id,
                "lifetime": visual.message.lifetime, "offset": visual.message.offset,
                "text": visual.message.text, "tint": visual.message.tint, "alpha": visual.alpha, "scale": visual.scale })).collect::<Vec<_>>();
            assert_snapshot(&json!(sample), &case["sample"], &format!("sample:{index}"), false);
        }
    }
}
