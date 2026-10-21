//! Reuse the source's authored sockets and check actual glow/ordering boundaries.
use serde_json::Value;
use solomon_sim::Vec2;
use solomon_sim::player_presentation::*;

fn number(value: &Value, key: &str) -> f64 {
    value[key].as_f64().unwrap()
}
fn integer(value: &Value, key: &str) -> usize {
    value[key].as_u64().unwrap() as usize
}
fn point(value: &Value) -> Vec2 {
    Vec2::new(value[0].as_f64().unwrap(), value[1].as_f64().unwrap())
}

#[test]
fn enchant_staff_and_orb_banks_replay_source_attachments() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/player-attachments-goldens.json"))
            .unwrap();
    for case in fixture["staff"].as_array().unwrap() {
        let input = &case["input"];
        let frame = &case["frame"];
        let actual = native_enchant_staff_draw_plan(NativeEnchantStaffDrawInput {
            attachment: NativeStaffAttachmentFrame {
                start: point(&frame["start"]),
                end: point(&frame["end"]),
                front: frame["front"].as_bool().unwrap(),
            },
            effective_rank: if input["learnedSkills"].as_array().unwrap().is_empty() {
                0
            } else {
                3
            },
            heading_index: integer(input, "headingIndex"),
            living: input["living"].as_bool().unwrap(),
            native_staff: input["nativeStaff"].as_bool().unwrap(),
            pose: integer(input, "pose"),
            selected_primary_skill_id: input["selectedPrimarySkillId"].as_u64().map(|id| id as u32),
            selector: integer(input, "selector"),
            tick: number(input, "tick"),
            weld_build_id: None,
            width_sample: input["widthSample"].as_f64(),
        });
        let expected = &case["plan"];
        if expected.is_null() {
            assert_eq!(actual, None);
            continue;
        }
        let actual = actual.unwrap();
        assert_eq!(actual.aura_record, expected["auraRecord"].as_u64().map(|id| id as usize));
        assert_eq!(actual.body_record, integer(expected, "bodyRecord"));
        assert_eq!(actual.front, expected["front"].as_bool().unwrap());
        assert_eq!(actual.tint, integer(expected, "tint") as u32);
        assert_eq!(actual.near_alpha, number(expected, "nearAlpha"));
        assert_eq!(actual.far_alpha, number(expected, "farAlpha"));
        assert_eq!(actual.width_factor, number(expected, "widthFactor"));
        assert_eq!(actual.start, point(&expected["start"]));
        assert_eq!(actual.end, point(&expected["end"]));
        for (index, vertex) in actual.vertices.iter().enumerate() {
            let x = expected["vertices"][index * 2].as_f64().unwrap();
            let y = expected["vertices"][index * 2 + 1].as_f64().unwrap();
            assert!((vertex.x - x).abs() < 1.0e-10 && (vertex.y - y).abs() < 1.0e-10);
        }
    }
    for row in fixture["weldColors"].as_array().unwrap() {
        assert_eq!(
            native_enchant_staff_glow_tint(Some(52), Some(integer(row, "build") as u32)),
            Some(integer(row, "tint") as u32)
        );
    }
    for row in fixture["banks"].as_array().unwrap() {
        let heading = integer(row, "headingIndex");
        let pose = integer(row, "pose");
        let phase = number(row, "phase");
        assert_eq!(player_character_staff_is_front(heading, pose), row["front"].as_bool().unwrap());
        let passes = player_character_staff_orb_passes(heading, pose, phase);
        assert_eq!(passes.front_base, row["passes"]["frontBase"].as_bool().unwrap());
        assert_eq!(passes.front_overlay, row["passes"]["frontOverlay"].as_bool().unwrap());
        assert_eq!(player_equipped_element_effect_scale(phase), number(row, "scale"));
    }
}
