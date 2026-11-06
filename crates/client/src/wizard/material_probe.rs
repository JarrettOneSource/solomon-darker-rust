//! Development-only composed regression for the actual articulated wizard painter.
//! The reference submits the independently recovered DrawSpecial program directly.

use solomon_assets::parse_bundle;
use solomon_graphics::{TextureColorMode, Vertex, assets};
use solomon_sim::profile::{NewGameSelection, WizardDiscipline};
use solomon_sim::secondary::motion::create_stoneskin_warp;
use solomon_sim::{HubWizard, Rng};

use super::*;

#[derive(serde::Deserialize)]
struct BareRows {
    rows: Vec<BareRow>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct BareRow {
    pose: usize,
    heading: usize,
    first_point0: [f64; 2],
    second_point0: [f64; 2],
    front_admitted: bool,
    back_admitted: bool,
    effect_point1: [f64; 2],
}

/// One probe comparison: the production painter's pixels, the reference pixels, and how many
/// channels differ and by how much at most.
pub struct MaterialProbeCase {
    pub name: String,
    pub actual: Vec<u8>,
    pub reference: Vec<u8>,
    pub different_channels: usize,
    pub maximum_difference: u8,
}

/// Uses tracked stock art and the production WizardView, without a game save or network.
pub async fn run(
    renderer: &mut Renderer,
    read_pixels: &mut impl FnMut(&mut Renderer) -> Vec<u8>,
) -> Result<Vec<MaterialProbeCase>, String> {
    let records = parse_bundle(&assets::fetch_bytes("assets/stock/Clothes.bundle").await?)
        .map_err(|error| error.to_string())?;
    let atlas = renderer.upload(&assets::load_image("assets/stock/Clothes.png").await?)?;
    let load = |name: &'static str| async move {
        assets::load_image(&format!("assets/stock/{name}.png")).await
    };
    let shadow = renderer.upload(&load("player-shadow").await?)?;
    let back = renderer.upload(&load("player-character-unselected-attachment-back").await?)?;
    let front = renderer.upload(&load("player-character-unselected-attachment-front").await?)?;
    let robe = renderer.upload(&load("player-character-unselected-robe-attachment").await?)?;
    let mut view = WizardView::new(
        records,
        atlas,
        shadow,
        StarterEquipmentAppearance { primary_tint: 0x697c59, secondary_tint: 0xb6b3aa },
        back,
        front,
        robe,
        renderer,
    )?;
    let spells = SpellView::load(renderer).await?;
    let mut player = PlayerState::new(
        NewGameSelection {
            name: "Stone material control".into(),
            element: WizardElement::Earth,
            discipline: WizardDiscipline::Mind,
        },
        &mut Rng::new(125),
    );
    let target = renderer.create_render_target(256)?;
    let reference_capture = renderer.create_render_target(256)?;
    let warp = create_stoneskin_warp(&mut Rng::new(125));
    let camera = Camera { origin: Vec2::new(-128.0, -153.0), scale: 1.0 };
    let mut cases = Vec::new();
    let bare_rows: BareRows = serde_json::from_str(include_str!("material_probe_bare.json"))
        .map_err(|error| error.to_string())?;
    for (weapon_name, weapon_kind, bare_pose) in [
        ("unselected", None, None),
        ("staff", Some(PlayerWeaponKind::Staff), None),
        ("wand", Some(PlayerWeaponKind::Wand), None),
        ("bare-idle", None, Some(0)),
        ("bare-cast2-start", None, Some(1)),
        ("bare-constant", None, Some(2)),
        ("bare-cast2-late", None, Some(3)),
    ] {
        view.equipment.appearance.weapon = bare_pose.is_none().then_some(
            PlayerRenderableWeaponAppearance::Native(PlayerWeaponAppearance {
                kind: weapon_kind.unwrap_or(PlayerWeaponKind::Staff),
                selector: 0,
            }),
        );
        for (heading, tint) in [(0, 0xffffff), (6, 0x697c87), (12, 0xffffff), (18, 0x697c87)] {
            let mut wizard = HubWizard::at(Vec2::ZERO);
            wizard.heading_index = heading;
            wizard.heading_degrees = heading as f64 * 15.0;
            wizard.gait_degrees = 80.0;
            wizard.walk_cycle_primary = 2.0;
            wizard.primary_cast.selected_primary_id = (weapon_kind.is_some()
                || bare_pose.is_some())
            .then_some(if bare_pose == Some(2) { 24 } else { 8 });
            for shield in [false, true] {
                let mut secondary = SecondaryPlayerState {
                    stoneskin_ticks_remaining: 900,
                    magic_shield_absorb: if shield { 80.0 } else { 0.0 },
                    magic_shield_maximum: 80.0,
                    ..SecondaryPlayerState::default()
                };
                if let Some(pose) = bare_pose {
                    configure_bare_pose(&mut wizard, &mut secondary, pose);
                }
                player.skill_runtime.harden.coating = 0.8;
                view.set_material_tint(solomon_sim::presentation::player_material_tint(
                    tint,
                    Some(&secondary),
                    &player.combat,
                ));
                for (mode_index, enhanced_effects) in [true, false, true].into_iter().enumerate() {
                    view.set_player_presentation(WizardPlayerPresentation {
                        player: &player,
                        secondary: Some(&secondary),
                        webbed: None,
                        stoneskin_warp: Some(&warp),
                        enhanced_effects,
                        overlay_effect_phase: if wizard.primary_cast.selected_primary_id.is_some() {
                            0.45
                        } else {
                            0.0
                        },
                        staff_action_pose: None,
                        complex_lighting: true,
                        tick: 300.0,
                        damage_x4_ticks_remaining: if wizard
                            .primary_cast
                            .selected_primary_id
                            .is_some()
                        {
                            100.0
                        } else {
                            0.0
                        },
                    });
                    let actual = capture(renderer, &target, read_pixels, |renderer| {
                        view.draw_body(renderer, &camera, &wizard, 300.0, &spells);
                    });
                    let reference = capture(renderer, &target, read_pixels, |renderer| {
                        // Ordinary underlying body and the special diffuse capture each use
                        // the once-applied actor half tint. The shell never enters this target.
                        view.draw_actor(
                            renderer,
                            &camera,
                            &wizard,
                            300.0,
                            &spells,
                            WizardDrawPass::World,
                        );
                        renderer.render_diffuse_to(&reference_capture, [256.0; 2], |renderer| {
                            view.draw_actor(
                                renderer,
                                &camera,
                                &wizard,
                                300.0,
                                &spells,
                                WizardDrawPass::Diffuse,
                            );
                        });
                        renderer.render_onto(&reference_capture, [256.0; 2], |renderer| {
                            let record = &view.records[3];
                            let head = wizard_draw_plan(heading, 2.0, 80.0).head_offset;
                            super::material::draw_centered_texture(
                                renderer,
                                &Camera { origin: Vec2::ZERO, scale: 1.0 },
                                &view.atlas,
                                crate::beam_mesh::sprite_texel_rect(record),
                                Vec2::new(128.0 + head.x, 128.0 + head.y),
                                Vec2::new(record.width as f64, record.height as f64),
                                heading as f64 * 24.0 * std::f64::consts::PI / 180.0,
                                [1.0; 4],
                                BlendMode::Multiply,
                            );
                        });
                        let vertices: Vec<_> = warp
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .enumerate()
                            .map(|(index, xy)| Vertex {
                                position: [128.0 + xy[0] as f32, 128.0 + xy[1] as f32],
                                texel: [
                                    (fround(fround((index / 10) as f64 * 0.05_f32 as f64) + 0.25)
                                        * 256.0) as f32,
                                    ((1.0
                                        - fround(
                                            fround((index % 10) as f64 * 0.05_f32 as f64) + 0.25,
                                        ))
                                        * 256.0) as f32,
                                ],
                                tint: [1.0; 4],
                            })
                            .collect();
                        let mut indices = Vec::new();
                        for x in 0..9_u32 {
                            for y in 0..9_u32 {
                                let a = x * 10 + y;
                                indices.extend([a, a + 10, a + 1, a + 1, a + 10, a + 11]);
                            }
                        }
                        for _ in 0..2 {
                            if enhanced_effects {
                                renderer.draw_indexed_with_color_mode(
                                    &reference_capture.texture,
                                    &vertices,
                                    &indices,
                                    BlendMode::Normal,
                                    TextureColorMode::Texture,
                                );
                            } else {
                                renderer.draw(
                                    &reference_capture.texture,
                                    Quad {
                                        source: [0.0, 256.0, 256.0, -256.0],
                                        destination: [-0.5, -0.5, 256.0, 256.0],
                                        tint: [1.0; 4],
                                        blend: BlendMode::Normal,
                                    },
                                );
                            }
                        }
                        if let Some(kind) = weapon_kind {
                            // Independently recovered pose0 sockets, in heading0/6/12/18 order.
                            // DrawSpecial calls the selected helper once, without gait translation.
                            let (sockets, scale) = if kind == PlayerWeaponKind::Staff {
                                ([[-32.5, -66.5], [38.5, -61.5], [32.5, -1.5], [-38.5, -6.5]], 5.5)
                            } else {
                                (
                                    [[14.5, -50.5], [39.5, -5.0], [-14.5, 16.0], [-39.5, -29.5]],
                                    3.3000001907348633,
                                )
                            };
                            let [x, y] = sockets[heading / 6];
                            let position = Vec2::new(x, y);
                            view.draw_damage_x4(renderer, &camera, position, 300.0, scale, &spells);
                            spells.draw_element_vfx(
                                renderer,
                                &camera,
                                position,
                                &selected_primary_element_vfx_plan(8, 300.0, scale),
                            );
                        }
                        if let Some(pose) = bare_pose {
                            let row = &bare_rows.rows[pose * 24 + heading];
                            draw_bare_reference(
                                &view, renderer, &camera, &wizard, &spells, row, true,
                            );
                        }
                        if let Some(shell) =
                            view.player_presentation.magic_shield.filter(|shell| shell.visible)
                        {
                            let record = &view.records[2];
                            let mut color = tint_rgba(shell.tint);
                            color[3] = ((shell.alpha * 255.0).trunc() / 255.0) as f32;
                            super::material::draw_centered_texture(
                                renderer,
                                &camera,
                                &view.atlas,
                                crate::beam_mesh::sprite_texel_rect(record),
                                Vec2::new(
                                    record.center_x as f64 * shell.scale,
                                    -35.0 + record.center_y as f64 * shell.scale,
                                ),
                                Vec2::new(
                                    record.width as f64 * shell.scale,
                                    record.height as f64 * shell.scale,
                                ),
                                0.0,
                                color,
                                BlendMode::Add,
                            );
                        }
                    });
                    let different_channels =
                        actual.iter().zip(&reference).filter(|(a, b)| a != b).count();
                    let maximum_difference = actual
                        .iter()
                        .zip(&reference)
                        .map(|(a, b)| a.abs_diff(*b))
                        .max()
                        .unwrap_or(0);
                    cases.push(MaterialProbeCase {
                    name: format!("{weapon_name}-heading-{heading}-shield-{shield}-mode-{mode_index}-{enhanced_effects}"),
                    actual,
                    reference,
                    different_channels,
                    maximum_difference,
                });
                }
            }
        }
    }
    cases.extend(enchant_cases(renderer, read_pixels, &target, &mut view, &camera));
    cases.push(texture_rgb_scope_case(renderer, read_pixels, &target)?);
    cases.extend(bare_cases(
        renderer,
        read_pixels,
        &target,
        &mut view,
        &player,
        &spells,
        &camera,
        &bare_rows,
    ));
    renderer.destroy_render_target(&reference_capture);
    renderer.destroy_render_target(&target);
    Ok(cases)
}

