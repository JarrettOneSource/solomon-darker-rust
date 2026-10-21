//! The Create/staff/selected-primary element plans share every authored sprite record.
use serde::Deserialize;
use solomon_sim::element_vfx::{self, ElementVfxBlend, ElementVfxDraw, ElementVfxSprite};
use solomon_sim::profile::WizardElement;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Golden {
    records: BTreeMap<String, Vec<usize>>,
    cases: Vec<ElementCase>,
    selected: Vec<SelectedCase>,
}

#[derive(Deserialize)]
struct ElementCase {
    element: String,
    phase: f64,
    scale: f64,
    plan: Vec<Draw>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SelectedCase {
    skill_id: u32,
    tick: f64,
    scale: f64,
    plan: Vec<Draw>,
}

#[derive(Deserialize)]
struct Draw {
    alpha: f64,
    anchor: Option<[f64; 2]>,
    blend: String,
    color: [f64; 3],
    frame: usize,
    rotation: f64,
    scale: f64,
    sprite: String,
    x: f64,
    y: f64,
}

fn sprite(name: &str) -> ElementVfxSprite {
    match name {
        "air" => ElementVfxSprite::Air,
        "aura" => ElementVfxSprite::Aura,
        "core" => ElementVfxSprite::Core,
        "earth" => ElementVfxSprite::Earth,
        "fire" => ElementVfxSprite::Fire,
        "ray" => ElementVfxSprite::Ray,
        "spark" => ElementVfxSprite::Spark,
        "steam" => ElementVfxSprite::Steam,
        "water" => ElementVfxSprite::Water,
        _ => panic!("unknown fixture sprite {name}"),
    }
}

fn compare_plan(actual: &[ElementVfxDraw], expected: &[Draw]) {
    assert_eq!(actual.len(), expected.len(), "element VFX pass count");
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual.sprite, sprite(&expected.sprite), "pass {index} sprite");
        assert_eq!(actual.frame, expected.frame, "pass {index} frame");
        assert_eq!(actual.anchor, expected.anchor, "pass {index} anchor");
        assert_eq!(actual.color, expected.color, "pass {index} tint");
        let blend = if expected.blend == "lighter" {
            ElementVfxBlend::Add
        } else {
            ElementVfxBlend::Normal
        };
        assert_eq!(actual.blend, blend, "pass {index} blend");
        // These channels contain sin/cos results. Record identity, hashing, anchors and
        // tints remain exact above; native libm and V8 may differ in the last trig bit.
        for (actual, expected) in [actual.alpha, actual.rotation, actual.scale, actual.x, actual.y]
            .into_iter()
            .zip([expected.alpha, expected.rotation, expected.scale, expected.x, expected.y])
        {
            assert!((actual - expected).abs() <= 1e-9, "pass {index}: {actual} != {expected}");
        }
    }
}

#[test]
fn elements_and_every_selected_weld_plan_replay_the_website() {
    let golden: Golden =
        serde_json::from_str(include_str!("fixtures/website/element-vfx-goldens.json"))
            .expect("element VFX fixture");
    for (name, records) in golden.records {
        assert_eq!(sprite(&name).records(), records);
    }
    for case in golden.cases {
        let element = WizardElement::ALL
            .into_iter()
            .find(|element| element.as_str() == case.element)
            .expect("fixture element");
        compare_plan(
            &element_vfx::native_element_vfx_plan_at_phase(element, case.phase, case.scale),
            &case.plan,
        );
    }
    for case in golden.selected {
        compare_plan(
            &element_vfx::native_selected_primary_element_vfx_plan(
                case.skill_id,
                case.tick,
                case.scale,
            ),
            &case.plan,
        );
    }
}
