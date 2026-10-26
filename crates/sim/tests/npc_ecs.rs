#![cfg(feature = "persistence")]

use solomon_sim::hub::npc::{
    HubSkorchaSchedule, HubStudentPopulation, HubStudentSplines, HubStudentState,
};

#[test]
fn student_route_retirement_birth_and_saved_continuation_preserve_population_order() {
    let splines = HubStudentSplines::new();
    let mut records: Vec<_> = (1..=3)
        .map(|id| HubStudentState::new(&splines, id, 0, id as u32 * 19, 0.75, id + 40))
        .collect();
    records[1].controller.path_cursor = -1.0;
    let mut live = HubStudentPopulation::cold(718, 44);
    live.spawning_enabled = false;
    live.next_id = 4;
    live.students = records.clone().into_iter().collect();
    assert_eq!(
        serde_json::to_value(&live.students).unwrap(),
        serde_json::to_value(&records).unwrap()
    );
    live.step_routes(None);
    assert_eq!(live.students.iter().map(|student| student.id).collect::<Vec<_>>(), [1, 3]);
    live.students.push(HubStudentState::new(&splines, 4, 1, 91, 0.9, 44));
    live.next_id = 5;
    assert_eq!(live.students.iter().map(|student| student.id).collect::<Vec<_>>(), [1, 3, 4]);

    let saved = serde_json::to_vec(&live).unwrap();
    let mut restored: HubStudentPopulation = serde_json::from_slice(&saved).unwrap();
    assert_eq!(serde_json::to_vec(&restored).unwrap(), saved);
    for _ in 0..300 {
        live.step_routes(None);
        restored.step_routes(None);
        assert_eq!(serde_json::to_vec(&restored).unwrap(), serde_json::to_vec(&live).unwrap());
    }
}

#[test]
fn skorcha_saved_windows_continue_through_retirement_and_reappearance() {
    let mut live = HubSkorchaSchedule::with_windows(719, Some(3), Some(4));
    for _ in 0..5 {
        live.step();
    }
    let saved = serde_json::to_vec(&live).unwrap();
    let mut restored: HubSkorchaSchedule = serde_json::from_slice(&saved).unwrap();
    assert_eq!(serde_json::to_vec(&restored).unwrap(), saved);
    let mut saw_present = false;
    let mut saw_absent = false;
    for _ in 0..30 {
        live.step();
        restored.step();
        saw_present |= live.skorcha.is_some();
        saw_absent |= !live.skorcha.is_some();
        assert_eq!(serde_json::to_vec(&restored).unwrap(), serde_json::to_vec(&live).unwrap());
    }
    assert!(saw_present && saw_absent);
}
