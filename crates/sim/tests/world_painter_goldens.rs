use serde::Deserialize;
use solomon_sim::painter::{WorldManagerLane, WorldPainterOrder};

#[derive(Deserialize)]
struct Ordinals {
    actor: usize,
    transient: usize,
}

#[derive(Deserialize)]
struct Sample {
    lane: String,
    count: usize,
    ordinals: Vec<usize>,
    next: Ordinals,
}

#[derive(Deserialize)]
struct Goldens {
    initial: Ordinals,
    samples: Vec<Sample>,
}

#[test]
fn manager_lanes_allocate_independently_and_resume_from_reserved_roots() {
    let goldens: Goldens =
        serde_json::from_str(include_str!("fixtures/website/world-painter-goldens.json")).unwrap();
    let mut order = WorldPainterOrder {
        next_actor_ordinal: goldens.initial.actor,
        next_transient_ordinal: goldens.initial.transient,
    };
    for sample in goldens.samples {
        let lane = match sample.lane.as_str() {
            "actor" => WorldManagerLane::Actor,
            "transient" => WorldManagerLane::Transient,
            other => panic!("unknown world painter lane {other}"),
        };
        let registrations = order.register_many(lane, sample.count);
        let ordinals: Vec<_> = registrations.iter().map(|item| item.registration_ordinal).collect();
        assert_eq!(ordinals, sample.ordinals);
        assert!(registrations.iter().all(|item| item.manager_lane == lane));
        assert_eq!(order.next_actor_ordinal, sample.next.actor);
        assert_eq!(order.next_transient_ordinal, sample.next.transient);
    }
}
