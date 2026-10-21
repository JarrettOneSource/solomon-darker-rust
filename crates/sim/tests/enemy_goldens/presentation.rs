//! Real actor-state projection and stock attachment draw-plan comparisons.

use super::*;
#[path = "sample_json.rs"]
mod sample_json;
use solomon_sim::boneyard::shadows::NativeShadowRecord;
use solomon_sim::enemies::actor_presentation::*;
use solomon_sim::enemies::attack_presentation::{
    EnemyRawFireBurstKind, native_enemy_raw_fire_burst_sample, native_imp_landing_flare_sample,
};
use solomon_sim::enemies::boss_presentation::native_boss_spell_plan;
use solomon_sim::enemies::death_presentation::native_enemy_death_effect_plan;
use solomon_sim::enemies::lightning_presentation::{
    native_dark_lightning_body, native_mage_lightning_pulse_plan,
};
use solomon_sim::enemies::model::{EnemyActor, EnemyMaggotActor};
use solomon_sim::enemies::projectile_presentation::native_enemy_projectile_effect_plan;
use solomon_sim::enemies::rain_presentation::NativeRainCloud;
use solomon_sim::enemies::tendrils::HeartmongerTendrils;
use solomon_sim::enemies::underlay::native_enemy_underlay_plan;
use solomon_sim::enemies::web_presentation::native_silk_mesh;
use solomon_sim::enemies::world::EnemyWorld;
use solomon_sim::spells::air::{NativeAirCoronaPlan, NativeAirRibbonLayer};
use std::collections::BTreeMap;

pub(super) struct EnemyPresentationGoldens {
    source: Value,
    animation_endpoints: BTreeMap<String, Value>,
    points: BTreeMap<String, Vec<Vec<solomon_sim::Vec2>>>,
    actor_index: BTreeMap<String, Vec<usize>>,
    maggot_index: BTreeMap<String, usize>,
    checked_actors: usize,
    checked_maggots: usize,
    effect_indices: [BTreeMap<String, usize>; 3],
    checked_effects: [usize; 3],
    mesh_indices: BTreeMap<String, Vec<usize>>,
    checked_meshes: usize,
    tendrils: BTreeMap<String, HeartmongerTendrils>,
    rain: BTreeMap<String, NativeRainCloud>,
}

fn key(group: &str, name: &str, tick: u64, id: usize) -> String {
    format!("{group}:{name}:{tick}:{id}")
}

impl EnemyArtPoints for EnemyPresentationGoldens {
    fn points(&self, atlas: EnemyArtAtlas, entry: usize) -> &[solomon_sim::Vec2] {
        &self.points[atlas.id()][entry]
    }
}

impl EnemyArtRecords for EnemyPresentationGoldens {
    fn record(&self, atlas: EnemyArtAtlas, entry: usize) -> EnemyArtRecord {
        let record = &self.source["artRecords"][atlas.id()][entry];
        EnemyArtRecord {
            anchor_x: number(record, "anchorX"),
            anchor_y: number(record, "anchorY"),
            width: number(record, "width"),
            height: number(record, "height"),
        }
    }
}

