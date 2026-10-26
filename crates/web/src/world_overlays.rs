//! Source world nameplates, accepted-chat speech and Hub activity badges.
//! These screen-space layers follow the final world camera and precede screen feedback.

mod graphics;

use solomon_protocol::HubActivity;
use solomon_sim::Vec2;
use solomon_sim::native_ui_text::NativeUiGlyphLayout;
use solomon_sim::profile::WizardElement;
use solomon_sim::social::{
    GameChatMessage, GameWorldSpeech, append_game_world_speech, sample_game_world_speech_alpha,
};
use solomon_sim::world_labels::*;

use crate::courtyard::Camera;
use crate::native_mesh::native_sprite_frame_texel_rect;
use graphics::{
    chamfered_plate, circle, circle_stroke, diamond, polygon, rectangle, round_rectangle,
};
use solomon_graphics::assets;
use solomon_graphics::{BlendMode, Quad, Renderer, Texture};

pub struct WorldOverlayPlayer<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub element: WizardElement,
    pub position: Vec2,
    pub current_health: f64,
    pub maximum_health: f64,
    /// Accepted Hub activity; Boneyard rows have no badge.
    pub activity: Option<HubActivity>,
}

pub struct WorldOverlayFrame<'a> {
    /// Hub rooms share one source renderer; each Boneyard run owns another.
    pub world_key: &'a str,
    /// The caller applies current-world/materialization and Hub region visibility.
    pub players: &'a [WorldOverlayPlayer<'a>],
    pub local_player_id: &'a str,
    pub camera: &'a Camera,
    pub viewport: Vec2,
    pub now_ms: f64,
    pub renderable: bool,
}

#[derive(Default, serde::Serialize)]
pub struct WorldOverlayDiagnostics {
    pub activity_player_ids: Vec<String>,
    pub activity_states: Vec<HubActivity>,
    pub speech_active_count: usize,
    pub speech_player_ids: Vec<String>,
    pub speech_sequences: Vec<u64>,
    pub speech_alphas: Vec<f64>,
}

impl WorldOverlayDiagnostics {
    #[cfg(target_arch = "wasm32")]
    pub fn apply(&self, canvas: &web_sys::HtmlCanvasElement) -> Result<(), String> {
        let attributes = [
            ("data-hub-activity-count", self.activity_player_ids.len().to_string()),
            ("data-hub-activity-player-ids", self.activity_player_ids.join(",")),
            (
                "data-hub-activity-states",
                self.activity_states
                    .iter()
                    .map(|activity| match activity {
                        HubActivity::Paused => "paused",
                        HubActivity::Occupied => "occupied",
                    })
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            ("data-world-speech-active-count", self.speech_active_count.to_string()),
            ("data-world-speech-count", self.speech_player_ids.len().to_string()),
            ("data-world-speech-player-ids", self.speech_player_ids.join(",")),
            (
                "data-world-speech-sequences",
                self.speech_sequences.iter().map(u64::to_string).collect::<Vec<_>>().join(","),
            ),
            (
                "data-world-speech-alphas",
                self.speech_alphas.iter().map(f64::to_string).collect::<Vec<_>>().join(","),
            ),
            (
                "data-world-speech-maximum-alpha",
                self.speech_alphas.iter().copied().fold(0.0_f64, f64::max).to_string(),
            ),
        ];
        for (name, value) in attributes {
            canvas.set_attribute(name, &value).map_err(|_| format!("Could not set {name}"))?;
        }
        Ok(())
    }
}

struct NameplateView {
    id: String,
    name: String,
    layout: WorldNameplateVisualLayout,
    position: Option<Vec2>,
    health_ratio: f64,
    element: WizardElement,
}

struct ActivityView {
    id: String,
    position: Option<Vec2>,
    activity: HubActivity,
}

struct SpeechView {
    id: String,
    text: String,
    sequence: u64,
    layout: NativeWorldSpeechLayout,
    position: Option<Vec2>,
    alpha: f64,
}

pub struct WorldOverlaysView {
    font_atlas: Texture,
    world_key: Option<String>,
    speeches: Vec<GameWorldSpeech>,
    // Source containers retain their insertion order when new IDs arrive later.
    nameplates: Vec<NameplateView>,
    activities: Vec<ActivityView>,
    speech_views: Vec<SpeechView>,
}

impl WorldOverlaysView {
    pub async fn load(renderer: &Renderer) -> Result<Self, String> {
        let font_atlas = renderer.upload(&assets::load_image("assets/stock/Fonts.png").await?)?;
        Ok(Self {
            font_atlas,
            world_key: None,
            speeches: Vec::new(),
            nameplates: Vec::new(),
            activities: Vec::new(),
            speech_views: Vec::new(),
        })
    }

