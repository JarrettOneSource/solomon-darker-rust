//! Cheat forms with local drafts and host-validated actions.

use super::{CheatFrame, button, copy};
use crate::gpu_ui::UiFrame;
use egui::{Color32, FontFamily, FontId, RichText, Stroke, vec2};
use solomon_sim::menu::cheats::{
    CHEAT_MENU_EXPERIENCE_MAX, CHEAT_MENU_GOLD_MAX, CHEAT_MENU_RUN_SEED_MAX,
    CHEAT_MENU_SPAWN_COUNT_MAX, CheatMenuAction, CheatMenuCatalogs,
};
use solomon_sim::profile::{WizardDiscipline, WizardElement};

pub(super) struct CheatControls {
    target: String,
    selected_target: String,
    roster: Vec<(String, String)>,
    gold: String,
    experience: String,
    seed: String,
    enemy: String,
    count: String,
    developer: Option<DeveloperControls>,
}
struct DeveloperControls {
    item: String,
    quantity: String,
    skill: String,
    ranks: String,
    weld: String,
    discipline: String,
    element: String,
}
impl Default for DeveloperControls {
    fn default() -> Self {
        Self {
            item: String::new(),
            quantity: "1".into(),
            skill: String::new(),
            ranks: "1".into(),
            weld: String::new(),
            discipline: "arcane".into(),
            element: "fire".into(),
        }
    }
}
impl CheatControls {
    pub fn new(frame: &CheatFrame) -> Result<Self, String> {
        let mut controls = Self {
            target: String::new(),
            selected_target: String::new(),
            roster: Vec::new(),
            gold: "10000".into(),
            experience: "1000".into(),
            seed: "42".into(),
            enemy: String::new(),
            count: "1".into(),
            developer: frame.developer.then(DeveloperControls::default),
        };
        controls.update(frame, false, None)?;
        Ok(controls)
    }
    pub fn update(
        &mut self,
        frame: &CheatFrame,
        _busy: bool,
        catalogs: Option<&CheatMenuCatalogs>,
    ) -> Result<(), String> {
        self.roster =
            frame.players.iter().map(|player| (player.id.clone(), player.name.clone())).collect();
        if !self.roster.iter().any(|(id, _)| *id == self.target) {
            self.target = if self.roster.iter().any(|(id, _)| *id == frame.local_player_id) {
                frame.local_player_id.clone()
            } else {
                self.roster.first().map(|(id, _)| id.clone()).unwrap_or_default()
            };
        }
        if self.target != self.selected_target {
            self.target_changed(frame);
            self.selected_target = self.target.clone();
        }
        if !frame.developer {
            self.developer = None;
        } else if self.developer.is_none() {
            self.developer = Some(DeveloperControls::default());
            if let Some(catalogs) = catalogs {
                self.set_catalogs_values(catalogs);
            }
        }
        Ok(())
    }
    pub fn set_catalogs(&mut self, catalogs: &CheatMenuCatalogs) -> Result<(), String> {
        self.set_catalogs_values(catalogs);
        Ok(())
    }
    fn set_catalogs_values(&mut self, catalogs: &CheatMenuCatalogs) {
        let enemies: Vec<_> = catalogs
            .enemies
            .iter()
            .map(|enemy| (enemy.key.clone(), display_identity(&enemy.base)))
            .collect();
        retain_choice(&mut self.enemy, &enemies);
        if let Some(developer) = &mut self.developer {
            let items: Vec<_> =
                catalogs.items.iter().map(|item| (item.key.clone(), item.name.clone())).collect();
            retain_choice(&mut developer.item, &items);
            let skills: Vec<_> = catalogs
                .skills
                .iter()
                .filter(|skill| !skill.weld_only)
                .map(|skill| {
                    (
                        skill.id.to_string(),
                        format!("{} · {}", skill.name, display_identity(&skill.family)),
                    )
                })
                .collect();
            retain_choice(&mut developer.skill, &skills);
            let welds: Vec<_> = catalogs
                .welds
                .iter()
                .map(|weld| (weld.id.to_string(), weld.name.clone()))
                .collect();
            retain_choice(&mut developer.weld, &welds);
        }
    }
    pub fn target_changed(&mut self, frame: &CheatFrame) {
        if let Some(player) = frame.players.iter().find(|player| player.id == self.target) {
            self.gold = player.gold.to_string();
        }
    }
    pub fn paint(
        &mut self,
        gpu: &UiFrame<'_>,
        ui: &mut egui::Ui,
        frame: &CheatFrame,
        busy: bool,
        catalogs: Option<&CheatMenuCatalogs>,
    ) -> Option<String> {
        let mut action = None;
        egui::Frame::new()
            .fill(Color32::from_black_alpha(209))
            .stroke(Stroke::new(1.0, Color32::from_rgba_unmultiplied(191, 157, 78, 148)))
            .inner_margin(12.0)
            .show(ui, |ui| {
                if select(gpu, ui, "target", "TARGET", &mut self.target, &self.roster) {
                    self.target_changed(frame);
                    self.selected_target = self.target.clone();
                }
                if let Some(target) = frame.players.iter().find(|player| player.id == self.target) {
                    egui::Grid::new("cheats/stats").num_columns(6).spacing(vec2(16.0, 5.0)).show(
                        ui,
                        |ui| {
                            for label in ["HP", "MP", "LEVEL / XP", "GOLD", "STATE", "POSITION"] {
                                ui.label(copy(label, 10.0));
                            }
                            ui.end_row();
                            for value in &target.stats {
                                ui.label(copy(value, 12.0));
                            }
                            ui.end_row();
                        },
                    );
                }
            });
        ui.add_space(12.0);
        let target = frame.players.iter().any(|player| player.id == self.target);
        ui.columns(if ui.available_width() > 650.0 { 2 } else { 1 }, |columns| {
            group(&mut columns[0], "PLAYER", |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (id, label) in [("health", "RESTORE HEALTH"), ("mana", "RESTORE MANA")] {
                        if button(gpu, ui, id, label, !busy && target).clicked() {
                            action = Some(id.into());
                        }
                    }
                });
                if inline(gpu, ui, "gold", "GOLD", &mut self.gold, "SET GOLD", !busy && target) {
                    action = Some("gold".into());
                }
                if inline(
                    gpu,
                    ui,
                    "experience",
                    "EXPERIENCE",
                    &mut self.experience,
                    "GRANT XP",
                    !busy && target,
                ) {
                    action = Some("experience".into());
                }
            });
            let last = columns.len() - 1;
            group(&mut columns[last], "WORLD", |ui| {
                if button(
                    gpu,
                    ui,
                    "next-wave",
                    "NEXT WAVE SPAWN",
                    !busy && frame.next_wave_spawn_available,
                )
                .clicked()
                {
                    action = Some("next-wave".into());
                }
                if !frame.next_wave_spawn_available {
                    hint(ui, "Available between spawns in an active survival Boneyard.");
                }
                if inline(
                    gpu,
                    ui,
                    "seed",
                    "RUN SEED",
                    &mut self.seed,
                    "SET NEXT SEED",
                    !busy && frame.hub_seed_available,
                ) {
                    action = Some("seed".into());
                }
                if !frame.hub_seed_available {
                    hint(ui, "Run seed is available in the Hub.");
                }
                let enemies: Vec<_> = catalogs
                    .map(|catalogs| {
                        catalogs
                            .enemies
                            .iter()
                            .map(|enemy| (enemy.key.clone(), display_identity(&enemy.base)))
                            .collect()
                    })
                    .unwrap_or_default();
                select(gpu, ui, "enemy", "ENEMY", &mut self.enemy, &enemies);
                field(gpu, ui, "count", "COUNT", &mut self.count);
                if button(
                    gpu,
                    ui,
                    "spawn",
                    "SPAWN",
                    !busy && frame.enemy_spawn_available && !self.enemy.is_empty(),
                )
                .clicked()
                {
                    action = Some("spawn".into());
                }
                if !frame.enemy_spawn_available {
                    hint(ui, "Enemy spawning requires an active Boneyard.");
                }
            });
        });
        if let Some(developer) = &mut self.developer {
            group(ui, "DEVELOPER GRANTS", |ui| {
                let items: Vec<_> = catalogs
                    .map(|catalogs| {
                        catalogs
                            .items
                            .iter()
                            .map(|item| (item.key.clone(), item.name.clone()))
                            .collect()
                    })
                    .unwrap_or_default();
                select(gpu, ui, "item", "ITEM", &mut developer.item, &items);
                field(gpu, ui, "quantity", "QTY", &mut developer.quantity);
                if button(gpu, ui, "grant-item", "GRANT ITEM", !busy && !developer.item.is_empty())
                    .clicked()
                {
                    action = Some("item".into());
                }
                let skills: Vec<_> = catalogs
                    .map(|catalogs| {
                        catalogs
                            .skills
                            .iter()
                            .filter(|skill| !skill.weld_only)
                            .map(|skill| {
                                (
                                    skill.id.to_string(),
                                    format!("{} · {}", skill.name, display_identity(&skill.family)),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                select(gpu, ui, "skill", "SKILL", &mut developer.skill, &skills);
                field(gpu, ui, "ranks", "RANKS", &mut developer.ranks);
                if button(
                    gpu,
                    ui,
                    "grant-skill",
                    "GRANT SKILL",
                    !busy && !developer.skill.is_empty(),
                )
                .clicked()
                {
                    action = Some("skill".into());
                }
                let welds: Vec<_> = catalogs
                    .map(|catalogs| {
                        catalogs
                            .welds
                            .iter()
                            .map(|weld| (weld.id.to_string(), weld.name.clone()))
                            .collect()
                    })
                    .unwrap_or_default();
                select(gpu, ui, "weld", "WELD", &mut developer.weld, &welds);
                if button(gpu, ui, "grant-weld", "GRANT WELD", !busy && !developer.weld.is_empty())
                    .clicked()
                {
                    action = Some("weld".into());
                }
                let disciplines: Vec<_> = WizardDiscipline::CHOICES
                    .iter()
                    .map(|value| (value.as_str().to_owned(), value.as_str().to_ascii_uppercase()))
                    .collect();
                select(
                    gpu,
                    ui,
                    "discipline",
                    "BOT DISCIPLINE",
                    &mut developer.discipline,
                    &disciplines,
                );
                let elements: Vec<_> = WizardElement::ALL
                    .iter()
                    .map(|value| (value.as_str().to_owned(), value.as_str().to_ascii_uppercase()))
                    .collect();
                select(gpu, ui, "element", "ELEMENT", &mut developer.element, &elements);
                if button(gpu, ui, "bot", "SUMMON BOT", !busy && frame.bot_summon_available)
                    .clicked()
                {
                    action = Some("bot".into());
                }
                if !frame.bot_summon_available {
                    hint(ui, "Bots require developer access in the shared Hub.");
                }
            });
        }
        action
    }
    pub fn action(
        &self,
        action: &str,
        frame: &CheatFrame,
        catalogs: Option<&CheatMenuCatalogs>,
    ) -> Result<Option<(&'static str, CheatMenuAction)>, String> {
        let player_id = self.target.clone();
        let has_target = frame.players.iter().any(|player| player.id == player_id);
        if matches!(action, "health" | "mana" | "gold" | "experience") && !has_target {
            return Ok(None);
        }
        let result = match action {
            "health" => ("Health restored", CheatMenuAction::RestoreHealth { player_id }),
            "mana" => ("Mana restored", CheatMenuAction::RestoreMana { player_id }),
            "gold" => (
                "Gold set",
                CheatMenuAction::SetGold {
                    player_id,
                    gold: integer_input(&self.gold.clone(), "Gold", 0.0, CHEAT_MENU_GOLD_MAX)?,
                },
            ),
            "experience" => (
                "Experience granted",
                CheatMenuAction::GrantExperience {
                    player_id,
                    amount: integer_input(
                        &self.experience.clone(),
                        "Experience",
                        0.0,
                        CHEAT_MENU_EXPERIENCE_MAX,
                    )?,
                },
            ),
            "seed" if frame.hub_seed_available => (
                "Next run seed set",
                CheatMenuAction::SetRunSeed {
                    seed: integer_input(
                        &self.seed.clone(),
                        "Run seed",
                        1.0,
                        CHEAT_MENU_RUN_SEED_MAX,
                    )?,
                },
            ),
            "next-wave" if frame.next_wave_spawn_available => {
                ("Next wave spawn queued", CheatMenuAction::SpawnNextWave)
            }
            "spawn" if frame.enemy_spawn_available && !self.enemy.clone().is_empty() => (
                "Enemy spawn queued",
                CheatMenuAction::SpawnEnemy {
                    player_id,
                    enemy_key: self.enemy.clone(),
                    count: integer_input(
                        &self.count.clone(),
                        "Enemy count",
                        1.0,
                        CHEAT_MENU_SPAWN_COUNT_MAX,
                    )?,
                },
            ),
            _ => {
                if !frame.developer {
                    return Ok(None);
                }
                let Some(developer) = &self.developer else { return Ok(None) };
                let result = match action {
                    "item" if !developer.item.clone().is_empty() => (
                        "Item granted",
                        CheatMenuAction::GrantItem {
                            player_id,
                            item_key: developer.item.clone(),
                            quantity: integer_input(
                                &developer.quantity.clone(),
                                "Item quantity",
                                1.0,
                                100.0,
                            )?,
                        },
                    ),
                    "skill" if !developer.skill.clone().is_empty() => {
                        let maximum = catalogs.map_or(1.0, |catalogs| {
                            skill_maximum(catalogs, &developer.skill.clone()) as f64
                        });
                        (
                            "Skill granted",
                            CheatMenuAction::GrantSkill {
                                player_id,
                                ranks: integer_input(
                                    &developer.ranks.clone(),
                                    "Skill ranks",
                                    1.0,
                                    maximum,
                                )?,
                                skill_id: integer_input(
                                    &developer.skill.clone(),
                                    "Skill id",
                                    8.0,
                                    79.0,
                                )?,
                            },
                        )
                    }
                    "weld" if !developer.weld.clone().is_empty() => (
                        "Weld granted",
                        CheatMenuAction::GrantWeld {
                            player_id,
                            build_id: integer_input(
                                &developer.weld.clone(),
                                "Weld build id",
                                1000.0,
                                1009.0,
                            )?,
                        },
                    ),
                    "bot" if frame.bot_summon_available => {
                        let discipline = WizardDiscipline::CHOICES
                            .into_iter()
                            .find(|discipline| discipline.as_str() == developer.discipline.clone())
                            .ok_or("Invalid bot discipline.")?;
                        let element = WizardElement::ALL
                            .into_iter()
                            .find(|element| element.as_str() == developer.element.clone())
                            .ok_or("Invalid bot element.")?;
                        ("Bot summoned", CheatMenuAction::SummonBot { discipline, element })
                    }
                    _ => return Ok(None),
                };
                return Ok(Some(result));
            }
        };
        Ok(Some(result))
    }
}

fn skill_maximum(catalogs: &CheatMenuCatalogs, id: &str) -> i64 {
    catalogs
        .skills
        .iter()
        .find(|skill| !skill.weld_only && skill.id.to_string() == id)
        .map_or(1, |skill| skill.maximum_rank)
}

fn integer_input(value: &str, label: &str, minimum: f64, maximum: f64) -> Result<f64, String> {
    // Number(value), including the source's empty-string-to-zero conversion.
    let number = crate::ui::parse_javascript_number(value);
    if !number.is_finite()
        || number.fract() != 0.0
        || number.abs() > 9_007_199_254_740_991.0
        || number < minimum
        || number > maximum
    {
        return Err(format!("{label} must be within {minimum}..{maximum}."));
    }
    Ok(number)
}

fn display_identity(value: &str) -> String {
    let mut result = String::new();
    let mut previous_word = false;
    for character in value.chars() {
        let character = if character == '_' { ' ' } else { character };
        let word = character.is_ascii_alphanumeric();
        result.push(if word && !previous_word {
            character.to_ascii_uppercase()
        } else {
            character
        });
        previous_word = word;
    }
    result
}

fn retain_choice(value: &mut String, rows: &[(String, String)]) {
    if !rows.iter().any(|(id, _)| id == value) {
        *value = rows.first().map(|(id, _)| id.clone()).unwrap_or_default();
    }
}
fn select(
    gpu: &UiFrame<'_>,
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    value: &mut String,
    rows: &[(String, String)],
) -> bool {
    ui.label(copy(label, 11.0));
    let old = value.clone();
    let selected = rows.iter().find(|(key, _)| key == value).map_or("", |(_, name)| name.as_str());
    let response = egui::ComboBox::from_id_salt(("cheats", id))
        .width(ui.available_width())
        .selected_text(copy(selected, 13.0))
        .show_ui(ui, |ui| {
            for (key, name) in rows {
                let response = ui.selectable_value(value, key.clone(), copy(name, 13.0));
                gpu.record(&format!("cheats/{id}/option:{key}"), name, "option", &response);
            }
        })
        .response;
    gpu.record(&format!("cheats/{id}"), label, "combobox", &response);
    gpu.record_value(&format!("cheats/{id}"), value, None);
    old != *value
}
fn field(gpu: &UiFrame<'_>, ui: &mut egui::Ui, id: &str, label: &str, value: &mut String) -> bool {
    ui.label(copy(label, 11.0));
    let (_, rect) = ui.allocate_space(egui::vec2(ui.available_width(), 26.0));
    let stable_id = format!("cheats/{id}");
    crate::gpu_ui::NativeTextEdit::singleline(value, &stable_id, label)
        .font(FontId::new(13.0, FontFamily::Name("Cinzel Bold".into())))
        .color(Color32::from_rgb(243, 226, 169))
        .background(Color32::from_rgb(10, 9, 7))
        .show(gpu, ui, rect)
        .submitted
}
fn inline(
    gpu: &UiFrame<'_>,
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    value: &mut String,
    action: &str,
    enabled: bool,
) -> bool {
    let submit = field(gpu, ui, id, label, value);
    let response = button(gpu, ui, &format!("submit:{id}"), action, enabled);
    enabled && (response.clicked() || submit)
}
fn group(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(8, 7, 5, 180))
        .stroke(Stroke::new(1.0, Color32::from_rgb(109, 91, 51)))
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.label(copy(title, 15.0));
            ui.separator();
            body(ui);
        });
}
fn hint(ui: &mut egui::Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .font(FontId::new(12.0, FontFamily::Name("Alegreya".into())))
            .color(Color32::from_rgb(169, 156, 118)),
    );
}