fn configure_bare_pose(wizard: &mut HubWizard, secondary: &mut SecondaryPlayerState, pose: usize) {
    wizard.primary_cast.hand_cast = None;
    wizard.primary_cast.channel_active = pose == 2;
    wizard.primary_cast.action_tick = (pose == 2).then_some(1.0);
    secondary.cast_action = if matches!(pose, 1 | 3) {
        let mut action = solomon_sim::secondary::SecondaryCastAction::new(None);
        action.progress = if pose == 1 { 0.5 } else { 1.5 };
        action.hand.as_mut().unwrap().pose = pose;
        Some(action)
    } else {
        None
    };
}

#[allow(clippy::too_many_arguments)]
fn draw_bare_reference(
    view: &WizardView,
    renderer: &mut Renderer,
    camera: &Camera,
    wizard: &HubWizard,
    spells: &SpellView,
    row: &BareRow,
    front: bool,
) {
    if !(if front { row.front_admitted } else { row.back_admitted }) {
        return;
    }
    if row.pose == 2 && !wizard.primary_cast.channel_active {
        return;
    }
    // Literal retained PE PI and independent extracted point1, not the production plan.
    let radians = fround(3.141592502593994 * fround(600.0) / 180.0);
    let bob = fround(solomon_sim::float::sin(radians));
    let position = Vec2::new(row.effect_point1[0], fround(row.effect_point1[1] + bob));
    let scale = 4.400000095367432;
    view.draw_damage_x4(renderer, camera, position, 300.0, scale, spells);
    spells.draw_element_vfx(
        renderer,
        camera,
        position,
        &selected_primary_element_vfx_plan(if row.pose == 2 { 24 } else { 8 }, 300.0, scale),
    );
}