    /// MainMenuScene admits actual chat callbacks, excluding activity announcements.
    /// Replayed Social history intentionally never calls this method.
    pub fn consume_chat(&mut self, message: &GameChatMessage, received_at_ms: f64) {
        if message.activity.is_none() {
            let _ = append_game_world_speech(&mut self.speeches, message, received_at_ms);
        }
    }

    pub fn reset(&mut self) {
        self.world_key = None;
        self.speeches.clear();
        self.nameplates.clear();
        self.activities.clear();
        self.speech_views.clear();
    }

    pub fn draw(
        &mut self,
        renderer: &mut Renderer,
        frame: &WorldOverlayFrame<'_>,
    ) -> WorldOverlayDiagnostics {
        if self.world_key.as_deref() != Some(frame.world_key) {
            self.nameplates.clear();
            self.activities.clear();
            self.speech_views.clear();
            self.world_key = Some(frame.world_key.to_owned());
        }
        let mut players: Vec<_> = frame.players.iter().collect();
        players.sort_by(|left, right| left.id.cmp(right.id));
        let mut diagnostics = WorldOverlayDiagnostics::default();
        self.prepare_nameplates(&players, frame);
        self.prepare_activities(&players, frame, &mut diagnostics);
        self.prepare_speech(&players, frame, &mut diagnostics);
        renderer.set_saturation(1.0);
        for view in &self.nameplates {
            if let Some(position) = view.position {
                self.draw_nameplate(renderer, position, view);
            }
        }
        for view in &self.activities {
            if let Some(position) = view.position {
                draw_activity(renderer, position, view.activity);
            }
        }
        for view in &self.speech_views {
            if let Some(position) = view.position {
                self.draw_speech(renderer, position, &view.layout, view.alpha);
            }
        }
        diagnostics
    }

