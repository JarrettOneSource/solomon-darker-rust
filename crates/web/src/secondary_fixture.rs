//! Development-only GPU acceptance: replay captured source actors in an isolated canvas.
//! Normal builds do not expose this fixture hook or include the actor JSON reader.

use crate::courtyard::Camera;
use crate::loot::LootView;
use crate::secondary::{SecondaryPuppetHit, SecondaryView};
use crate::spells::SpellView;
use serde_json::{Value, json};
use solomon_graphics::Renderer;
use solomon_sim::boneyard::loot::{BoneyardGoodieState, BoneyardLootActor, BoneyardLootStore};
use solomon_sim::enemies::puppet_hits::EnemyPuppetHit;
use solomon_sim::hub::painter::hub_painter_order;
use solomon_sim::painter::{PainterRegistration, WorldManagerLane};
use solomon_sim::secondary::golem::{NativeGolemPhase, NativeSecondaryGolemState};
use solomon_sim::secondary::{NativeSecondaryActorKind, NativeSecondaryActorState};
use solomon_sim::spells::model::PrimarySpellTransientBase;
use solomon_sim::spells::staff::*;
use solomon_sim::{NativeRng, Vec2};
use std::cell::RefCell;
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{HtmlCanvasElement, WebGl2RenderingContext};

struct FixtureView {
    canvas: HtmlCanvasElement,
    gl: WebGl2RenderingContext,
    renderer: Renderer,
    secondary: SecondaryView,
    spells: SpellView,
    loot: LootView,
}
thread_local! {
    static FIXTURE: RefCell<Option<FixtureView>> = const { RefCell::new(None) };
}

#[wasm_bindgen]
pub async fn secondary_fixture_init() -> Result<(), JsValue> {
    let document = web_sys::window().unwrap().document().unwrap();
    let canvas: HtmlCanvasElement = document.create_element("canvas")?.dyn_into()?;
    canvas.set_id("secondary-fixture");
    canvas.set_attribute("style", "position:fixed;left:0;top:0;z-index:1000000")?;
    canvas.set_width(640);
    canvas.set_height(480);
    document.body().unwrap().append_child(&canvas)?;
    let options = js_sys::Object::new();
    // Match the actual game context; retain its pixels only for this readback fixture.
    for (key, value) in [
        ("alpha", false),
        ("antialias", false),
        ("premultipliedAlpha", false),
        ("preserveDrawingBuffer", true),
    ] {
        js_sys::Reflect::set(&options, &key.into(), &JsValue::from_bool(value))?;
    }
    let gl: WebGl2RenderingContext =
        canvas.get_context_with_context_options("webgl2", &options)?.unwrap().dyn_into()?;
    let renderer = Renderer::new(gl.clone()).map_err(|error| JsValue::from_str(&error))?;
    let spells = SpellView::load(&renderer).await.map_err(|error| JsValue::from_str(&error))?;
    let secondary =
        SecondaryView::load(&renderer, &spells).await.map_err(|error| JsValue::from_str(&error))?;
    FIXTURE.with(|fixture| {
        *fixture.borrow_mut() = Some(FixtureView {
            canvas,
            gl,
            renderer,
            secondary,
            spells,
            loot: LootView::default(),
        });
    });
    Ok(())
}