#[allow(clippy::too_many_arguments)]
fn bare_cases(
    renderer: &mut Renderer,
    read_pixels: &mut impl FnMut(&mut Renderer) -> Vec<u8>,
    target: &RenderTarget,
    view: &mut WizardView,
    player: &PlayerState,
    spells: &SpellView,
    camera: &Camera,
    rows: &BareRows,
) -> Vec<MaterialProbeCase> {
    let mut cases = Vec::new();
    view.equipment.appearance.weapon = None;
    for row in &rows.rows {
        let mut wizard = HubWizard::at(Vec2::ZERO);
        wizard.heading_index = row.heading;
        wizard.primary_cast.selected_primary_id = Some(if row.pose == 2 { 24 } else { 8 });
        let mut secondary = SecondaryPlayerState::default();
        configure_bare_pose(&mut wizard, &mut secondary, row.pose);
        view.set_player_presentation(WizardPlayerPresentation {
            player,
            secondary: Some(&secondary),
            webbed: None,
            stoneskin_warp: None,
            enhanced_effects: true,
            overlay_effect_phase: 0.45,
            staff_action_pose: None,
            complex_lighting: true,
            tick: 300.0,
            damage_x4_ticks_remaining: 100.0,
        });
        for front in [false, true] {
            let actual = capture(renderer, target, read_pixels, |renderer| {
                view.draw_selected_element(
                    renderer,
                    camera,
                    &wizard,
                    300.0,
                    spells,
                    Vec2::ZERO,
                    front,
                );
            });
            let reference = capture(renderer, target, read_pixels, |renderer| {
                draw_bare_reference(view, renderer, camera, &wizard, spells, row, front);
            });
            cases.push(compare_case(
                format!("bare-helper-pose-{}-heading-{}-front-{front}", row.pose, row.heading),
                actual,
                reference,
            ));
        }
        let frame = WeaponFrame {
            position: Vec2::ZERO,
            heading: row.heading,
            pose: row.pose,
            wand_pose: 0,
            front_offset: Vec2::new(1.5, -0.5),
            weapon: None,
            unselected: false,
            bare_pose: Some(row.pose),
            material_tint: 0x697c87,
            tick: 300.0,
            enchant: false,
        };
        let actual = capture(renderer, target, read_pixels, |renderer| {
            view.draw_weapon(renderer, camera, frame, false);
            view.draw_weapon(renderer, camera, frame, true);
        });
        let reference = capture(renderer, target, read_pixels, |renderer| {
            for front in [false, true] {
                for (base, depth) in [(484, row.first_point0[1]), (676, row.second_point0[1])] {
                    if (depth > 0.5) == front {
                        view.draw_cell_layer(
                            renderer,
                            camera,
                            Vec2::ZERO,
                            base + row.pose * 24 + row.heading,
                            if front { frame.front_offset } else { Vec2::ZERO },
                            frame.material_tint,
                        );
                    }
                }
            }
        });
        cases.push(compare_case(
            format!("bare-hands-pose-{}-heading-{}", row.pose, row.heading),
            actual,
            reference,
        ));
    }
    // A Cast1 terminal K2 without a held channel must suppress all helper output.
    let mut wizard = HubWizard::at(Vec2::ZERO);
    wizard.primary_cast.selected_primary_id = Some(8);
    wizard.primary_cast.hand_cast = Some(solomon_sim::spells::hand_cast::HandCast1::new(
        0.095_f32 as f64,
        2,
        &mut Rng::new(125),
    ));
    let secondary = SecondaryPlayerState::default();
    view.set_player_presentation(WizardPlayerPresentation {
        player,
        secondary: Some(&secondary),
        webbed: None,
        stoneskin_warp: None,
        enhanced_effects: true,
        overlay_effect_phase: 0.45,
        staff_action_pose: None,
        complex_lighting: true,
        tick: 300.0,
        damage_x4_ticks_remaining: 100.0,
    });
    for front in [false, true] {
        let actual = capture(renderer, target, read_pixels, |renderer| {
            view.draw_selected_element(renderer, camera, &wizard, 300.0, spells, Vec2::ZERO, front);
        });
        let reference = capture(renderer, target, read_pixels, |_| {});
        cases.push(compare_case(
            format!("bare-terminal-k2-no-channel-front-{front}"),
            actual,
            reference,
        ));
    }
    cases
}