impl EnemyPresentationGoldens {
    pub(super) fn new() -> Self {
        let source: Value =
            serde_json::from_str(&fixture("enemy-presentation-goldens.json")).unwrap();
        let samples: Value = serde_json::from_str(&fixture("enemy-samples-goldens.json")).unwrap();
        let mut animation_endpoints = BTreeMap::new();
        for record in samples["endpoints"].as_array().unwrap() {
            let key = format!(
                "{}:{}",
                record["key"].as_str().unwrap(),
                record["hitOverride"].as_bool().unwrap()
            );
            animation_endpoints.insert(key, record["expected"].clone());
        }
        let mut points = BTreeMap::new();
        for (atlas, rows) in source["points"].as_object().unwrap() {
            let rows = rows
                .as_array()
                .unwrap()
                .iter()
                .map(|points| points.as_array().unwrap().iter().map(point_from_json).collect())
                .collect();
            points.insert(atlas.clone(), rows);
        }
        let mut actor_index: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (index, record) in source["actors"].as_array().unwrap().iter().enumerate() {
            actor_index
                .entry(key(
                    record["group"].as_str().unwrap(),
                    record["name"].as_str().unwrap(),
                    record["tick"].as_u64().unwrap(),
                    record["id"].as_u64().unwrap() as usize,
                ))
                .or_default()
                .push(index);
        }
        let mut maggot_index = BTreeMap::new();
        for (index, record) in source["maggots"].as_array().unwrap().iter().enumerate() {
            maggot_index.insert(
                key(
                    "ai",
                    record["name"].as_str().unwrap(),
                    record["tick"].as_u64().unwrap(),
                    record["id"].as_u64().unwrap() as usize,
                ),
                index,
            );
        }
        let mut effect_indices: [BTreeMap<String, usize>; 3] =
            std::array::from_fn(|_| BTreeMap::new());
        for (lane_index, lane) in ["projectile", "death", "boss"].into_iter().enumerate() {
            for (index, record) in source["effects"][lane].as_array().unwrap().iter().enumerate() {
                effect_indices[lane_index].insert(
                    key(
                        lane,
                        record["name"].as_str().unwrap(),
                        record["tick"].as_u64().unwrap(),
                        record["id"].as_u64().unwrap() as usize,
                    ),
                    index,
                );
            }
        }
        let mut mesh_indices: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (index, record) in source["meshes"].as_array().unwrap().iter().enumerate() {
            let kind = record["kind"].as_str().unwrap();
            let lane = if kind.starts_with("mage-") {
                "mage"
            } else if kind.starts_with("silk-") {
                "silk"
            } else {
                "dark"
            };
            mesh_indices
                .entry(key(
                    lane,
                    record["name"].as_str().unwrap(),
                    record["tick"].as_u64().unwrap(),
                    record["id"].as_u64().unwrap() as usize,
                ))
                .or_default()
                .push(index);
        }
        Self {
            source,
            animation_endpoints,
            points,
            actor_index,
            maggot_index,
            checked_actors: 0,
            checked_maggots: 0,
            effect_indices,
            checked_effects: [0; 3],
            mesh_indices,
            checked_meshes: 0,
            tendrils: BTreeMap::new(),
            rain: BTreeMap::new(),
        }
    }

