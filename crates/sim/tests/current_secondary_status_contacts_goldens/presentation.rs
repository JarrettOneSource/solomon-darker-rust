//! Shared factory meshes, independent coronas, light lifetime and actual Pixi painter rows.
use super::{fixture, native_bits, support};
use serde_json::{Value, json};
use solomon_sim::Vec2;
use solomon_sim::boneyard::lighting::secondary::native_secondary_provider_light_source;
use solomon_sim::secondary::electric_burn::native_electric_burn_arc_plan;
use solomon_sim::secondary::presentation::*;
use solomon_sim::secondary::*;
use solomon_sim::spells::air::NativeAirRibbonLayer;

fn read_actor(value: &Value) -> NativeSecondaryActorState {
    let kind = NativeSecondaryActorKind::ALL
        .into_iter()
        .find(|kind| Some(kind.as_str()) == value["kind"].as_str())
        .unwrap();
    let mut actor = NativeSecondaryActorState::new(
        value["id"].as_u64().unwrap(),
        kind,
        value["ownerId"].as_str().unwrap().into(),
        value["worldKey"].as_str().unwrap().into(),
        None,
    );
    actor.age_ticks = value["ageTicks"].as_u64().unwrap();
    actor.alpha = value["alpha"].as_f64().unwrap();
    actor.enhanced = value["enhanced"].as_bool().unwrap();
    actor.endpoint = support::point(&value["endpoint"]);
    actor.midpoint = support::point(&value["midpoint"]);
    actor.phase = value["phase"].as_f64().unwrap();
    actor.position = support::point(&value["position"]);
    actor.radius = value["radius"].as_f64().unwrap();
    actor.rotation_radians = value["rotationRadians"].as_f64().unwrap();
    actor.scale = value["scale"].as_f64().unwrap();
    actor.painter_registrations = value["painterRegistrations"]
        .as_array()
        .unwrap()
        .iter()
        .map(support::registration)
        .collect();
    actor
}
fn flat(points: &[Vec2]) -> Vec<f64> {
    points.iter().flat_map(|point| [point.x, point.y]).collect()
}
fn ribbon(layer: &NativeAirRibbonLayer) -> Value {
    let branch=layer.branch.as_ref().map(|branch|json!({"geometryRecord":branch.geometry_record,
        "indices":branch.mesh.indices,"mirrorX":branch.mirror_x,"scale":branch.scale,
        "textureRecord":branch.texture_record,"uvs":flat(&branch.mesh.uvs),"vertices":flat(&branch.mesh.vertices)}));
    json!({"alpha":layer.alpha,"branch":branch,"indices":layer.mesh.indices,
        "parameterSamples":layer.parameter_samples,"phaseDegrees":layer.phase_degrees,
        "phaseOffset":layer.phase_offset,"textureRecord":layer.texture_record,"tint":layer.tint,
        "uvs":flat(&layer.mesh.uvs),"vertices":flat(&layer.mesh.vertices),"width":layer.width})
}
fn sprite(draw: &NativeSecondarySpriteDraw) -> Value {
    json!({"alpha":draw.alpha,"atlas":draw.atlas.as_str(),"blend":match draw.blend {
        NativeSecondaryBlend::Normal=>"normal",NativeSecondaryBlend::Add=>"add"},"entry":draw.entry,
        "offset":support::point_snapshot(draw.offset),"role":draw.role,
        "rotationRadians":draw.rotation_radians,"scaleX":draw.scale_x,"scaleY":draw.scale_y,"tint":draw.tint})
}

#[test]
fn electric_coronas_body_factory_and_light_plan_match_source() {
    for row in fixture()["presentations"].as_array().unwrap() {
        let actor = read_actor(&row["actor"]);
        let plan = native_secondary_presentation_plan(&actor, 0.5, 0.75);
        let body = (actor.kind == NativeSecondaryActorKind::ElectricBurnArc)
            .then(|| native_electric_burn_arc_plan(&actor, actor.age_ticks as f64, actor.enhanced))
            .flatten()
            .map(|plan| json!({"layers":plan.body.iter().map(ribbon).collect::<Vec<_>>()}));
        let light = native_secondary_provider_light_source(&actor, 5, true, 0.75).map(|light| {
            json!({
            "castsDirectionalShadow":light.casts_directional_shadow,"intensity":light.intensity,
            "position":support::point_snapshot(light.position),"radius":light.radius})
        });
        let actual = json!({"draws":plan.draws.iter().map(sprite).collect::<Vec<_>>(),
            "queueFamily":match plan.queue_family {NativeSecondaryQueueFamily::OrdinaryDynamic=>"ordinary-dynamic",NativeSecondaryQueueFamily::Zanim=>"zanim"},
            "root":support::point_snapshot(plan.root),"sortBias":plan.sort_bias,"worldY":plan.world_y,"body":body,
            "lightDisposition":match native_secondary_light_disposition(&actor) {
                NativeSecondaryLightDisposition::ActorProvider=>"actor-provider",NativeSecondaryLightDisposition::Misc=>"misc",
                NativeSecondaryLightDisposition::None=>"none",NativeSecondaryLightDisposition::TransientProvider=>"transient-provider"},
            "painterLane":match native_secondary_painter_manager_lane(actor.kind) {
                solomon_sim::painter::WorldManagerLane::Actor=>"actor",solomon_sim::painter::WorldManagerLane::Transient=>"transient"},"light":light});
        native_bits::compare(
            &actual,
            &row["expected"],
            &format!("{} {} age{}", actor.kind.as_str(), actor.id, actor.age_ticks),
        );
    }
}

#[test]
fn electric_arc_painters_match_actual_source_view_across_quality_and_retirement() {
    for row in fixture()["arcViews"].as_array().unwrap() {
        let actor = read_actor(&row["actor"]);
        let plan = native_electric_burn_arc_plan(
            &actor,
            actor.age_ticks as f64,
            row["enhancedEffects"].as_bool().unwrap(),
        );
        let mut painters = Vec::new();
        let mut primitive_count = 0;
        if let Some(plan) = plan {
            primitive_count = plan.bands.len();
            painters.push(json!({"id":format!("secondary:{}:body",actor.id),"lane":"world-sorted",
                "queueFamily":"ordinary-dynamic","regionLightPoint":null,
                "registration":support::registration_snapshot(actor.painter_registrations[0]),
                "sourceOrder":0,"sortBias":0,"visible":false,"worldY":actor.position.y+plan.bounds_y,
                "insertions":plan.bands.iter().enumerate().map(|(index,band)|json!({
                    "id":format!("secondary:{}:body-band-{index}",actor.id),"sortBias":0,"visible":true,
                    "worldY":actor.position.y+band.painter_y})).collect::<Vec<_>>()}));
        }
        native_bits::compare(
            &json!({"painters":painters,"primitiveCount":primitive_count}),
            &row["expected"],
            &format!("arc{} age{} quality{}", actor.id, actor.age_ticks, row["enhancedEffects"]),
        );
    }
}