    fn prepare_nameplates(
        &mut self,
        players: &[&WorldOverlayPlayer<'_>],
        frame: &WorldOverlayFrame<'_>,
    ) {
        let mut live = Vec::new();
        for player in players {
            if player.id == frame.local_player_id
                || player.name.is_empty()
                || !player.position.x.is_finite()
                || !player.position.y.is_finite()
            {
                continue;
            }
            let Some(health_ratio) =
                native_world_nameplate_health_ratio(player.current_health, player.maximum_health)
            else {
                continue;
            };
            live.push(player.id);
            let position = project_anchor(player.position, WORLD_NAMEPLATE_WORLD_OFFSET_Y, frame);
            if let Some(view) = self.nameplates.iter_mut().find(|view| view.id == player.id) {
                if view.name != player.name {
                    view.name = player.name.to_owned();
                    view.layout = world_nameplate_visual_layout(player.name);
                }
                view.position = position;
                view.health_ratio = health_ratio;
                view.element = player.element;
            } else {
                self.nameplates.push(NameplateView {
                    id: player.id.to_owned(),
                    name: player.name.to_owned(),
                    layout: world_nameplate_visual_layout(player.name),
                    position,
                    health_ratio,
                    element: player.element,
                });
            }
        }
        self.nameplates.retain(|view| live.contains(&view.id.as_str()));
    }

    fn prepare_activities(
        &mut self,
        players: &[&WorldOverlayPlayer<'_>],
        frame: &WorldOverlayFrame<'_>,
        diagnostics: &mut WorldOverlayDiagnostics,
    ) {
        let mut live = Vec::new();
        for player in players {
            let Some(activity) = player.activity.filter(|_| frame.renderable) else { continue };
            live.push(player.id);
            let position = project_anchor(player.position, WORLD_SPEECH_WORLD_OFFSET_Y, frame);
            if let Some(view) = self.activities.iter_mut().find(|view| view.id == player.id) {
                view.position = position;
                view.activity = activity;
            } else {
                self.activities.push(ActivityView { id: player.id.to_owned(), position, activity });
            }
            if position.is_some() {
                diagnostics.activity_player_ids.push(player.id.to_owned());
                diagnostics.activity_states.push(activity);
            }
        }
        self.activities.retain(|view| live.contains(&view.id.as_str()));
    }

    fn prepare_speech(
        &mut self,
        players: &[&WorldOverlayPlayer<'_>],
        frame: &WorldOverlayFrame<'_>,
        diagnostics: &mut WorldOverlayDiagnostics,
    ) {
        let mut live = Vec::new();
        let mut speeches: Vec<_> = self.speeches.iter().collect();
        speeches.sort_by(|left, right| left.player_id.cmp(&right.player_id));
        for speech in speeches {
            let Some(alpha) = sample_game_world_speech_alpha(speech, frame.now_ms) else {
                continue;
            };
            let Some(player) = players.iter().find(|player| player.id == speech.player_id) else {
                continue;
            };
            let layout = layout_native_world_speech(&speech.text);
            if layout.glyph_count == 0 {
                continue;
            }
            live.push(speech.player_id.as_str());
            diagnostics.speech_active_count += 1;
            let position = project_anchor(player.position, WORLD_SPEECH_WORLD_OFFSET_Y, frame);
            if let Some(view) =
                self.speech_views.iter_mut().find(|view| view.id == speech.player_id)
            {
                if view.text != speech.text || view.sequence != speech.sequence {
                    view.text.clone_from(&speech.text);
                    view.sequence = speech.sequence;
                    view.layout = layout;
                }
                view.position = position;
                view.alpha = alpha;
            } else {
                self.speech_views.push(SpeechView {
                    id: speech.player_id.clone(),
                    text: speech.text.clone(),
                    sequence: speech.sequence,
                    layout,
                    position,
                    alpha,
                });
            }
            if position.is_some() {
                diagnostics.speech_player_ids.push(speech.player_id.clone());
                diagnostics.speech_sequences.push(speech.sequence);
                diagnostics.speech_alphas.push(alpha);
            }
        }
        self.speech_views.retain(|view| live.contains(&view.id.as_str()));
    }

    fn draw_nameplate(&self, renderer: &mut Renderer, position: Vec2, view: &NameplateView) {
        let width = view.layout.width;
        let half_width = width / 2.0;
        chamfered_plate(renderer, position, [half_width + 1.0, -16.0, 26.0], 4.0, 0x000000, 0.45);
        chamfered_plate(renderer, position, [half_width, -16.0, 24.0], 3.0, 0xc8a862, 0.92);
        chamfered_plate(renderer, position, [half_width - 1.0, -15.0, 22.0], 3.0, 0x0f0c13, 0.9);
        let sheen_left = -half_width + 4.0;
        let sheen_width = width - 8.0;
        rectangle(renderer, position, [sheen_left, -15.0, sheen_width, 1.0], 0xf0d491, 0.22);
        rectangle(renderer, position, [sheen_left, -14.0, sheen_width, 4.0], 0xffffff, 0.035);
        let rail_left = -half_width + 4.0;
        let rail_width = width - 8.0;
        rectangle(
            renderer,
            position,
            [rail_left - 1.0, 1.0, rail_width + 2.0, 6.0],
            0x000000,
            0.55,
        );
        let accent = match view.element {
            WizardElement::Air => 0xd9f3ff,
            WizardElement::Earth => 0x86d65e,
            WizardElement::Ether => 0xc97cff,
            WizardElement::Fire => 0xff8436,
            WizardElement::Water => 0x55b8ff,
        };
        for x in [-half_width, half_width] {
            diamond(renderer, position, Vec2::new(x, -7.0), 5.0, accent, 0.26);
            diamond(renderer, position, Vec2::new(x, -7.0), 3.0, accent, 1.0);
            rectangle(renderer, position, [x - 1.0, -8.0, 1.0, 1.0], 0xffffff, 0.85);
        }
        rectangle(renderer, position, [rail_left, 2.0, rail_width, 4.0], 0x2b1312, 0.94);
        let fill_width = (rail_width * view.health_ratio).round();
        if fill_width > 0.0 {
            rectangle(renderer, position, [rail_left, 2.0, fill_width, 4.0], 0xb9342c, 1.0);
            rectangle(renderer, position, [rail_left, 2.0, fill_width, 1.0], 0xe78369, 0.9);
            rectangle(renderer, position, [rail_left, 5.0, fill_width, 1.0], 0x8a241f, 0.9);
        }
        for shadow in [true, false] {
            let shadow_offset = if shadow { 1.0 } else { 0.0 };
            for glyph in &view.layout.glyphs {
                let offset = view.layout.glyph_offset;
                self.draw_glyph(
                    renderer,
                    glyph,
                    Vec2::new(position.x + offset.x, position.y + offset.y + shadow_offset),
                    if shadow { 0x000000 } else { 0xefe3c6 },
                    if shadow { 0.5 } else { 1.0 },
                );
            }
        }
    }

    fn draw_speech(
        &self,
        renderer: &mut Renderer,
        position: Vec2,
        layout: &NativeWorldSpeechLayout,
        alpha: f64,
    ) {
        let left = -layout.width / 2.0;
        let top = -layout.height - 6.0;
        round_rectangle(
            renderer,
            position,
            [left, top, layout.width, layout.height],
            4.0,
            0xc8a862,
            0.88 * alpha,
        );
        round_rectangle(
            renderer,
            position,
            [left + 1.0, top + 1.0, layout.width - 2.0, layout.height - 2.0],
            3.0,
            0x0f0c13,
            0.9 * alpha,
        );
        polygon(
            renderer,
            position,
            &[Vec2::new(-5.0, -6.0), Vec2::new(5.0, -6.0), Vec2::ZERO],
            0xc8a862,
            0.88 * alpha,
        );
        polygon(
            renderer,
            position,
            &[Vec2::new(-3.0, -6.0), Vec2::new(3.0, -6.0), Vec2::new(0.0, -2.0)],
            0x0f0c13,
            0.9 * alpha,
        );
        let cap_center = (-19.0 + 6.0) * WORLD_SPEECH_GLYPH_SCALE / 2.0;
        for (index, line) in layout.lines.iter().enumerate() {
            let baseline = top
                + 6.0
                + index as f64 * WORLD_SPEECH_LINE_HEIGHT
                + WORLD_SPEECH_LINE_HEIGHT / 2.0
                - cap_center;
            for shadow in [true, false] {
                let shadow_offset = if shadow { 1.0 } else { 0.0 };
                for glyph in &line.glyphs {
                    self.draw_glyph(
                        renderer,
                        glyph,
                        Vec2::new(
                            position.x - line.advance / 2.0,
                            position.y + baseline + shadow_offset,
                        ),
                        if shadow { 0x000000 } else { 0xefe3c6 },
                        alpha * if shadow { 0.55 } else { 1.0 },
                    );
                }
            }
        }
    }

    fn draw_glyph(
        &self,
        renderer: &mut Renderer,
        layout: &NativeUiGlyphLayout,
        origin: Vec2,
        tint: u32,
        alpha: f64,
    ) {
        let glyph = layout.glyph;
        let [_, _, width, height] = glyph.frame;
        renderer.draw(
            &self.font_atlas,
            Quad {
                source: native_sprite_frame_texel_rect(glyph.frame),
                destination: [
                    (origin.x
                        + layout.center_x
                        + (glyph.trim_origin[0] - glyph.logical_size[0] / 2.0) * layout.scale)
                        as f32,
                    (origin.y
                        + layout.center_y
                        + (glyph.trim_origin[1] - glyph.logical_size[1] / 2.0) * layout.scale)
                        as f32,
                    (width * layout.scale) as f32,
                    (height * layout.scale) as f32,
                ],
                tint: graphics::rgba(tint, alpha),
                blend: BlendMode::Normal,
            },
        );
    }
}

fn project_anchor(position: Vec2, offset_y: f64, frame: &WorldOverlayFrame<'_>) -> Option<Vec2> {
    if !frame.renderable {
        return None;
    }
    project_native_world_point(
        Vec2::new(position.x, position.y + offset_y),
        Vec2::new(
            -frame.camera.origin.x * frame.camera.scale,
            -frame.camera.origin.y * frame.camera.scale,
        ),
        frame.camera.scale,
        frame.viewport,
    )
    .map(|point| Vec2::new(point.x.round(), point.y.round()))
}

fn draw_activity(renderer: &mut Renderer, position: Vec2, activity: HubActivity) {
    circle(renderer, position, Vec2::ZERO, 10.0, 0x0b0d12, 0.9);
    circle_stroke(renderer, position, 10.0, 1.5, 0xd8ba72, 0.95);
    if activity == HubActivity::Paused {
        round_rectangle(renderer, position, [-4.5, -5.0, 3.0, 10.0], 1.0, 0xf0d58f, 1.0);
        round_rectangle(renderer, position, [1.5, -5.0, 3.0, 10.0], 1.0, 0xf0d58f, 1.0);
        return;
    }
    for x in [-4.5, 0.0, 4.5] {
        circle(renderer, position, Vec2::new(x, 0.0), 1.5, 0xf0d58f, 1.0);
    }
}