    pub(super) fn check_actor(&mut self, group: &str, name: &str, tick: u64, actor: &EnemyActor) {
        let record_key = key(group, name, tick, actor.id);
        let Some(records) = self.actor_index.get(&record_key) else { return };
        for index in records {
            let record = &self.source["actors"][index];
            let mut actor = actor.clone();
            if record["hitOverride"].as_bool() == Some(true) {
                actor.hit_feedback.receive(tick, 1.0);
            }
            let input = &record["context"];
            let context = EnemyActorAppearanceContext {
                tick,
                application_tick: input["applicationTick"].as_u64().unwrap(),
                complex_lighting: input["complexLighting"].as_bool().unwrap(),
                spider_exposure: number(input, "spiderExposure"),
                spider_target: input["spiderTarget"].as_object().map(|_| {
                    EnemySpiderOutlineTarget {
                        position: point_from_json(&input["spiderTarget"]["position"]),
                        primary_tint: input["spiderTarget"]["primaryTint"].as_u64().unwrap() as u32,
                    }
                }),
                pike_target_position: input["pikeTargetPosition"]
                    .as_object()
                    .map(|_| point_from_json(&input["pikeTargetPosition"])),
            };
            let sample = native_enemy_presentation_input(&actor, context);
            let endpoint_key =
                format!("{}:{}", record_key, record["hitOverride"].as_bool().unwrap());
            assert_plan(
                &sample_json::animation_json(&sample),
                &self.animation_endpoints[&endpoint_key],
                &format!("{endpoint_key}.animation"),
            );
            let plan = native_enemy_actor_plan(&actor, context, self);
            let actual = json!({"facing":plan.facing,"rootScale":plan.root_scale,
                "layers":plan.layers.iter().map(layer_json).collect::<Vec<_>>(),
                "segments":plan.segments.iter().map(segment_json).collect::<Vec<_>>(),
                "spawnAgeTicks":plan.spawn_age_ticks});
            assert_plan(&actual, &record["expected"], &record_key);
            let origin = plan.attack_effect_origin.map(point_json);
            assert_plan(
                &json!(origin),
                &record["attackEffectOrigin"],
                &format!("{record_key}.attack-origin"),
            );
            let shadow_records = self.source["shadowRecords"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| NativeShadowRecord {
                    base_alpha: number(row, "baseAlpha"),
                    behind_scalar: number(row, "behindScalar"),
                    direction: point_from_json(&row["direction"]),
                    distance_fraction: number(row, "distanceFraction"),
                    projection_distance: number(row, "projectionDistance"),
                    source_position: point_from_json(&row["sourcePosition"]),
                    source_radius: number(row, "sourceRadius"),
                })
                .collect::<Vec<_>>();
            let underlay = native_enemy_underlay_plan(
                &actor,
                tick,
                &shadow_records,
                0.8,
                context.complex_lighting,
                self,
            );
            let actual = json!(
                underlay
                    .iter()
                    .map(|layer| json!({"alphas":layer.alphas,
                "atlas":layer.atlas.id(),"blendMode":blend(layer.blend),"entry":layer.entry,
                "role":layer.role,"tint":layer.tint,
                "vertices":layer.vertices.map(point_json)}))
                    .collect::<Vec<_>>()
            );
            assert_plan(&actual, &record["underlay"], &format!("{record_key}.underlay"));
            if let solomon_sim::enemies::model::EnemyBrain::Heartmonger(brain) = &actor.brain {
                let state_key = format!("{group}:{name}:{}", actor.id);
                let state = self.tendrils.entry(state_key).or_insert_with(|| {
                    let mut rng =
                        solomon_sim::NativeRng::new(actor.id as i64 + actor.spawn_tick as i64);
                    HeartmongerTendrils::new(actor.position, actor.heading_deg, &mut rng)
                });
                let facing =
                    (((actor.heading_deg + 10.0) / 20.0).trunc() as i64).rem_euclid(18) as usize;
                let entry = 200 + brain.leg_phase.trunc() as usize * 18 + facing;
                state.move_to(brain.leg_phase, actor.position, &self.points["Heartmonger"][entry]);
                let actual = json!(
                    state
                        .layers(actor.position, shadow_records[0].direction)
                        .iter()
                        .map(layer_json)
                        .collect::<Vec<_>>()
                );
                assert_plan(&actual, &record["tendrils"], &format!("{record_key}.tendrils"));
            }
            self.checked_actors += 1;
        }
    }