#[wasm_bindgen]
pub fn render_secondary_fixture(input: &str) -> Result<String, JsValue> {
    let input: Value =
        serde_json::from_str(input).map_err(|error| JsValue::from_str(&error.to_string()))?;
    FIXTURE.with(|fixture| {
        let mut fixture = fixture.borrow_mut();
        let view = fixture.as_mut().expect("secondary_fixture_init completes before rendering");
        let actors = input["actors"].as_array().unwrap().iter().map(read_actor).collect::<Vec<_>>();
        let staff = input["staff"]
            .as_array()
            .map(|rows| rows.iter().map(read_staff).collect::<Vec<_>>())
            .unwrap_or_default();
        let width = input["width"].as_u64().unwrap() as u32;
        let height = input["height"].as_u64().unwrap() as u32;
        view.canvas.set_width(width);
        view.canvas.set_height(height);
        view.renderer.set_native_world_pixel_centers(
            input["nativeWorldPixelCenters"].as_bool().unwrap_or(false),
        );
        view.renderer.begin_frame(width as i32, height as i32, [width as f32, height as f32]);
        view.renderer.set_saturation(input["saturation"].as_f64().unwrap() as f32);
        // Source clears the target rather than drawing a background quad. A quad
        // would inherit native pixel centers and leave a half-covered top/left edge.
        view.gl.clear_color(31.0 / 255.0, 31.0 / 255.0, 31.0 / 255.0, 1.0);
        view.gl.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
        view.secondary.memory = Default::default();
        let point_gain = input["pointGain"].as_f64().unwrap();
        view.secondary
            .prepare(
                &mut view.renderer,
                &actors,
                input["worldKey"].as_str().unwrap(),
                input["frame"].as_f64(),
                None,
                &|_| point_gain,
                input["enhancedEffects"].as_bool().unwrap_or(true),
            )
            .map_err(|error| JsValue::from_str(&error))?;
        let mut hits = std::collections::BTreeMap::new();
        if let Some(hit) = input["hit"].as_object() {
            hits.insert(
                hit["id"].as_u64().unwrap(),
                SecondaryPuppetHit {
                    alpha: hit["alpha"].as_f64().unwrap(),
                    tint: hit["tint"].as_u64().unwrap() as u32,
                },
            );
        }
        view.secondary
            .prepare_puppet_hits(&mut view.renderer, &hits)
            .map_err(|error| JsValue::from_str(&error))?;
        let camera =
            Camera { origin: point(&input["origin"]), scale: input["scale"].as_f64().unwrap() };
        view.secondary.draw_pre_world(&mut view.renderer, &camera);
        let mut layers =
            view.secondary.painters().map(|(_, painter)| painter.layer).collect::<Vec<_>>();
        let secondary_count = layers.len();
        for actor in &staff {
            layers.push(
                SecondaryView::staff_painter(actor)
                    .expect("fixture admits visible staff rows")
                    .layer,
            );
        }
        let world_tint = input["worldTint"].as_u64().unwrap_or(0xffffff) as u32;
        for index in hub_painter_order(&layers, input["referenceY"].as_f64().unwrap()) {
            if index < secondary_count {
                view.secondary.draw(&mut view.renderer, &camera, index, [1.0; 3]);
            } else {
                view.secondary.draw_staff_transient(
                    &mut view.renderer,
                    &camera,
                    &staff[index - secondary_count],
                    world_tint,
                );
            }
        }
        view.renderer.end_frame();
        Ok(json!({ "painters": layers.len(), "actors": actors.len() + staff.len() }).to_string())
    })
}

fn point(value: &Value) -> Vec2 {
    Vec2::new(value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}
fn read_rng(value: &Value) -> NativeRng {
    let mut rng = NativeRng::new(0);
    rng.index_a = value["indexA"].as_u64().unwrap() as usize;
    rng.index_b = value["indexB"].as_u64().unwrap() as usize;
    for (word, value) in rng.words.iter_mut().zip(value["words"].as_array().unwrap()) {
        *word = value.as_u64().unwrap() as u32;
    }
    rng
}
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
        value["skillId"].as_u64().map(|id| id as usize),
    );
    actor.age_ticks = value["ageTicks"].as_u64().unwrap();
    actor.alpha = value["alpha"].as_f64().unwrap();
    actor.damage = value["damage"].as_f64().unwrap();
    actor.enhanced = value["enhanced"].as_bool().unwrap();
    actor.endpoint = point(&value["endpoint"]);
    actor.frame = value["frame"].as_f64().unwrap();
    actor.freeze_ticks = value["freezeTicks"].as_u64().unwrap();
    actor.golem = value["golem"].as_object().map(|_| golem(&value["golem"]));
    actor.hit_target_ids =
        value["hitTargetIds"].as_array().unwrap().iter().map(|id| id.as_u64().unwrap()).collect();
    actor.lifetime_ticks = value["lifetimeTicks"].as_u64().unwrap();
    actor.midpoint = point(&value["midpoint"]);
    actor.phase = value["phase"].as_f64().unwrap();
    actor.position = point(&value["position"]);
    actor.presentation_rng =
        value["presentationRng"].as_object().map(|_| read_rng(&value["presentationRng"]));
    actor.quantity = value["quantity"].as_f64().unwrap();
    actor.radius = value["radius"].as_f64().unwrap();
    actor.rank = value["rank"].as_u64().unwrap() as u32;
    actor.rotation_radians = value["rotationRadians"].as_f64().unwrap();
    actor.scale = value["scale"].as_f64().unwrap();
    actor.slow_factor = value["slowFactor"].as_f64().unwrap();
    actor.target_id = value["targetId"].as_u64().map(|id| id as usize);
    actor.variant = value["variant"].as_i64().unwrap() as i32;
    actor.velocity = point(&value["velocity"]);
    actor.painter_registrations = registrations(value);
    actor
}

