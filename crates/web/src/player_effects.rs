//! Source-owned player death views and level-up world sparkles. The borrowed rows
//! work for local and remote participants; only cosmetic view lifetime is retained.
//! References: `renderer/player-death-{burst,weapon}-view.ts`,
//! `world-player-view.ts::updateDeathLayers`, and `level-up-world-view.ts`.

use solomon_sim::float;
use std::collections::{BTreeMap, BTreeSet};

use solomon_sim::Vec2;
use solomon_sim::appearance::equipment::*;
use solomon_sim::death_animations::NativeDeathWeaponActor;
use solomon_sim::hub::{HubWizard, painter::PainterLayer};
use solomon_sim::player::PlayerState;
use solomon_sim::player_presentation::*;
use solomon_sim::presentation::multiply_native_tints;
use solomon_sim::profile::WizardElement;
use solomon_sim::spells::casting::NativePlayerWeaponKind;
use solomon_sim::spells::presentation::{NativeSpellSprite, NativeSpellSpritePlan, SpellBlend};

use crate::courtyard::Camera;
use crate::spells::SpellView;
use solomon_graphics::assets;
use solomon_graphics::{BlendMode, Quad, Renderer, Texture};

pub struct PlayerEffectPlayer<'a> {
    pub id: &'a str,
    pub player: &'a PlayerState,
    pub wizard: &'a HubWizard,
    pub material_tint: u32,
    pub level_up: Option<&'a NativeLevelUpPresentationFrame>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerEffectKind {
    DeathWeapon,
    DeathBurst,
    LevelUp,
}

pub struct PlayerEffectPainter {
    pub kind: PlayerEffectKind,
    pub owner_id: String,
    /// Weapons have their own source registration. Bursts and level-up particles
    /// follow their owner's assigned depth by 0 and 0.1 respectively.
    pub layer: Option<PainterLayer>,
    pub owner_depth_offset: f64,
    pub region_light_point: Option<Vec2>,
}

struct ActiveDeathBurst {
    started_tick: f64,
    trigger: PlayerDeathBurstTrigger,
}

struct DeathBodyFrame {
    position: Vec2,
    heading: usize,
    facing: usize,
    frame: usize,
    appearance: PlayerDeathEquipmentAppearance,
    element: WizardElement,
    material_tint: u32,
}

enum PlayerEffectDraw {
    Weapon {
        position: Vec2,
        height: f64,
        rotation_radians: f64,
        alpha: f64,
        weapon: PlayerWeaponAppearance,
    },
    Sprites(NativeSpellSpritePlan),
}

pub struct PlayerEffectsView {
    textures: BTreeMap<String, Texture>,
    run_id: Option<String>,
    tracker: Option<PlayerDeathBurstCrossingTracker>,
    bursts: Vec<ActiveDeathBurst>,
    bodies: BTreeMap<String, DeathBodyFrame>,
    hidden_players: BTreeSet<String>,
    painters: Vec<PlayerEffectPainter>,
    draws: Vec<PlayerEffectDraw>,
}

impl PlayerEffectsView {
    pub async fn load(renderer: &Renderer) -> Result<Self, String> {
        let mut names = Vec::new();
        for element in WizardElement::ALL {
            names.push(format!("player-character-death-{}", element.as_str()));
        }
        for selector in 0..4 {
            names.push(format!("player-character-death-hat-primary-{selector}"));
            names.push(format!("player-character-death-hat-secondary-{selector}"));
        }
        for selector in 0..3 {
            names.push(format!("player-character-death-robe-primary-{selector}"));
            names.push(format!("player-character-death-robe-secondary-{selector}"));
        }
        for suffix in [
            "hat-special-primary",
            "hat-special-secondary",
            "robe-fixed-primary-a",
            "robe-fixed-primary-b",
            "robe-fixed-secondary-a",
            "robe-fixed-secondary-b",
            "wand",
        ] {
            names.push(format!("player-character-death-{suffix}"));
        }
        for selector in 0..6 {
            names.push(format!("player-character-death-staff-{selector}"));
        }
        let mut textures = BTreeMap::new();
        for name in names {
            let image = assets::load_image(&format!("assets/stock/{name}.png")).await?;
            textures.insert(name, renderer.upload(&image)?);
        }
        Ok(Self {
            textures,
            run_id: None,
            tracker: None,
            bursts: Vec::new(),
            bodies: BTreeMap::new(),
            hidden_players: BTreeSet::new(),
            painters: Vec::new(),
            draws: Vec::new(),
        })
    }

    pub fn reset(&mut self) {
        self.run_id = None;
        self.tracker = None;
        self.bursts.clear();
        self.bodies.clear();
        self.hidden_players.clear();
        self.painters.clear();
        self.draws.clear();
    }