    pub(super) fn check_world(&mut self, name: &str, tick: u64, world: &EnemyWorld) {
        for actor in &world.actors {
            self.check_actor("ai", name, tick, actor);
        }
        for maggot in &world.maggots {
            self.check_maggot(name, tick, maggot);
        }
        for effect in &world.projectile_effects {
            let record_key = key("projectile", name, tick, effect.state.id);
            let Some(index) = self.effect_indices[0].get(&record_key) else { continue };
            let record = &self.source["effects"]["projectile"][index];
            let plan = native_enemy_projectile_effect_plan(effect, number(record, "pointGain"));
            let glow = plan.ground_glow.map(|glow| {
                json!({"alpha":glow.alpha,"position":point_json(glow.position),
                "scale":glow.scale,"tint":glow.tint})
            });
            let actual = json!({"groundGlow":glow,"position":point_json(plan.position),
                "layers":plan.layers.iter().map(layer_json).collect::<Vec<_>>()});
            assert_plan(&actual, &record["expected"], &record_key);
            self.checked_effects[0] += 1;
        }
        for effect in &world.death_effects {
            let record_key = key("death", name, tick, effect.id);
            let Some(index) = self.effect_indices[1].get(&record_key) else { continue };
            let record = &self.source["effects"]["death"][index];
            let plan = native_enemy_death_effect_plan(effect, 900.0);
            let actual = if matches!(
                effect.kind,
                solomon_sim::enemies::model::EnemyDeathEffectKind::Banish
                    | solomon_sim::enemies::model::EnemyDeathEffectKind::BanishBlack
            ) {
                json!({"gradients":plan.gradients.iter().map(gradient_json).collect::<Vec<_>>(),
                    "layers":plan.layers.iter().map(layer_json).collect::<Vec<_>>()})
            } else {
                let (main, preceding_layers) = plan.layers.split_last().unwrap();
                let shadow = preceding_layers.first().map(death_layer_json);
                json!({"effect":death_layer_json(main),"shadow":shadow,"position":point_json(effect.position)})
            };
            assert_plan(&actual, &record["expected"], &record_key);
            self.checked_effects[1] += 1;
        }
        for spell in &world.boss_spells {
            let rain = if matches!(
                spell.kind,
                solomon_sim::enemies::boss_spell::BossSpellKind::RainOfBones { .. }
            ) {
                let state = self
                    .rain
                    .entry(format!("{name}:{}", spell.owner.id))
                    .or_insert_with(|| NativeRainCloud::new(spell, tick));
                Some(state.update(spell, tick))
            } else {
                None
            };
            let record_key = key("boss", name, tick, spell.owner.id);
            let Some(index) = self.effect_indices[2].get(&record_key) else { continue };
            let record = &self.source["effects"]["boss"][index];
            let plan = native_boss_spell_plan(spell, tick, 900.0);
            let actual = json!({"gradients":plan.gradients.iter().map(gradient_json).collect::<Vec<_>>(),
                "layers":plan.layers.iter().map(layer_json).collect::<Vec<_>>()});
            assert_plan(&actual, &record["expected"], &record_key);
            if let Some(layers) = rain {
                let actual = json!(layers.iter().map(layer_json).collect::<Vec<_>>());
                assert_plan(&actual, &record["rain"], &format!("{record_key}.rain"));
            }
            self.checked_effects[2] += 1;
        }
        self.check_meshes(name, tick, world);
    }

    fn check_meshes(&mut self, name: &str, tick: u64, world: &EnemyWorld) {
        for pulse in &world.mage_lightning_pulses {
            let record_key = key("mage", name, tick, pulse.id);
            let Some(indices) = self.mesh_indices.get(&record_key) else { continue };
            for index in indices {
                let record = &self.source["meshes"][index];
                let plan = native_mage_lightning_pulse_plan(
                    pulse,
                    record["presentationTick"].as_u64().unwrap(),
                );
                let actual = plan.map_or(Value::Null,|plan| {
                    let corona = corona_json(plan.contact);
                    let contact = match plan.contact_target_player_id {
                        Some(id)=>json!({"corona":corona,"kind":"target-attached",
                            "localOffset":corona["center"],"targetPlayerId":id}),
                        None=>json!({"corona":corona,"kind":"world","position":corona["center"]}),
                    };
                    json!({"ageTicks":plan.age_ticks,"body":plan.body.map(|layers|
                        json!({"layers":layers.iter().map(ribbon_json).collect::<Vec<_>>()})),
                        "contact":contact,"endpoint":point_json(plan.endpoint),"midpoint":point_json(plan.midpoint),
                        "pathLights":plan.path_lights.iter().map(|light|json!({"castsDirectionalShadow":light.casts_directional_shadow,
                            "intensity":light.intensity,"position":point_json(light.position),"radius":light.radius})).collect::<Vec<_>>(),
                        "source":point_json(plan.source),"sourceCorona":plan.source_corona.map(corona_json)})
                });
                assert_plan(
                    &actual,
                    &record["expected"],
                    &format!("{record_key}.{}", record["kind"]),
                );
                self.checked_meshes += 1;
            }
        }
        for silk in &world.silks {
            let record_key = key("silk", name, tick, silk.id);
            let Some(indices) = self.mesh_indices.get(&record_key) else { continue };
            for index in indices {
                let record = &self.source["meshes"][index];
                let sample = record["presentationTick"].as_u64().unwrap();
                let mut rng = solomon_sim::NativeRng::new((silk.id as u64 ^ sample) as i64);
                let plan = native_silk_mesh(
                    &silk.state,
                    &|p| (p.x / 800.0 + p.y / 1600.0).clamp(0.0, 1.0),
                    &mut rng,
                );
                let actual = json!({"vertices":flat_points(&plan.mesh.vertices),"colors":plan.mesh.colors,
                    "indices":plan.mesh.indices,"segmentCount":plan.segment_count,"sparkleCount":plan.sparkle_count});
                assert_plan(&actual, &record["expected"], &record_key);
                self.checked_meshes += 1;
            }
        }
        for spell in &world.boss_spells {
            let record_key = key("dark", name, tick, spell.owner.id);
            let Some(indices) = self.mesh_indices.get(&record_key) else { continue };
            for index in indices {
                let record = &self.source["meshes"][index];
                let body = native_dark_lightning_body(spell);
                let actual = json!({"layers":body.iter().map(ribbon_json).collect::<Vec<_>>()});
                assert_plan(&actual, &record["expected"], &record_key);
                self.checked_meshes += 1;
            }
        }
    }