fn registrations(value: &Value) -> Vec<PainterRegistration> {
    value["painterRegistrations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|registration| PainterRegistration {
            manager_lane: match registration["managerLane"].as_str().unwrap() {
                "actor" => WorldManagerLane::Actor,
                "transient" => WorldManagerLane::Transient,
                other => panic!("unknown painter lane {other}"),
            },
            registration_ordinal: registration["registrationOrdinal"].as_u64().unwrap() as usize,
        })
        .collect()
}

fn read_staff(value: &Value) -> NativePlayerStaffTransient {
    let common = PrimarySpellTransientBase {
        age_ticks: value["ageTicks"].as_u64().unwrap(),
        id: value["id"].as_u64().unwrap(),
        light_registration: None,
        owner_id: value["ownerId"].as_str().unwrap().into(),
        painter_registrations: registrations(value),
        world_key: value["worldKey"].as_str().unwrap().into(),
    };
    let position = point(&value["position"]);
    if value["kind"] == "player-staff-pike-break" {
        return NativePlayerStaffTransient::PikeBreak(NativePlayerStaffPikeBreakVfx {
            common,
            enhanced_shadow: value["enhancedShadow"].as_bool().unwrap_or(true),
            heading_degrees: value["headingDegrees"].as_f64().unwrap(),
            position,
            presentation_rng: read_rng(&value["presentationRng"]),
            target_id: value["targetId"].as_str().unwrap().into(),
        });
    }
    let alpha = value["alpha"].as_f64().unwrap();
    let alpha_loss = value["alphaLoss"].as_f64().unwrap();
    let entry = value["entry"].as_u64().unwrap() as usize;
    let rotation_degrees = value["rotationDegrees"].as_f64().unwrap();
    let scale = value["scale"].as_f64().unwrap();
    NativePlayerStaffTransient::Vfx(match value["kind"].as_str().unwrap() {
        "player-staff-smoke" => NativePlayerStaffVfx::Smoke(NativePlayerStaffSmokeVfx {
            common,
            alpha,
            alpha_loss,
            entry,
            position,
            rotation_degrees,
            scale,
            angular_velocity_degrees: value["angularVelocityDegrees"].as_f64().unwrap(),
        }),
        "player-staff-move-fade" => NativePlayerStaffVfx::MoveFade(NativePlayerStaffMoveFadeVfx {
            common,
            alpha,
            alpha_loss,
            entry,
            position,
            rotation_degrees,
            scale,
            tint: value["tint"].as_u64().unwrap() as u32,
            velocity: point(&value["velocity"]),
            velocity_factor: value["velocityFactor"].as_f64().unwrap(),
        }),
        "player-staff-perspective-fade" => {
            NativePlayerStaffVfx::PerspectiveFade(NativePlayerStaffPerspectiveVfx {
                common,
                alpha,
                alpha_loss,
                entry,
                position,
                rotation_degrees,
                scale,
                tint: value["tint"].as_u64().unwrap() as u32,
            })
        }
        other => panic!("unexpected visible staff row {other}"),
    })
}