fn texture_rgb_scope_case(
    renderer: &mut Renderer,
    read_pixels: &mut impl FnMut(&mut Renderer) -> Vec<u8>,
    target: &RenderTarget,
) -> Result<MaterialProbeCase, String> {
    let texture = renderer.upload_rgba(1, 1, &[37, 129, 211, 173])?;
    let draw = |renderer: &mut Renderer, index: usize, mode| {
        let x = 16.0 + index as f32 * 72.0;
        renderer.draw_quad_with_color_mode(
            &texture,
            [0.0, 0.0, 1.0, 1.0],
            [[x, 16.0], [x + 64.0, 16.0], [x + 64.0, 240.0], [x, 240.0]],
            [[0.4, 0.6, 0.8, 0.7]; 4],
            BlendMode::Normal,
            mode,
        );
    };
    let actual = capture(renderer, target, read_pixels, |renderer| {
        renderer.draw_diffuse_with_opacity(1.0, |renderer| {
            draw(renderer, 0, TextureColorMode::Texture);
            renderer
                .draw_with_texture_rgb(|renderer| draw(renderer, 1, TextureColorMode::AlphaMask));
            draw(renderer, 2, TextureColorMode::Texture);
        });
    });
    let reference = capture(renderer, target, read_pixels, |renderer| {
        draw(renderer, 0, TextureColorMode::AlphaMask);
        draw(renderer, 1, TextureColorMode::Texture);
        draw(renderer, 2, TextureColorMode::AlphaMask);
    });
    renderer.destroy_texture(&texture);
    Ok(compare_case("stone-enchant-texture-rgb-scope-restores-hands".into(), actual, reference))
}