    fn check_maggot(&mut self, name: &str, tick: u64, maggot: &EnemyMaggotActor) {
        let record_key = key("ai", name, tick, maggot.id);
        let Some(index) = self.maggot_index.get(&record_key) else { return };
        let record = &self.source["maggots"][index];
        let layers = native_maggot_plan(maggot, tick, record["complexLighting"].as_bool().unwrap());
        let actual = json!(layers.iter().map(layer_json).collect::<Vec<_>>());
        assert_plan(&actual, &record["expected"], &record_key);
        self.checked_maggots += 1;
    }

    pub(super) fn assert_complete(&self, group: &str) {
        if group == "construction" {
            for sample in self.source["attackEffects"].as_array().unwrap() {
                let id = sample["eventId"].as_u64().unwrap() as usize;
                let age = sample["ageTicks"].as_u64().unwrap();
                let kind = sample["kind"].as_str().unwrap();
                let actual = if kind == "imp-landing-flare" {
                    native_imp_landing_flare_sample(id,age).map_or(Value::Null,|sample|json!({
                        "alpha":sample.alpha,"green":sample.green,"scaleX":sample.scale_x,"scaleY":sample.scale_y}))
                } else {
                    let kind = if kind == "imp-contact" {
                        EnemyRawFireBurstKind::ImpContact
                    } else {
                        EnemyRawFireBurstKind::DemonBombMuzzle
                    };
                    native_enemy_raw_fire_burst_sample(kind,id,age).map_or(Value::Null,|sample|json!({
                        "frameEntry":sample.frame_entry,"frameRotationRadians":sample.frame_rotation_radians,
                        "glowAlpha":sample.glow_alpha,"scale":sample.scale,"verticalOffset":sample.vertical_offset}))
                };
                assert_plan(
                    &actual,
                    &sample["expected"],
                    &format!("attack-effect:{kind}:{id}:{age}"),
                );
            }
        }
        let expected = self.source["actors"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|record| record["group"] == group)
            .count();
        assert_eq!(self.checked_actors, expected, "every {group} actor presentation was compared");
        if group == "ai" {
            assert_eq!(
                self.checked_meshes,
                self.source["meshes"].as_array().unwrap().len(),
                "every Mage, Dark Lightning and Silk mesh was compared"
            );
            assert_eq!(self.checked_maggots, self.source["maggots"].as_array().unwrap().len());
            for (index, lane) in ["projectile", "death", "boss"].into_iter().enumerate() {
                assert_eq!(
                    self.checked_effects[index],
                    self.source["effects"][lane].as_array().unwrap().len(),
                    "every {lane} presentation was compared"
                );
            }
        }
    }
}

fn death_layer_json(layer: &EnemySpriteLayer) -> Value {
    json!({"alpha":layer.alpha,"atlas":layer.atlas.id(),"blendMode":blend(layer.blend),
        "entry":layer.entry,"offset":point_json(layer.offset),"rotationRadians":layer.rotation_radians,
        "scale":{"x":layer.scale_x.unwrap_or(layer.scale),"y":layer.scale_y.unwrap_or(layer.scale)},"tint":layer.tint})
}

fn flat_points(points: &[solomon_sim::Vec2]) -> Vec<f64> {
    points.iter().flat_map(|point| [point.x, point.y]).collect()
}