fn golem(value: &Value) -> NativeSecondaryGolemState {
    NativeSecondaryGolemState {
        action_heading_offset_degrees: value["actionHeadingOffsetDegrees"].as_f64().unwrap(),
        action_duration_ticks: value["actionDurationTicks"].as_u64().unwrap(),
        action_tick: value["actionTick"].as_u64().unwrap(),
        circle_slow_ticks: value["circleSlowTicks"].as_u64().unwrap(),
        current_health: value["currentHealth"].as_f64().unwrap(),
        damage_maximum: value["damageMaximum"].as_f64().unwrap(),
        iron: value["iron"].as_bool().unwrap(),
        gait_tick: value["gaitTick"].as_u64().unwrap(),
        left_connector_offset: point(&value["leftConnectorOffset"]),
        left_foot: point(&value["leftFoot"]),
        left_foot_bob: point(&value["leftFootBob"]),
        left_foot_next: point(&value["leftFootNext"]),
        left_foot_previous: point(&value["leftFootPrevious"]),
        left_foot_progress: value["leftFootProgress"].as_f64().unwrap(),
        left_foot_rotation_degrees: value["leftFootRotationDegrees"].as_f64().unwrap(),
        left_limb_mode: value["leftLimbMode"].as_u64().unwrap() as u32,
        maximum_health: value["maximumHealth"].as_f64().unwrap(),
        orbit_direction: value["orbitDirection"].as_f64().unwrap(),
        orbit_heading_radians: value["orbitHeadingRadians"].as_f64(),
        phase: match value["phase"].as_str().unwrap() {
            "active" => NativeGolemPhase::Active,
            "assembly" => NativeGolemPhase::Assembly,
            "attack" => NativeGolemPhase::Attack,
            "provoke" => NativeGolemPhase::Provoke,
            other => panic!("unknown phase {other}"),
        },
        pose_variant: value["poseVariant"].as_u64().unwrap() as u32,
        provoke_roll_bound: value["provokeRollBound"].as_i64().unwrap() as i32,
        reflect_factor: value["reflectFactor"].as_f64().unwrap(),
        right_connector_offset: point(&value["rightConnectorOffset"]),
        right_foot: point(&value["rightFoot"]),
        right_foot_bob: point(&value["rightFootBob"]),
        right_foot_next: point(&value["rightFootNext"]),
        right_foot_previous: point(&value["rightFootPrevious"]),
        right_foot_progress: value["rightFootProgress"].as_f64().unwrap(),
        right_foot_rotation_degrees: value["rightFootRotationDegrees"].as_f64().unwrap(),
        right_limb_mode: value["rightLimbMode"].as_u64().unwrap() as u32,
        target_poll_ticks_remaining: value["targetPollTicksRemaining"].as_i64().unwrap() as i32,
    }
}

