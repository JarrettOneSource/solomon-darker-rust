//! Actual current primary entry-point emissions and the Hub post-contact factory.
use serde_json::{Value, json};
use solomon_sim::boneyard::BoneyardBounds;
use solomon_sim::hub::HubWizard;
use solomon_sim::painter::{PainterRegistration, WorldManagerLane, WorldPainterOrder};
use solomon_sim::spells::casting::NativePlayerWeaponKind;
use solomon_sim::spells::model::{
    PrimarySpellFlameLashPresentation, PrimarySpellTransient, SpellState,
};
use solomon_sim::spells::targeting::{
    PrimarySpellQueryLane, PrimarySpellTarget, PrimarySpellTargetKind,
};
use solomon_sim::spells::weld::{NativeWeldFlameLashFadeVariant, NativeWeldWorldActor};
use solomon_sim::spells::{PrimarySpellCastAuthority, PrimarySpellInput, PrimarySpellWorldContext};
use solomon_sim::{NativeRng, Vec2};
#[path = "support/website_rows.rs"]
mod website_rows;
#[path = "support/weld_profile.rs"]
mod weld_profile;
fn point(value: Vec2) -> Value {
    json!({"x":value.x,"y":value.y})
}
fn registration(value: PainterRegistration) -> Value {
    json!({"managerLane":match value.manager_lane{
    WorldManagerLane::Actor=>"actor",WorldManagerLane::Transient=>"transient"},"registrationOrdinal":value.registration_ordinal})
}
fn random(value: &NativeRng) -> Value {
    json!({"indexA":value.index_a,"indexB":value.index_b,"words":value.words.as_slice()})
}
fn presentation(value: &PrimarySpellFlameLashPresentation) -> Value {
    let geometry = value.native_geometry.unwrap();
    json!({"cameraDisplacement":value.camera_displacement.unwrap().map(point),"midpoint":value.midpoint.map(point),
        "targetId":value.target_id,"nativeGeometry":{"start":point(geometry.start),"control":point(geometry.control),
        "end":point(geometry.end),"offset":point(geometry.offset)},"preclipEndpoint":value.preclip_endpoint.map(point),
        "creatorRegistration":value.creator_registration.map(registration)})
}
#[test]
fn current_flame_cast_reserves_before_factory_and_clips_the_actual_view() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/website/flame-cast-goldens.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let profile = weld_profile::profile(&case["profile"]);
        let mut wizard = HubWizard::at(Vec2::new(100.0, 200.0));
        let mut spells = SpellState::default();
        let mut rng = NativeRng::new(731);
        let mut order = WorldPainterOrder { next_actor_ordinal: 49, next_transient_ordinal: 3 };
        let targets = if case["targets"].as_array().unwrap().is_empty() {
            Vec::new()
        } else {
            vec![PrimarySpellTarget {
                active: true,
                actor_flags: 2,
                attachment: Vec2::new(0.0, -3.0),
                body_radius: 15.0,
                cell_binding_order: 0,
                heading_degrees: Some(0.0),
                grid_position: None,
                id: "enemy:1".into(),
                kind: PrimarySpellTargetKind::Enemy,
                native_priority: 0,
                pending_remove: false,
                position: Vec2::new(450.0, 200.0),
                query_lane: PrimarySpellQueryLane::Grid,
                registration_order: 0,
            }]
        };
        let bounds = &case["bounds"];
        let view_bounds = if bounds.is_null() {
            None
        } else {
            Some(BoneyardBounds::new(
                bounds["x"].as_f64().unwrap(),
                bounds["y"].as_f64().unwrap(),
                bounds["w"].as_f64().unwrap(),
                bounds["h"].as_f64().unwrap(),
            ))
        };
        let enhanced_effects = case["enhancedEffects"].as_bool().unwrap();
        for tick in 0..=case["tick"].as_u64().unwrap() {
            let result = spells.step_primary(
                &mut wizard,
                PrimarySpellCastAuthority {
                    weapon_kind: Some(NativePlayerWeaponKind::Staff),
                    alive: true,
                    available_mana: case["mana"].as_f64().unwrap(),
                    cast_progress_factor: 1.0,
                    eligible: true,
                    plane_active: false,
                    primary_skill: &profile,
                },
                PrimarySpellInput { aim: Some(Vec2::new(700.0, 200.0)), primary: true },
                &PrimarySpellWorldContext {
                    tick,
                    enhanced_effects,
                    flame_lash_view_bounds: view_bounds,
                    player_light_registration: Some(PainterRegistration {
                        manager_lane: WorldManagerLane::Actor,
                        registration_ordinal: 48,
                    }),
                    view_scale: 1.2,
                    owner_id: "hero",
                    world_key: "hub:courtyard",
                    targets: &targets,
                    can_place: &|_, _, _| true,
                    can_traverse: &|_, _, _, _, _| true,
                    obstruction: &|_, _, _, _| None,
                    range_endpoint: &|start, direction, _| {
                        Vec2::new(start.x + direction.x * 600.0, start.y + direction.y * 600.0)
                    },
                    navigation: None,
                },
                &mut rng,
                Some(&mut order),
            );
            if result.channel_emissions.is_empty() {
                assert!(tick < case["tick"].as_u64().unwrap());
                continue;
            }
            assert_eq!(tick, case["tick"].as_u64().unwrap());
            assert_eq!(result.channel_emissions.len(), 1);
            website_rows::compare(
                &presentation(
                    result.channel_emissions[0].flame_lash_presentation.as_ref().unwrap(),
                ),
                &case["presentation"],
                "Flame retained cast geometry and normal-only camera sample",
            );
            website_rows::compare(
                &json!({"nextId":spells.next_id,"actors":spells.transients.len(),"rng":random(&rng)}),
                &case["before"],
                "Flame identity reservation before native birth",
            );
            spells
                .finalize_hub_flame_lash_visuals(
                    &result.channel_emissions,
                    tick,
                    &mut rng,
                    enhanced_effects,
                    &mut order,
                )
                .unwrap();
            let roots:Vec<_>=spells.transients.iter().map(|actor|{
                let PrimarySpellTransient::Weld(actor)=actor else{unreachable!()};
                let common=actor.common();
                let (kind,variant)=match actor{
                    NativeWeldWorldActor::Channel(channel)=>("weld-channel",json!(channel.variant)),
                    NativeWeldWorldActor::FlameLashFade(fade)=>("weld-flame-lash-fade",json!(match fade.variant{
                        NativeWeldFlameLashFadeVariant::Source=>"source",NativeWeldFlameLashFadeVariant::Endpoint=>"endpoint",
                        NativeWeldFlameLashFadeVariant::Chain=>"chain"})),_=>unreachable!(),
                };
                json!({"id":common.owner.id,"kind":kind,"variant":variant,"lightRegistration":common.owner.light_registration.map(registration),
                    "painterRegistrations":common.owner.painter_registrations.iter().copied().map(registration).collect::<Vec<_>>()})
            }).collect();
            website_rows::compare(
                &json!({"nextId":spells.next_id,"rng":random(&rng),"order":{"actor":order.next_actor_ordinal,
                "transient":order.next_transient_ordinal},"roots":roots}),
                &case["after"],
                "Flame endpoint/source/channel publication",
            );
        }
    }
}