fn ribbon_json(layer: &NativeAirRibbonLayer) -> Value {
    let branch = layer.branch.as_ref().map(|branch| {
        json!({"geometryRecord":branch.geometry_record,
        "indices":branch.mesh.indices,"mirrorX":branch.mirror_x,"scale":branch.scale,
        "textureRecord":branch.texture_record,"uvs":flat_points(&branch.mesh.uvs),
        "vertices":flat_points(&branch.mesh.vertices)})
    });
    let mut row = json!({"alpha":layer.alpha,"branch":branch,"indices":layer.mesh.indices,
        "parameterSamples":layer.parameter_samples,"phaseDegrees":layer.phase_degrees,
        "phaseOffset":layer.phase_offset,"textureRecord":layer.texture_record,"tint":layer.tint,
        "uvs":flat_points(&layer.mesh.uvs),"vertices":flat_points(&layer.mesh.vertices),"width":layer.width});
    if layer.texture_record == 64 {
        row["blendMode"] = json!("normal");
    }
    row
}

fn corona_json(corona: NativeAirCoronaPlan) -> Value {
    json!({"alpha":corona.alpha,"center":point_json(corona.center),
        "circles":corona.draws[..4].iter().map(|draw|json!({"alpha":draw.alpha,"record":draw.record,
            "scale":draw.scale.x,"tint":draw.tint})).collect::<Vec<_>>(),
        "forks":corona.draws[4..].iter().map(|draw|json!({"alpha":draw.alpha,"record":draw.record,
            "rotation":draw.rotation,"scale":draw.scale.x,"tint":draw.tint})).collect::<Vec<_>>()})
}

fn gradient_json(
    gradient: &solomon_sim::enemies::death_presentation::EnemyBanishGradient,
) -> Value {
    json!({"x":gradient.position.x,"y":gradient.position.y,"width":gradient.width,"height":gradient.height,
        "startColor":gradient.start_color,"endColor":gradient.end_color,
        "startAlpha":gradient.start_alpha,"endAlpha":gradient.end_alpha})
}

fn layer_json(layer: &EnemySpriteLayer) -> Value {
    let mut record = json!({"alpha":layer.alpha,"atlas":layer.atlas.id(),"blendMode":blend(layer.blend),
        "entry":layer.entry,"offset":point_json(layer.offset),"role":layer.role,
        "rotationRadians":layer.rotation_radians,"scale":layer.scale,"tint":layer.tint});
    if let Some(value) = layer.scale_x {
        record["scaleX"] = json!(value);
    }
    if let Some(value) = layer.scale_y {
        record["scaleY"] = json!(value);
    }
    if let Some(value) = layer.stretch {
        record["stretch"] = json!({"start":point_json(value.start),"end":point_json(value.end)});
    }
    if !layer.apply_vertical_offset {
        record["applyVerticalOffset"] = json!(false);
    }
    if layer.texture_diffuse {
        record["textureColor"] = json!("diffuse");
    }
    record
}
fn segment_json(segment: &EnemySegmentLayer) -> Value {
    json!({"alpha":segment.alpha,"beforeRole":segment.before_role,"blendMode":blend(segment.blend),
        "end":point_json(segment.end),"role":segment.role,"start":point_json(segment.start),"tint":segment.tint,"width":segment.width})
}
fn blend(blend: EnemyArtBlend) -> &'static str {
    match blend {
        EnemyArtBlend::Normal => "normal",
        EnemyArtBlend::Add => "add",
    }
}

fn assert_plan(actual: &Value, expected: &Value, label: &str) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            if [".entry", ".facing", ".spawnAgeTicks", ".tint"]
                .iter()
                .any(|key| label.ends_with(key))
            {
                assert_eq!(actual.as_f64(), expected.as_f64(), "{label}");
            } else {
                assert_trig(actual.as_f64().unwrap(), expected.as_f64().unwrap(), label);
            }
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{label} length");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                assert_plan(
                    actual,
                    expected,
                    &format!("{label}[{index}:{}]", expected["role"].as_str().unwrap_or("")),
                );
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{label} fields");
            for (key, value) in actual {
                assert_plan(value, &expected[key], &format!("{label}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{label}"),
    }
}