/// Accepts the canonical persisted loot rows so the fixture cannot invent a
/// second item or store model. Source snapshot conversion stays in the harness.
#[wasm_bindgen]
pub fn render_loot_fixture(input: &str) -> Result<String, JsValue> {
    let input: Value =
        serde_json::from_str(input).map_err(|error| JsValue::from_str(&error.to_string()))?;
    let actors: Vec<BoneyardLootActor> = serde_json::from_value(input["lootActors"].clone())
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let goodies: Vec<BoneyardGoodieState> = serde_json::from_value(input["goodies"].clone())
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let hits: Vec<EnemyPuppetHit> = serde_json::from_value(input["hits"].clone())
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    FIXTURE.with(|fixture| {
        let mut fixture = fixture.borrow_mut();
        let view = fixture.as_mut().expect("secondary_fixture_init completes before rendering");
        let width = input["width"].as_u64().unwrap() as u32;
        let height = input["height"].as_u64().unwrap() as u32;
        view.canvas.set_width(width);
        view.canvas.set_height(height);
        view.renderer.begin_frame(width as i32, height as i32, [width as f32, height as f32]);
        view.renderer.set_saturation(input["saturation"].as_f64().unwrap() as f32);
        view.renderer.draw_solid(
            [0.0, 0.0, width as f32, height as f32],
            [31.0 / 255.0, 31.0 / 255.0, 31.0 / 255.0, 1.0],
        );
        let mut store = BoneyardLootStore::new("fixture", &[]);
        store.actors = actors;
        store.goodies = goodies;
        if input["reset"].as_bool().unwrap() {
            view.loot.reset();
        }
        view.loot
            .prepare(
                &store,
                input["tick"].as_f64().unwrap(),
                &hits,
                None,
                input["complexLighting"].as_bool().unwrap(),
                &[],
                None,
            )
            .map_err(|error| JsValue::from_str(&error))?;
        let camera =
            Camera { origin: point(&input["origin"]), scale: input["scale"].as_f64().unwrap() };
        let layers = view.loot.painters().map(|(_, painter)| painter.layer).collect::<Vec<_>>();
        let tint = input["worldTint"].as_u64().unwrap() as u32;
        let tint = [
            ((tint >> 16) & 255) as f32 / 255.0,
            ((tint >> 8) & 255) as f32 / 255.0,
            (tint & 255) as f32 / 255.0,
        ];
        for index in hub_painter_order(&layers, input["referenceY"].as_f64().unwrap()) {
            view.loot.draw(&mut view.renderer, &camera, &view.secondary, index, tint);
        }
        view.renderer.end_frame();
        Ok(json!({ "painters": layers.len(), "actors": store.actors.len() + store.goodies.len() })
            .to_string())
    })
}

/// Uses the real SpellView and canonical persistent actors for isolated GPU comparison.
#[wasm_bindgen]
pub fn render_harden_fixture(input: &str) -> Result<String, JsValue> {
    let input: Value =
        serde_json::from_str(input).map_err(|error| JsValue::from_str(&error.to_string()))?;
    let spells: solomon_sim::spells::model::SpellState =
        serde_json::from_value(input["spells"].clone())
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let puppet_hits: Vec<EnemyPuppetHit> = match input.get("puppetHits") {
        Some(rows) => serde_json::from_value(rows.clone())
            .map_err(|error| JsValue::from_str(&error.to_string()))?,
        None => Vec::new(),
    };
    FIXTURE.with(|fixture| {
        let mut fixture = fixture.borrow_mut();
        let view = fixture.as_mut().expect("secondary_fixture_init completes before rendering");
        let width = input["width"].as_u64().unwrap() as u32;
        let height = input["height"].as_u64().unwrap() as u32;
        view.canvas.set_width(width);
        view.canvas.set_height(height);
        view.renderer.begin_frame(width as i32, height as i32, [width as f32, height as f32]);
        view.renderer.set_saturation(input["saturation"].as_f64().unwrap() as f32);
        view.renderer.draw_solid(
            [0.0, 0.0, width as f32, height as f32],
            [31.0 / 255.0, 31.0 / 255.0, 31.0 / 255.0, 1.0],
        );
        let world_key =
            spells.transients.first().map_or("", |effect| effect.common().world_key.as_str());
        view.spells.prepare(&spells, world_key, 0, &|_| 1.0, &puppet_hits, None, true, None);
        let camera =
            Camera { origin: point(&input["origin"]), scale: input["scale"].as_f64().unwrap() };
        use solomon_sim::spells::presentation::SpellDrawQueue;
        view.spells.draw_queue(&mut view.renderer, &camera, SpellDrawQueue::PreWorld);
        let layers = view.spells.painters().map(|(_, painter)| painter.layer).collect::<Vec<_>>();
        for index in hub_painter_order(&layers, input["referenceY"].as_f64().unwrap()) {
            view.spells.draw(&mut view.renderer, &camera, index, [1.0; 3]);
        }
        view.spells.draw_queue(&mut view.renderer, &camera, SpellDrawQueue::PostWorld);
        view.renderer.end_frame();
        Ok(json!({ "actors": spells.transients.len(), "worldPainters": layers.len() }).to_string())
    })
}
