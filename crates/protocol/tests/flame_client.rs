//! Actual current Source admission, replay retirement, live fallback and camera retention.
use serde_json::{Value, json};
use solomon_protocol::flame_presentation::*;
use solomon_sim::Vec2;
use solomon_sim::spells::model::{
    PrimarySpellFireParticle, PrimarySpellTransient, PrimarySpellTransientBase,
    PrimaryTransientRef, SpellState,
};
use solomon_sim::spells::weld::WeldWorldRef;

#[path = "../../sim/tests/support/website_rows.rs"]
mod website_rows;

fn state(births: &[FlamePresentationBirth], indices: &Value) -> SpellState {
    let ordinary = PrimarySpellTransient::FireParticle(PrimarySpellFireParticle {
        common: PrimarySpellTransientBase {
            age_ticks: 7,
            id: 3,
            light_registration: None,
            owner_id: "hero".into(),
            painter_registrations: Vec::new(),
            world_key: "boneyard:flame".into(),
        },
        enhanced_effects: true,
        direction: Vec2::ZERO,
        origin: Vec2::ZERO,
        variant: 0,
    });
    let mut state = SpellState {
        next_id: 100,
        projectiles: Default::default(),
        transients: [ordinary].into_iter().collect(),
    };
    for index in indices.as_array().unwrap() {
        let birth = &births[index.as_u64().unwrap() as usize];
        state
            .transients
            .push(materialize_flame_presentation(birth, birth.birth_tick as f64).unwrap());
    }
    state
}
fn rows(state: &SpellState) -> Value {
    let rows: Vec<_> = state
        .transients
        .iter()
        .map(|actor| {
            let (kind, alpha) = match actor {
                PrimaryTransientRef::Weld(WeldWorldRef::Channel(_)) => ("weld-channel", None),
                PrimaryTransientRef::Weld(WeldWorldRef::FlameLashFade(fade)) => {
                    ("weld-flame-lash-fade", Some(fade.alpha))
                }
                PrimaryTransientRef::FireParticle(_) => ("fire", None),
                _ => unreachable!(),
            };
            json!({"id":actor.common().id,"kind":kind,"age":actor.common().age_ticks,"alpha":alpha})
        })
        .collect();
    json!(rows)
}

#[test]
fn retained_client_uses_exact_birth_visibility_tombstones_and_reset_boundaries() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../sim/tests/fixtures/website/flame-client-goldens.json"
    ))
    .unwrap();
    let births: Vec<FlamePresentationBirth> =
        serde_json::from_value(fixture["births"].clone()).unwrap();
    for case in fixture["replays"].as_array().unwrap() {
        let initial: FlamePresentationFrame =
            serde_json::from_value(case["initial"].clone()).unwrap();
        let mut client = RetainedFlamePresentation::new(Some(&initial)).unwrap();
        for step in case["steps"].as_array().unwrap() {
            if step["kind"] == "accept" {
                let frame: FlamePresentationFrame =
                    serde_json::from_value(step["frame"].clone()).unwrap();
                let result = client.accept(Some(&frame), step["oldestTick"].as_u64().unwrap());
                if let Some(error) = step["error"].as_str() {
                    assert_eq!(result.unwrap_err(), error);
                } else {
                    assert_eq!(result.unwrap(), step["replaced"].as_bool().unwrap());
                }
                continue;
            }
            let tick = step["tick"].as_f64().unwrap();
            let mut state = state(&births, &step["stateBirths"]);
            let cameras = client
                .camera_displacements(&state, tick, step["liveTick"].as_f64().unwrap())
                .unwrap();
            let mut camera_rows = Vec::new();
            for proposal in cameras {
                camera_rows.push(json!({
                    "id": proposal.id,
                    "birthTick": proposal.birth_tick,
                    "worldKey": proposal.world_key,
                    "displacement": {"x": proposal.displacement.x, "y": proposal.displacement.y},
                }));
            }
            website_rows::compare(
                &json!(camera_rows),
                &step["cameras"],
                "Retained camera lifetime",
            );
            client.sample(&mut state, tick).unwrap();
            website_rows::compare(&rows(&state), &step["actors"], case["name"].as_str().unwrap());
        }
    }
}