    pub fn prepare(
        &mut self,
        run_id: &str,
        tick: f64,
        players: &[PlayerEffectPlayer<'_>],
        weapons: &[NativeDeathWeaponActor],
    ) {
        if self.run_id.as_deref() != Some(run_id) {
            self.reset();
            self.run_id = Some(run_id.to_owned());
        }
        let observations: Vec<_> = players
            .iter()
            .map(|row| PlayerDeathObservation {
                player_id: row.id,
                combat: &row.player.combat,
                position: row.wizard.position,
            })
            .collect();
        if let Some(tracker) = &mut self.tracker {
            for trigger in tracker.update(run_id, &observations) {
                self.bursts.push(ActiveDeathBurst { started_tick: tick, trigger });
            }
        } else {
            self.tracker = Some(PlayerDeathBurstCrossingTracker::new(run_id, &observations));
        }
        self.bursts.retain(|burst| tick - burst.started_tick < 10.0);
        self.bodies.clear();
        self.hidden_players.clear();
        self.painters.clear();
        self.draws.clear();
        for row in players {
            let combat = &row.player.combat;
            if combat.corpse_consumed {
                self.hidden_players.insert(row.id.to_owned());
            }
            if let Some(frame) = combat.death_frame() {
                let appearance = player_death_equipment_appearance(
                    row.player.selection.element,
                    &row.player.economy.equipment,
                );
                self.bodies.insert(
                    row.id.to_owned(),
                    DeathBodyFrame {
                        position: row.wizard.position,
                        heading: row.wizard.heading_index % 24,
                        facing: ((row.wizard.heading_index + 2) / 4) % 6,
                        frame,
                        appearance,
                        element: row.player.selection.element,
                        material_tint: row.material_tint,
                    },
                );
            }
            if let Some(frame) = row.level_up {
                let mut plan = empty_sprite_plan(row.wizard.position);
                for particle in &frame.particles {
                    plan.draws.push(NativeSpellSprite {
                        alpha: particle.alpha,
                        blend: SpellBlend::Add,
                        offset: Vec2::new(particle.offset_x, particle.offset_y),
                        rotation: particle.rotation_radians,
                        ..NativeSpellSprite::new(73, particle.scale)
                    });
                }
                self.painters.push(PlayerEffectPainter {
                    kind: PlayerEffectKind::LevelUp,
                    owner_id: row.id.to_owned(),
                    layer: None,
                    owner_depth_offset: 0.1,
                    region_light_point: None,
                });
                self.draws.push(PlayerEffectDraw::Sprites(plan));
            }
        }
        // Current Source samples these retained actors from one selected snapshot;
        // it never reconstructs motion from the corpse age or requires a live owner.
        for actor in weapons {
            let position = actor.motion.position;
            self.painters.push(PlayerEffectPainter {
                kind: PlayerEffectKind::DeathWeapon,
                owner_id: actor.owner_id.clone(),
                layer: Some(actor.painter_registration.layer(position.y, 0.0)),
                owner_depth_offset: 0.0,
                region_light_point: Some(position),
            });
            self.draws.push(PlayerEffectDraw::Weapon {
                position,
                height: actor.motion.height,
                rotation_radians: actor.motion.rotation_degrees * std::f64::consts::PI / 180.0,
                alpha: actor.life.min(1.0),
                weapon: actor.weapon,
            });
        }
        for burst in &self.bursts {
            let mut plan = empty_sprite_plan(burst.trigger.position);
            for layer in player_death_burst_layers(&burst.trigger, tick - burst.started_tick) {
                plan.draws.push(NativeSpellSprite {
                    alpha: layer.alpha,
                    blend: SpellBlend::Add,
                    offset: layer.offset,
                    scale: layer.scale,
                    tint: 0x808080,
                    ..NativeSpellSprite::new(10, 1.0)
                });
            }
            self.painters.push(PlayerEffectPainter {
                kind: PlayerEffectKind::DeathBurst,
                owner_id: burst.trigger.player_id.clone(),
                layer: None,
                owner_depth_offset: 0.0,
                region_light_point: None,
            });
            self.draws.push(PlayerEffectDraw::Sprites(plan));
        }
    }

    pub fn painters(&self) -> impl Iterator<Item = (usize, &PlayerEffectPainter)> {
        self.painters.iter().enumerate()
    }

    /// Returns true when this participant uses the death body instead of living layers.
    pub fn draw_death_body(&self, renderer: &mut Renderer, camera: &Camera, id: &str) -> bool {
        if self.hidden_players.contains(id) {
            return true;
        }
        let Some(body) = self.bodies.get(id) else {
            return false;
        };
        self.draw_body_layers(renderer, camera, body);
        true
    }

    pub fn draw(
        &self,
        renderer: &mut Renderer,
        camera: &Camera,
        index: usize,
        spells: &SpellView,
        tint: u32,
    ) {
        match &self.draws[index] {
            PlayerEffectDraw::Sprites(plan) => spells.draw_sprite_plan(renderer, camera, plan),
            PlayerEffectDraw::Weapon {
                position,
                height: vertical_offset,
                rotation_radians,
                alpha,
                weapon,
            } => {
                let name = match weapon.kind {
                    NativePlayerWeaponKind::Staff => {
                        format!("player-character-death-staff-{}", weapon.selector)
                    }
                    NativePlayerWeaponKind::Wand => "player-character-death-wand".to_owned(),
                };
                let texture = &self.textures[&name];
                let width = f64::from(texture.width);
                let height = f64::from(texture.height);
                for shadow in [true, false] {
                    let vertical_scale = if shadow { 0.75 } else { 1.0 };
                    let offset_y = if shadow { 2.0 } else { *vertical_offset };
                    let (sin, cos) = float::sin_cos(*rotation_radians);
                    let corners = [
                        [-width * 0.5, -height * vertical_scale * 0.5],
                        [width * 0.5, -height * vertical_scale * 0.5],
                        [width * 0.5, height * vertical_scale * 0.5],
                        [-width * 0.5, height * vertical_scale * 0.5],
                    ]
                    .map(|[x, y]| {
                        let projected = camera.project(
                            position.x + x * cos - y * sin,
                            position.y + offset_y + x * sin + y * cos,
                            0.0,
                            0.0,
                        );
                        [projected[0], projected[1]]
                    });
                    let mut color = rgba(if shadow { 0 } else { tint });
                    color[3] = (alpha.clamp(0.0, 1.0) * 255.0).trunc() as f32 / 255.0;
                    renderer.draw_quad(
                        texture,
                        // Packed player composites use ordinary frame UVs;
                        // native stock-record half-texel insets do not apply.
                        [0.0, 0.0, texture.width, texture.height],
                        corners,
                        [color; 4],
                        BlendMode::Normal,
                    );
                }
            }
        }
    }

    fn draw_body_layers(&self, renderer: &mut Renderer, camera: &Camera, body: &DeathBodyFrame) {
        let appearance = body.appearance;
        let mut layers = Vec::with_capacity(9);
        for (suffix, tint) in [
            (format!("robe-primary-{}", appearance.robe.selector), appearance.robe.primary_tint),
            (
                format!("robe-secondary-{}", appearance.robe.selector),
                appearance.robe.secondary_tint,
            ),
            ("robe-fixed-primary-a".into(), appearance.robe.primary_tint),
            ("robe-fixed-primary-b".into(), appearance.robe.primary_tint),
            ("robe-fixed-secondary-a".into(), appearance.robe.secondary_tint),
            ("robe-fixed-secondary-b".into(), appearance.robe.secondary_tint),
            (body.element.as_str().into(), 0xffffff),
        ] {
            layers.push((suffix, body.frame, body.facing, Vec2::ZERO, tint));
        }
        let special = appearance.hat.selector == 3 && body.frame == 3;
        let hat_offset =
            if special { Vec2::ZERO } else { player_death_hat_anchor(body.frame, body.facing) };
        for (pass, tint) in
            [("primary", appearance.hat.primary_tint), ("secondary", appearance.hat.secondary_tint)]
        {
            layers.push((
                if special {
                    format!("hat-special-{pass}")
                } else {
                    format!("hat-{pass}-{}", appearance.hat.selector)
                },
                0,
                if special { body.facing } else { body.heading },
                hat_offset,
                tint,
            ));
        }
        for shadow in [true, false] {
            if shadow && body.frame != 3 {
                continue;
            }
            for (name, column, row, offset, tint) in &layers {
                let texture = &self.textures[&format!("player-character-death-{name}")];
                let tint =
                    if shadow { 0 } else { multiply_native_tints(*tint, body.material_tint) };
                renderer.draw(
                    texture,
                    Quad {
                        source: [*column as f32 * 170.0, *row as f32 * 170.0, 170.0, 170.0],
                        destination: camera.project(
                            body.position.x + offset.x - 85.0,
                            body.position.y + offset.y - 85.0 + if shadow { 4.0 } else { 0.0 },
                            170.0,
                            170.0,
                        ),
                        tint: rgba(tint),
                        blend: BlendMode::Normal,
                    },
                );
            }
        }
    }
}

fn empty_sprite_plan(position: Vec2) -> NativeSpellSpritePlan {
    NativeSpellSpritePlan {
        position,
        world_y: position.y,
        sort_bias: 0.0,
        region_light_point: None,
        draws: Vec::new(),
    }
}

fn rgba(color: u32) -> [f32; 4] {
    [
        ((color >> 16) & 0xff) as f32 / 255.0,
        ((color >> 8) & 0xff) as f32 / 255.0,
        (color & 0xff) as f32 / 255.0,
        1.0,
    ]
}
