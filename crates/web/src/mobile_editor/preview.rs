//! GPU source MobileUiElementPreview/MobileUiBeltPreview over actual player fields.

use super::MobileUiEditor;
use crate::gpu_ui::{StockAtlas, UiFrame};
use egui::{Color32, FontFamily, FontId, Painter, Pos2, Rect, Stroke, pos2, vec2};
use solomon_sim::appearance::equipment::native_wizard_icon_primary_tint;
use solomon_sim::belt::{
    NATIVE_BELT_SLOT_COUNT, NativeBeltEntry, NativeBeltPotionKind, native_belt_entry_item,
    native_belt_potion_projection,
};
use solomon_sim::economy::{DOWSING_EQUIPMENT_RECIPES, EquipmentType, HubInventoryItem};
use solomon_sim::hud::quickbar::{
    NativeSkillQuickbarCooldownPresentation, native_skill_quickbar_icon_alpha,
};
use solomon_sim::mobile_ui::{MobileUiElementId, mobile_ui_belt_element_id};
use solomon_sim::player::PlayerState;
use solomon_sim::profile::native_skill_category;
use solomon_sim::secondary::{
    NativeSecondaryAbilityId, native_secondary_ability_mana_cost_with_stat_book,
};

#[derive(Clone, Debug, PartialEq)]
struct ItemIcon {
    id: u64,
    records: Vec<usize>,
    tints: [Option<u32>; 2],
    equipment_type: Option<EquipmentType>,
    quantity: u64,
}