fn enchant_cases(
    renderer: &mut Renderer,
    read_pixels: &mut impl FnMut(&mut Renderer) -> Vec<u8>,
    target: &RenderTarget,
    view: &mut WizardView,
    camera: &Camera,
) -> Vec<MaterialProbeCase> {
    let mut cases = Vec::new();
    view.player_presentation.enchant_staff_rank = 1;
    view.player_presentation.enchant_primary_skill_id = Some(8);
    view.player_presentation.stoneskin = true;
    for selector in 0..6 {
        for heading in [0, 6, 12, 18] {
            let weapon = PlayerRenderableWeaponAppearance::Native(PlayerWeaponAppearance {
                kind: PlayerWeaponKind::Staff,
                selector,
            });
            let frame = WeaponFrame {
                position: Vec2::ZERO,
                heading,
                pose: 0,
                wand_pose: 0,
                front_offset: Vec2::new(1.5, -0.5),
                weapon: Some(&weapon),
                unselected: false,
                bare_pose: None,
                material_tint: 0x697c87,
                tick: 300.0,
                enchant: true,
            };
            let actual = capture(renderer, target, read_pixels, |renderer| {
                renderer.draw_diffuse_with_opacity(1.0, |renderer| {
                    view.draw_weapon(renderer, camera, frame, false);
                    view.draw_weapon(renderer, camera, frame, true);
                });
            });
            let reference = capture(renderer, target, read_pixels, |renderer| {
                // Submit the native shaft/gradient/hands sequence independently.
                // There is no additive shaft redraw in this Stone branch.
                let index = heading;
                let hand = &view.records[3244 + index];
                let front = hand.points[0][1] > 0.5;
                let offset = if front { frame.front_offset } else { Vec2::ZERO };
                renderer.draw_diffuse_with_opacity(1.0, |renderer| {
                    view.draw_segment(
                        renderer,
                        camera,
                        frame.position,
                        5 + selector,
                        hand.points[1],
                        hand.points[2],
                        offset,
                        frame.material_tint,
                        BlendMode::Normal,
                    );
                    let plan = enchant_staff_draw_plan(EnchantStaffDrawInput {
                        attachment: StaffAttachmentFrame {
                            start: Vec2::new(hand.points[1][0] as f64, hand.points[1][1] as f64),
                            end: Vec2::new(hand.points[2][0] as f64, hand.points[2][1] as f64),
                            front,
                        },
                        effective_rank: 1,
                        heading_index: heading,
                        living: true,
                        staff: true,
                        pose: 0,
                        selected_primary_skill_id: Some(8),
                        selector,
                        tick: 300.0,
                        weld_build_id: None,
                        width_sample: None,
                    })
                    .expect("learned native Staff");
                    renderer.draw_with_texture_rgb(|renderer| {
                        view.draw_staff_aura(renderer, camera, frame.position, offset, plan);
                    });
                    for base in [3244, 3484] {
                        view.draw_cell_layer(
                            renderer,
                            camera,
                            frame.position,
                            base + index,
                            offset,
                            frame.material_tint,
                        );
                    }
                });
            });
            cases.push(compare_case(
                format!("stone-enchant-selector-{selector}-heading-{heading}"),
                actual,
                reference,
            ));
        }
    }
    cases
}

fn compare_case(name: String, actual: Vec<u8>, reference: Vec<u8>) -> MaterialProbeCase {
    let different_channels = actual.iter().zip(&reference).filter(|(a, b)| a != b).count();
    let maximum_difference =
        actual.iter().zip(&reference).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);
    MaterialProbeCase { name, actual, reference, different_channels, maximum_difference }
}

fn capture(
    renderer: &mut Renderer,
    target: &RenderTarget,
    read_pixels: &mut impl FnMut(&mut Renderer) -> Vec<u8>,
    draw: impl FnOnce(&mut Renderer),
) -> Vec<u8> {
    let mut pixels = Vec::new();
    renderer.render_to_color(target, [256.0; 2], [0.12, 0.12, 0.12, 1.0], |renderer| {
        draw(renderer);
        renderer.flush();
        pixels = read_pixels(renderer);
    });
    pixels
}
