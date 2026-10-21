use serde_json::{Value, json};
use solomon_sim::modding::consumable_presentation::*;
use solomon_sim::modding::inventory::GameSimulationModEffect;

#[test]
fn mod_consumable_glow_and_authored_ring_match_source() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/website/mod-consumable-presentation-goldens.json"
    ))
    .unwrap();
    for case in fixture["plans"].as_array().unwrap() {
        let effect = &case["effect"];
        let effect = GameSimulationModEffect {
            color: std::array::from_fn(|index| effect["color"][index].as_f64().unwrap()),
            content_id: effect["contentId"].as_str().unwrap().into(),
            expires_tick: effect["expiresTick"].as_u64().unwrap(),
            player_id: effect["playerId"].as_str().unwrap().into(),
            started_tick: effect["startedTick"].as_u64().unwrap(),
            use_id: effect["useId"].as_u64().unwrap(),
        };
        assert_eq!(json!(mod_consumable_effect_id(&effect)), case["id"]);
        let plan = mod_consumable_effect_plan(&effect, case["tick"].as_f64().unwrap());
        let expected = &case["plan"];
        assert_eq!(plan.alpha, expected["alpha"].as_f64().unwrap());
        for index in 0..4 {
            assert_eq!(plan.flash_alphas[index], expected["flashAlphas"][index].as_f64().unwrap());
            assert_eq!(
                plan.flash_rotations[index],
                expected["flashRotations"][index].as_f64().unwrap()
            );
        }
        assert_eq!(json!(plan.flash_visible), expected["flashVisible"]);
        assert_eq!(json!(plan.tint), expected["tint"]);
        // Only the radius pulse and activation scales contain sine arithmetic.
        assert!((plan.radius - expected["radius"].as_f64().unwrap()).abs() <= 1e-12);
        for (index, scale) in plan.flash_scales.iter().enumerate() {
            assert!((scale - expected["flashScales"][index].as_f64().unwrap()).abs() <= 1e-12);
        }
    }
    let pixels = mod_consumable_ring_rgba();
    for (index, alpha) in fixture["ringAlphas"].as_array().unwrap().iter().enumerate() {
        assert_eq!(&pixels[index * 4..index * 4 + 3], &[255, 255, 255]);
        assert_eq!(pixels[index * 4 + 3], alpha.as_u64().unwrap() as u8, "pixel {index}");
    }
}