#[derive(Clone, Debug, PartialEq)]
enum BeltIcon {
    Skill {
        id: usize,
        record: usize,
        unavailable: bool,
        cooldown: NativeSkillQuickbarCooldownPresentation,
    },
    Item(ItemIcon),
    Potion {
        mana: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MobileEditorPlayerPreview {
    slots: [Option<BeltIcon>; 8],
    aliases: [Option<&'static str>; 8],
    potions: [BeltIcon; 2],
    in_hub: bool,
}

impl MobileEditorPlayerPreview {
    pub fn from_player(player: &PlayerState, in_hub: bool) -> Self {
        let factors = player.derived_stats().offensive_factors(&player.skill_runtime);
        let mut slots = std::array::from_fn(|_| None);
        let mut aliases = [None; 8];
        // Source MobileUiElementPreview renders the eight named editor controls
        // from bank zero. The live HUD selects its own bank before projecting it;
        // the editor must never enumerate the other sixteen persisted entries.
        for (slot, entry) in player.belt.entries.iter().take(NATIVE_BELT_SLOT_COUNT).enumerate() {
            let id = mobile_ui_belt_element_id(&player.belt, slot);
            if id != MobileUiElementId::SLOTS[slot] {
                aliases[slot] =
                    Some(if id == MobileUiElementId::HealthPotion { "Health" } else { "Mana" });
                continue;
            }
            slots[slot] = match *entry {
                Some(NativeBeltEntry::Skill { skill_id }) => {
                    let ability = NativeSecondaryAbilityId::from_skill_id(skill_id);
                    let unavailable = ability.is_some_and(|ability| {
                        in_hub
                            || player.combat.current_mana
                                < native_secondary_ability_mana_cost_with_stat_book(
                                    &player.book,
                                    &player.stat_book,
                                    ability,
                                    &factors,
                                )
                    });
                    Some(BeltIcon::Skill {
                        id: skill_id,
                        record: player.book.skill_icon_record(skill_id),
                        unavailable,
                        cooldown: NativeSkillQuickbarCooldownPresentation::default(),
                    })
                }
                Some(entry) => {
                    if let Some(item) = native_belt_entry_item(entry, &player.economy) {
                        Some(BeltIcon::Item(item_icon(item, u64::from(item.quantity), player)))
                    } else {
                        match entry {
                            NativeBeltEntry::HealthPotion => Some(BeltIcon::Potion { mana: false }),
                            NativeBeltEntry::ManaPotion => Some(BeltIcon::Potion { mana: true }),
                            _ => None,
                        }
                    }
                }
                None => None,
            };
        }
        let potions = [NativeBeltPotionKind::Health, NativeBeltPotionKind::Mana].map(|kind| {
            let projection = native_belt_potion_projection(&player.economy.backpack, kind);
            projection
                .item
                .map_or(BeltIcon::Potion { mana: kind == NativeBeltPotionKind::Mana }, |item| {
                    BeltIcon::Item(item_icon(item, projection.count, player))
                })
        });
        Self { slots, aliases, potions, in_hub }
    }

    pub fn set_cooldowns(&mut self, cooldowns: &[NativeSkillQuickbarCooldownPresentation; 83]) {
        for icon in self.slots.iter_mut().flatten() {
            if let BeltIcon::Skill { id, cooldown, .. } = icon {
                *cooldown = if native_skill_category(*id) == Some(2) {
                    cooldowns[*id]
                } else {
                    NativeSkillQuickbarCooldownPresentation::default()
                };
            }
        }
    }
}

fn item_icon(item: &HubInventoryItem, quantity: u64, player: &PlayerState) -> ItemIcon {
    ItemIcon {
        id: item.id,
        records: item.icon_records.clone(),
        equipment_type: item.equipment_type,
        quantity,
        tints: if matches!(item.equipment_type, Some(EquipmentType::Hat | EquipmentType::Robe)) {
            item.icon_tints
                .or_else(|| {
                    item.recipe_index
                        .and_then(|index| DOWSING_EQUIPMENT_RECIPES.get(index))
                        .map(|recipe| recipe.icon_tints)
                })
                .unwrap_or([
                    Some(native_wizard_icon_primary_tint(player.selection.element)),
                    Some(0xffffff),
                ])
        } else {
            [None, None]
        },
    }
}

#[derive(Clone, Copy)]
pub(super) struct ElementPaint {
    pub center: Pos2,
    pub natural: egui::Vec2,
    pub scale: f32,
    pub rotation: f32,
}
impl ElementPaint {
    pub fn point(self, point: Pos2) -> Pos2 {
        self.center
            + egui::emath::Rot2::from_angle(self.rotation)
                * ((point.to_vec2() - self.natural / 2.0) * self.scale)
    }
    pub fn quad(self, rect: Rect) -> [Pos2; 4] {
        [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()]
            .map(|point| self.point(point))
    }
    fn image(
        self,
        frame: &UiFrame<'_>,
        painter: &Painter,
        path: &str,
        rect: Rect,
        uv: Option<Rect>,
        tint: Color32,
    ) {
        frame.image_quad(
            painter,
            path,
            self.quad(rect),
            uv.unwrap_or(Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0))),
            tint,
        );
    }
    fn rounded(self, painter: &Painter, rect: Rect, radius: f32, fill: Color32, stroke: Stroke) {
        let radius = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
        let mut points = Vec::new();
        for (center, angle) in [
            (rect.right_top() + vec2(-radius, radius), -std::f32::consts::FRAC_PI_2),
            (rect.right_bottom() - vec2(radius, radius), 0.0),
            (rect.left_bottom() + vec2(radius, -radius), std::f32::consts::FRAC_PI_2),
            (rect.left_top() + vec2(radius, radius), std::f32::consts::PI),
        ] {
            for i in 0..=8 {
                let angle = angle + i as f32 * std::f32::consts::FRAC_PI_2 / 8.0;
                points.push(self.point(center + vec2(angle.cos(), angle.sin()) * radius));
            }
        }
        painter.add(egui::Shape::convex_polygon(
            points,
            fill,
            Stroke::new(stroke.width * self.scale, stroke.color),
        ));
    }
    fn circle(self, painter: &Painter, center: Pos2, radius: f32, fill: Color32, stroke: Stroke) {
        painter.circle(
            self.point(center),
            radius * self.scale,
            fill,
            Stroke::new(stroke.width * self.scale, stroke.color),
        );
    }
    // This preview primitive keeps placement, alignment, font style, and ink independent.
    #[allow(clippy::too_many_arguments)]
    fn text(
        self,
        painter: &Painter,
        point: Pos2,
        align: egui::Align2,
        text: &str,
        size: f32,
        family: &str,
        color: Color32,
    ) {
        let galley = painter.layout_no_wrap(
            text.to_owned(),
            FontId::new(size * self.scale, FontFamily::Name(family.into())),
            color,
        );
        let rect = align.anchor_rect(Rect::from_min_size(self.point(point), galley.size()));
        painter
            .add(egui::epaint::TextShape::new(rect.min, galley, color).with_angle(self.rotation));
    }
    // Registered sprites retain independent Source center, per-axis scale, rotation, and tint.
    #[allow(clippy::too_many_arguments)]
    fn art(
        self,
        frame: &UiFrame<'_>,
        painter: &Painter,
        atlas: &StockAtlas,
        index: usize,
        center: Pos2,
        scale: egui::Vec2,
        rotation: f32,
        tint: Color32,
    ) -> Result<(), String> {
        let record = atlas.record(index)?;
        let [left, top] = record.registered_origin();
        let logical = vec2(record.logical_width as f32, record.logical_height as f32);
        let start = vec2(left, top) - logical / 2.0;
        let rotate = egui::emath::Rot2::from_angle(rotation);
        let corners = [
            start,
            start + vec2(record.width as f32, 0.0),
            start + vec2(record.width as f32, record.height as f32),
            start + vec2(0.0, record.height as f32),
        ]
        .map(|point| self.point(center + rotate * (point * scale)));
        let image = frame
            .image_size(atlas.path())
            .ok_or_else(|| format!("{} is not ready", atlas.path()))?;
        let uv = Rect::from_min_max(
            pos2(record.x as f32 / image.x, record.y as f32 / image.y),
            pos2(
                (record.x + record.width) as f32 / image.x,
                (record.y + record.height) as f32 / image.y,
            ),
        );
        frame.image_quad(painter, atlas.path(), corners, uv, tint);
        Ok(())
    }
}
impl MobileUiEditor {
    pub(super) fn paint_preview(
        &self,
        frame: &UiFrame<'_>,
        painter: &Painter,
        id: MobileUiElementId,
        paint: ElementPaint,
    ) -> Result<(), String> {
        let size = paint.natural;
        let center = (size / 2.0).to_pos2();
        match id {
            MobileUiElementId::Pause
            | MobileUiElementId::Inventory
            | MobileUiElementId::Skillbook => {
                let (record, fraction) = match id {
                    MobileUiElementId::Pause => (42, 36.0 / 44.0),
                    MobileUiElementId::Inventory => (47, 116.0 / 130.0),
                    _ => (48, 116.0 / 130.0),
                };
                let definition = self.art.ui.record(record)?;
                let scale = size.x * fraction / definition.logical_width as f32;
                paint.art(
                    frame,
                    painter,
                    &self.art.ui,
                    record,
                    center,
                    vec2(scale, scale),
                    0.0,
                    Color32::WHITE,
                )?;
            }
            MobileUiElementId::Diagnostics => paint.text(
                painter,
                center,
                egui::Align2::CENTER_CENTER,
                "60 FPS   0 ms",
                10.2,
                "JetBrains Mono",
                Color32::from_rgb(216, 212, 200),
            ),
            MobileUiElementId::Meters => {
                let width = size.x * 0.34375;
                for (left, asset) in [(0.0, "red"), (size.x - width, "blue")] {
                    let rect = Rect::from_min_size(pos2(left, 0.0), vec2(width, size.y));
                    paint.rounded(
                        painter,
                        rect,
                        size.y / 2.0,
                        Color32::from_rgb(32, 38, 41),
                        Stroke::new(1.0, Color32::from_rgb(169, 163, 154)),
                    );
                    paint.image(
                        frame,
                        painter,
                        &format!("assets/stock/hub-hud-bar-{asset}.png"),
                        Rect::from_min_size(
                            pos2(left + width * 0.0273, size.y * 0.15),
                            vec2(width * 0.9091, size.y * 0.5),
                        ),
                        None,
                        Color32::WHITE,
                    );
                }
            }
            MobileUiElementId::LeftJoystick | MobileUiElementId::RightJoystick => {
                paint.circle(
                    painter,
                    center,
                    size.x / 2.0,
                    Color32::from_rgba_unmultiplied(18, 25, 29, 158),
                    Stroke::new(3.0, Color32::from_rgba_unmultiplied(238, 232, 207, 184)),
                );
                paint.circle(
                    painter,
                    center,
                    size.x * 0.21,
                    Color32::from_rgb(37, 43, 44),
                    Stroke::new(2.0, Color32::from_rgba_unmultiplied(239, 232, 208, 178)),
                );
                paint.circle(
                    painter,
                    center - vec2(size.x * 0.015, size.y * 0.025),
                    size.x * 0.15,
                    Color32::from_rgb(85, 91, 88),
                    Stroke::NONE,
                );
                paint.text(
                    painter,
                    pos2(size.x / 2.0, size.y * 0.85),
                    egui::Align2::CENTER_CENTER,
                    if id == MobileUiElementId::LeftJoystick { "MOVE" } else { "AIM / CAST" },
                    11.0 * self.ui_scale as f32,
                    "Cinzel Bold",
                    Color32::from_rgb(217, 210, 188),
                );
            }
            MobileUiElementId::Xp => {
                for record in [81, 82] {
                    let definition = self.art.ui.record(record)?;
                    paint.art(
                        frame,
                        painter,
                        &self.art.ui,
                        record,
                        center,
                        vec2(
                            size.x / definition.logical_width as f32,
                            size.y / definition.logical_height as f32,
                        ),
                        0.0,
                        Color32::WHITE,
                    )?;
                }
            }
            MobileUiElementId::HealthPotion | MobileUiElementId::ManaPotion => {
                let mana = id == MobileUiElementId::ManaPotion;
                let fallback = BeltIcon::Potion { mana };
                let icon = self
                    .preview
                    .as_ref()
                    .map_or(&fallback, |preview| &preview.potions[usize::from(mana)]);
                self.paint_belt(frame, painter, paint, Some(icon))?;
            }
            _ => {
                let slot = MobileUiElementId::SLOTS
                    .iter()
                    .position(|slot| *slot == id)
                    .expect("remaining editor elements are belt slots");
                let icon = self.preview.as_ref().and_then(|preview| preview.slots[slot].as_ref());
                self.paint_belt(frame, painter, paint, icon)?;
                paint.text(
                    painter,
                    pos2(4.0, 3.0),
                    egui::Align2::LEFT_TOP,
                    &(slot + 1).to_string(),
                    10.0,
                    "JetBrains Mono",
                    Color32::from_rgb(255, 241, 184),
                );
                if let Some(alias) = self.preview.as_ref().and_then(|preview| preview.aliases[slot])
                {
                    paint.text(
                        painter,
                        center,
                        egui::Align2::CENTER_CENTER,
                        &format!("{alias}\ncontrol"),
                        9.0,
                        "Cinzel",
                        Color32::from_rgb(238, 231, 209),
                    );
                }
            }
        }
        Ok(())
    }
    fn paint_belt(
        &self,
        frame: &UiFrame<'_>,
        painter: &Painter,
        paint: ElementPaint,
        icon: Option<&BeltIcon>,
    ) -> Result<(), String> {
        let natural = paint.natural;
        let center = (natural / 2.0).to_pos2();
        let base =
            (self.geometry.sizes[MobileUiElementId::LeftJoystick as usize].width / 237.5) as f32;
        paint.rounded(
            painter,
            Rect::from_min_size(Pos2::ZERO, natural),
            14.0 * base,
            Color32::from_rgba_unmultiplied(9, 13, 18, if icon.is_some() { 158 } else { 82 }),
            Stroke::new(
                2.0 * base,
                Color32::from_rgba_unmultiplied(
                    226,
                    216,
                    181,
                    if icon.is_some() { 107 } else { 46 },
                ),
            ),
        );
        match icon {
            Some(BeltIcon::Skill { record, unavailable, cooldown, .. }) => {
                if cooldown.remaining > 0.0 && cooldown.capacity > 0.0 {
                    let points = crate::gpu_ui::cooldown_sector_points(
                        cooldown.remaining,
                        cooldown.capacity,
                    );
                    let points: Vec<_> = points
                        .into_iter()
                        .map(|point| {
                            paint
                                .point(pos2(point.x / 53.0 * natural.x, point.y / 53.0 * natural.y))
                        })
                        .collect();
                    let mut mesh = egui::Mesh::default();
                    for point in &points {
                        mesh.colored_vertex(*point, Color32::from_black_alpha(160));
                    }
                    for index in 1..points.len().saturating_sub(1) {
                        mesh.add_triangle(0, index as u32, (index + 1) as u32);
                    }
                    painter.add(egui::Shape::mesh(mesh));
                }
                let alpha =
                    native_skill_quickbar_icon_alpha(cooldown.remaining > 0.0, *unavailable) as f32;
                let dim = if self.preview.as_ref().is_some_and(|preview| preview.in_hub) {
                    0.25
                } else {
                    1.0
                };
                paint.art(
                    frame,
                    painter,
                    &self.art.skills,
                    *record,
                    center,
                    vec2(base, base),
                    0.0,
                    Color32::from_rgba_unmultiplied(
                        (255.0 * dim) as u8,
                        (255.0 * dim) as u8,
                        (255.0 * dim) as u8,
                        (255.0 * alpha) as u8,
                    ),
                )?;
            }
            Some(BeltIcon::Item(item)) => {
                let (translation, rotation) = match item.equipment_type {
                    Some(EquipmentType::Amulet) => (vec2(0.0, -5.0), 0.0),
                    Some(EquipmentType::Staff) => {
                        (vec2(-22.94306, 32.76608), 35.0_f32.to_radians())
                    }
                    Some(EquipmentType::Wand) => (vec2(0.0, 0.0), 45.0_f32.to_radians()),
                    _ => (egui::Vec2::ZERO, 0.0),
                };
                for (index, record) in item.records.iter().enumerate() {
                    let tint = item.tints.get(index).copied().flatten().unwrap_or(0xffffff);
                    paint.art(
                        frame,
                        painter,
                        &self.art.inventory,
                        *record,
                        center + translation * base,
                        vec2(base, base),
                        rotation,
                        Color32::from_rgb((tint >> 16) as u8, (tint >> 8) as u8, tint as u8),
                    )?;
                }
                if item.quantity > 1 {
                    paint.text(
                        painter,
                        pos2(natural.x - 4.0, natural.y - 3.0),
                        egui::Align2::RIGHT_BOTTOM,
                        &item.quantity.to_string(),
                        12.0,
                        "Georgia",
                        Color32::WHITE,
                    );
                }
            }
            Some(BeltIcon::Potion { mana }) => paint.art(
                frame,
                painter,
                &self.art.inventory,
                if *mana { 47 } else { 46 },
                center,
                vec2(base, base),
                0.0,
                Color32::WHITE,
            )?,
            None => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solomon_sim::NativeRng;
    use solomon_sim::profile::{NewGameSelection, WizardDiscipline, WizardElement};

    #[test]
    fn current_three_bank_player_keeps_editor_on_first_eight_controls() {
        let mut player = PlayerState::new(
            NewGameSelection {
                name: "Felix".into(),
                element: WizardElement::Ether,
                discipline: WizardDiscipline::Arcane,
            },
            &mut NativeRng::new(123),
        );
        assert_eq!(player.belt.entries.len(), 24);
        player.belt.entries.fill(None);
        player.belt.entries[0] = Some(NativeBeltEntry::Skill { skill_id: 8 });
        player.belt.entries[3] = Some(NativeBeltEntry::HealthPotion);
        let before = MobileEditorPlayerPreview::from_player(&player, true);
        assert!(matches!(before.slots[0], Some(BeltIcon::Skill { id: 8, .. })));
        assert_eq!(before.aliases[3], Some("Health"));
        for entry in &mut player.belt.entries[NATIVE_BELT_SLOT_COUNT..] {
            *entry = Some(NativeBeltEntry::ManaPotion);
        }
        assert_eq!(MobileEditorPlayerPreview::from_player(&player, true), before);
    }
}
