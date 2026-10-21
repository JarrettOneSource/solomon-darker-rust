//! The source CheatMenu's native HTML forms. Inputs retain their own drafts while
//! the borrowed accepted frame refreshes only roster, statistics and admission.

use solomon_sim::menu::cheats::{
    CHEAT_MENU_EXPERIENCE_MAX, CHEAT_MENU_GOLD_MAX, CHEAT_MENU_RUN_SEED_MAX,
    CHEAT_MENU_SPAWN_COUNT_MAX, CheatMenuAction, CheatMenuCatalogs,
};
use solomon_sim::profile::{WizardDiscipline, WizardElement};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, HtmlElement, HtmlInputElement, HtmlSelectElement};

use super::{CheatFrame, button, text};
use crate::ui::{append, element};

pub(super) struct CheatControls {
    pub root: HtmlElement,
    document: Document,
    target: HtmlSelectElement,
    stats: HtmlElement,
    stat_values: Vec<HtmlElement>,
    selected_target: String,
    roster: Vec<(String, String)>,
    restore_health: HtmlElement,
    restore_mana: HtmlElement,
    gold: HtmlInputElement,
    set_gold: HtmlElement,
    experience: HtmlInputElement,
    grant_experience: HtmlElement,
    seed: HtmlInputElement,
    set_seed: HtmlElement,
    seed_hint: HtmlElement,
    next_wave: HtmlElement,
    next_wave_hint: HtmlElement,
    enemy: HtmlSelectElement,
    count: HtmlInputElement,
    spawn: HtmlElement,
    enemy_hint: HtmlElement,
    developer: Option<DeveloperControls>,
}

struct DeveloperControls {
    root: HtmlElement,
    item: HtmlSelectElement,
    quantity: HtmlInputElement,
    grant_item: HtmlElement,
    skill: HtmlSelectElement,
    ranks: HtmlInputElement,
    grant_skill: HtmlElement,
    weld: HtmlSelectElement,
    grant_weld: HtmlElement,
    discipline: HtmlSelectElement,
    element: HtmlSelectElement,
    summon_bot: HtmlElement,
    bot_hint: HtmlElement,
}

impl CheatControls {
    pub fn new(document: &Document, frame: &CheatFrame) -> Result<Self, String> {
        let root = element(document, "div", "game-settings-content cheat-menu-content")?;
        root.set_id("cheat-menu-cheats");
        root.set_attribute("role", "tabpanel").map_err(|_| "cheat forms role")?;
        root.set_attribute("aria-labelledby", "cheat-menu-cheats-tab")
            .map_err(|_| "cheat forms label")?;
        let target_row = element(document, "section", "cheat-menu-target")?;
        target_row
            .set_attribute("aria-label", "Target player")
            .map_err(|_| "cheat target label")?;
        let target = select(document, &target_row, "TARGET")?;
        target.set_attribute("data-cheat-field", "target").map_err(|_| "cheat target field")?;
        let stats = element(document, "dl", "")?;
        let mut stat_values = Vec::new();
        for label in ["HP", "MP", "LEVEL / XP", "GOLD", "STATE", "POSITION"] {
            let row = element(document, "div", "")?;
            text(document, &row, "dt", "", label)?;
            stat_values.push(text(document, &row, "dd", "", "")?);
            append(&stats, &row)?;
        }
        append(&target_row, &stats)?;
        append(&root, &target_row)?;

        let columns = element(document, "div", "cheat-menu-columns")?;
        let (player_group, player_body) = group(document, "PLAYER")?;
        let pair = element(document, "div", "cheat-menu-button-pair")?;
        let restore_health = button(document, "RESTORE HEALTH", "health", "cheat-menu-action")?;
        let restore_mana = button(document, "RESTORE MANA", "mana", "cheat-menu-action")?;
        append(&pair, &restore_health)?;
        append(&pair, &restore_mana)?;
        append(&player_body, &pair)?;
        let (gold, set_gold) = inline_action(
            document,
            &player_body,
            "GOLD",
            "SET GOLD",
            "gold",
            "10000",
            (0.0, CHEAT_MENU_GOLD_MAX),
        )?;
        let (experience, grant_experience) = inline_action(
            document,
            &player_body,
            "EXPERIENCE",
            "GRANT XP",
            "experience",
            "1000",
            (0.0, CHEAT_MENU_EXPERIENCE_MAX),
        )?;
        append(&columns, &player_group)?;
        let (world_group, world_body) = group(document, "WORLD")?;
        let next_wave = button(document, "NEXT WAVE SPAWN", "next-wave", "cheat-menu-action")?;
        append(&world_body, &next_wave)?;
        let next_wave_hint = text(
            document,
            &world_body,
            "small",
            "",
            "Available between spawns in an active survival Boneyard.",
        )?;
        let (seed, set_seed) = inline_action(
            document,
            &world_body,
            "RUN SEED",
            "SET NEXT SEED",
            "seed",
            "42",
            (1.0, CHEAT_MENU_RUN_SEED_MAX),
        )?;
        let seed_hint =
            text(document, &world_body, "small", "", "Run seed is available in the Hub.")?;
        let spawn_form = form(document, "cheat-menu-catalog-action", "spawn")?;
        let enemy = select(document, &spawn_form, "ENEMY")?;
        let count =
            number(document, &spawn_form, "COUNT", "1", 1.0, CHEAT_MENU_SPAWN_COUNT_MAX, true)?;
        let spawn = submit_button(document, &spawn_form, "SPAWN")?;
        append(&world_body, &spawn_form)?;
        let enemy_hint = text(
            document,
            &world_body,
            "small",
            "",
            "Enemy spawning requires an active Boneyard.",
        )?;
        append(&columns, &world_group)?;
        append(&root, &columns)?;
        let developer = if frame.developer {
            let controls = DeveloperControls::new(document)?;
            append(&root, &controls.root)?;
            Some(controls)
        } else {
            None
        };
        let mut controls = Self {
            root,
            document: document.clone(),
            target,
            stats,
            stat_values,
            selected_target: String::new(),
            roster: Vec::new(),
            restore_health,
            restore_mana,
            gold,
            set_gold,
            experience,
            grant_experience,
            seed,
            set_seed,
            seed_hint,
            next_wave,
            next_wave_hint,
            enemy,
            count,
            spawn,
            enemy_hint,
            developer,
        };
        controls.update(frame, false, None)?;
        Ok(controls)
    }

    pub fn update(
        &mut self,
        frame: &CheatFrame,
        busy: bool,
        catalogs: Option<&CheatMenuCatalogs>,
    ) -> Result<(), String> {
        let roster: Vec<_> =
            frame.players.iter().map(|player| (player.id.clone(), player.name.clone())).collect();
        if roster != self.roster {
            let previous = self.target.value();
            options(&self.document, &self.target, &roster)?;
            if roster.iter().any(|(id, _)| *id == previous) {
                self.target.set_value(&previous);
            } else if roster.iter().any(|(id, _)| *id == frame.local_player_id) {
                self.target.set_value(&frame.local_player_id);
            }
            self.roster = roster;
        }
        if self.target.value() != self.selected_target {
            self.target_changed(frame);
            self.selected_target = self.target.value();
        }
        let target = frame.players.iter().find(|player| player.id == self.target.value());
        self.stats.set_hidden(target.is_none());
        if let Some(target) = target {
            for (node, value) in self.stat_values.iter().zip(&target.stats) {
                if node.text_content().as_deref() != Some(value) {
                    node.set_text_content(Some(value));
                }
            }
        }
        for button in
            [&self.restore_health, &self.restore_mana, &self.set_gold, &self.grant_experience]
        {
            disabled(button, busy || target.is_none())?;
        }
        disabled(&self.set_seed, busy || !frame.hub_seed_available)?;
        self.seed_hint.set_hidden(frame.hub_seed_available);
        disabled(&self.next_wave, busy || !frame.next_wave_spawn_available)?;
        self.next_wave_hint.set_hidden(frame.next_wave_spawn_available);
        disabled(
            &self.spawn,
            busy || !frame.enemy_spawn_available || self.enemy.value().is_empty(),
        )?;
        self.enemy_hint.set_hidden(frame.enemy_spawn_available);
        if !frame.developer {
            if let Some(controls) = self.developer.take() {
                controls.root.remove();
            }
        } else if self.developer.is_none() {
            let controls = DeveloperControls::new(&self.document)?;
            // Source places developer grants before asynchronous catalog receipts.
            let receipt = self
                .root
                .query_selector(".cheat-menu-receipt")
                .map_err(|_| "cheat receipt location")?;
            self.root
                .insert_before(&controls.root, receipt.as_ref().map(|node| node.as_ref()))
                .map_err(|_| "cheat developer group")?;
            self.developer = Some(controls);
            if let Some(catalogs) = catalogs {
                self.set_catalogs(&self.document.clone(), catalogs)?;
            }
        }
        if let Some(controls) = &self.developer {
            disabled(&controls.grant_item, busy || controls.item.value().is_empty())?;
            disabled(&controls.grant_skill, busy || controls.skill.value().is_empty())?;
            disabled(&controls.grant_weld, busy || controls.weld.value().is_empty())?;
            disabled(&controls.summon_bot, busy || !frame.bot_summon_available)?;
            controls.bot_hint.set_hidden(frame.bot_summon_available);
        }
        Ok(())
    }

    pub fn set_catalogs(
        &self,
        document: &Document,
        catalogs: &CheatMenuCatalogs,
    ) -> Result<(), String> {
        let rows = catalogs
            .enemies
            .iter()
            .map(|enemy| (enemy.key.clone(), display_identity(&enemy.base)))
            .collect::<Vec<_>>();
        options(document, &self.enemy, &rows)?;
        if let Some(developer) = &self.developer {
            let rows = catalogs
                .items
                .iter()
                .map(|item| (item.key.clone(), item.name.clone()))
                .collect::<Vec<_>>();
            options(document, &developer.item, &rows)?;
            let rows = catalogs
                .skills
                .iter()
                .filter(|skill| !skill.weld_only)
                .map(|skill| {
                    (
                        skill.id.to_string(),
                        format!("{} · {}", skill.name, display_identity(&skill.family)),
                    )
                })
                .collect::<Vec<_>>();
            options(document, &developer.skill, &rows)?;
            let rows = catalogs
                .welds
                .iter()
                .map(|weld| (weld.id.to_string(), weld.name.clone()))
                .collect::<Vec<_>>();
            options(document, &developer.weld, &rows)?;
            self.skill_changed(catalogs)?;
        }
        Ok(())
    }

    pub fn target_changed(&self, frame: &CheatFrame) {
        if let Some(player) = frame.players.iter().find(|player| player.id == self.target.value()) {
            self.gold.set_value(&player.gold.to_string());
        }
    }

    pub fn skill_changed(&self, catalogs: &CheatMenuCatalogs) -> Result<(), String> {
        if let Some(developer) = &self.developer {
            developer.ranks.set_max(&skill_maximum(catalogs, &developer.skill.value()).to_string());
        }
        Ok(())
    }

    pub fn action(
        &self,
        action: &str,
        frame: &CheatFrame,
        catalogs: Option<&CheatMenuCatalogs>,
    ) -> Result<Option<(&'static str, CheatMenuAction)>, String> {
        let player_id = self.target.value();
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
                    gold: integer_input(&self.gold.value(), "Gold", 0.0, CHEAT_MENU_GOLD_MAX)?,
                },
            ),
            "experience" => (
                "Experience granted",
                CheatMenuAction::GrantExperience {
                    player_id,
                    amount: integer_input(
                        &self.experience.value(),
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
                        &self.seed.value(),
                        "Run seed",
                        1.0,
                        CHEAT_MENU_RUN_SEED_MAX,
                    )?,
                },
            ),
            "next-wave" if frame.next_wave_spawn_available => {
                ("Next wave spawn queued", CheatMenuAction::SpawnNextWave)
            }
            "spawn" if frame.enemy_spawn_available && !self.enemy.value().is_empty() => (
                "Enemy spawn queued",
                CheatMenuAction::SpawnEnemy {
                    player_id,
                    enemy_key: self.enemy.value(),
                    count: integer_input(
                        &self.count.value(),
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
                    "item" if !developer.item.value().is_empty() => (
                        "Item granted",
                        CheatMenuAction::GrantItem {
                            player_id,
                            item_key: developer.item.value(),
                            quantity: integer_input(
                                &developer.quantity.value(),
                                "Item quantity",
                                1.0,
                                100.0,
                            )?,
                        },
                    ),
                    "skill" if !developer.skill.value().is_empty() => {
                        let maximum = catalogs.map_or(1.0, |catalogs| {
                            skill_maximum(catalogs, &developer.skill.value()) as f64
                        });
                        (
                            "Skill granted",
                            CheatMenuAction::GrantSkill {
                                player_id,
                                ranks: integer_input(
                                    &developer.ranks.value(),
                                    "Skill ranks",
                                    1.0,
                                    maximum,
                                )?,
                                skill_id: integer_input(
                                    &developer.skill.value(),
                                    "Skill id",
                                    8.0,
                                    79.0,
                                )?,
                            },
                        )
                    }
                    "weld" if !developer.weld.value().is_empty() => (
                        "Weld granted",
                        CheatMenuAction::GrantWeld {
                            player_id,
                            build_id: integer_input(
                                &developer.weld.value(),
                                "Weld build id",
                                1000.0,
                                1009.0,
                            )?,
                        },
                    ),
                    "bot" if frame.bot_summon_available => {
                        let discipline = WizardDiscipline::CHOICES
                            .into_iter()
                            .find(|discipline| discipline.as_str() == developer.discipline.value())
                            .ok_or("Invalid bot discipline.")?;
                        let element = WizardElement::ALL
                            .into_iter()
                            .find(|element| element.as_str() == developer.element.value())
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

impl DeveloperControls {
    fn new(document: &Document) -> Result<Self, String> {
        let (root, body) = group(document, "DEVELOPER GRANTS")?;
        let grid = element(document, "div", "cheat-menu-developer-grid")?;
        let item_form = form(document, "cheat-menu-catalog-action", "item")?;
        let item = select(document, &item_form, "ITEM")?;
        let quantity = number(document, &item_form, "QTY", "1", 1.0, 100.0, true)?;
        let grant_item = submit_button(document, &item_form, "GRANT ITEM")?;
        append(&grid, &item_form)?;
        let skill_form = form(document, "cheat-menu-catalog-action", "skill")?;
        let skill = select(document, &skill_form, "SKILL")?;
        skill.set_attribute("data-cheat-field", "skill").map_err(|_| "cheat skill field")?;
        let ranks = number(document, &skill_form, "RANKS", "1", 1.0, 1.0, true)?;
        let grant_skill = submit_button(document, &skill_form, "GRANT SKILL")?;
        append(&grid, &skill_form)?;
        let weld_form = form(document, "cheat-menu-catalog-action", "weld")?;
        let weld = select(document, &weld_form, "WELD")?;
        let grant_weld = submit_button(document, &weld_form, "GRANT WELD")?;
        append(&grid, &weld_form)?;
        let bot_form = form(document, "cheat-menu-catalog-action", "bot")?;
        let discipline = select(document, &bot_form, "BOT DISCIPLINE")?;
        options(
            document,
            &discipline,
            &WizardDiscipline::CHOICES
                .iter()
                .map(|discipline| {
                    (discipline.as_str().to_owned(), discipline.as_str().to_ascii_uppercase())
                })
                .collect::<Vec<_>>(),
        )?;
        discipline.set_value("arcane");
        let element = select(document, &bot_form, "ELEMENT")?;
        options(
            document,
            &element,
            &WizardElement::ALL
                .iter()
                .map(|element| (element.as_str().to_owned(), element.as_str().to_ascii_uppercase()))
                .collect::<Vec<_>>(),
        )?;
        element.set_value("fire");
        let summon_bot = submit_button(document, &bot_form, "SUMMON BOT")?;
        append(&grid, &bot_form)?;
        append(&body, &grid)?;
        let bot_hint =
            text(document, &root, "small", "", "Bots require developer access in the shared Hub.")?;
        Ok(Self {
            root,
            item,
            quantity,
            grant_item,
            skill,
            ranks,
            grant_skill,
            weld,
            grant_weld,
            discipline,
            element,
            summon_bot,
            bot_hint,
        })
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
    let parsed = value.parse::<js_sys::Number>().unwrap_or_else(|never| match never {});
    let number = parsed.value_of();
    if !js_sys::Number::is_safe_integer(&JsValue::from_f64(number))
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

fn disabled(button: &HtmlElement, disabled: bool) -> Result<(), String> {
    button
        .toggle_attribute_with_force("disabled", disabled)
        .map(|_| ())
        .map_err(|_| "cheat control admission".to_owned())
}

fn group(document: &Document, title: &str) -> Result<(HtmlElement, HtmlElement), String> {
    let root = element(document, "section", "game-settings-group cheat-menu-group")?;
    root.set_attribute("aria-label", title).map_err(|_| "cheat group description")?;
    text(document, &root, "h3", "", title)?;
    let body = element(document, "div", "")?;
    append(&root, &body)?;
    Ok((root, body))
}

fn form(document: &Document, class: &str, action: &str) -> Result<HtmlElement, String> {
    let form = element(document, "form", class)?;
    form.set_attribute("data-cheat-submit", action).map_err(|_| "cheat form action")?;
    Ok(form)
}

fn inline_action(
    document: &Document,
    parent: &HtmlElement,
    label: &str,
    action_label: &str,
    action: &str,
    value: &str,
    range: (f64, f64),
) -> Result<(HtmlInputElement, HtmlElement), String> {
    let form = form(document, "cheat-menu-inline-action", action)?;
    let input = number(document, &form, label, value, range.0, range.1, false)?;
    let button = submit_button(document, &form, action_label)?;
    append(parent, &form)?;
    Ok((input, button))
}

fn select(
    document: &Document,
    parent: &HtmlElement,
    label: &str,
) -> Result<HtmlSelectElement, String> {
    let wrapper = element(document, "label", "")?;
    text(document, &wrapper, "span", "", label)?;
    let node = element(document, "select", "")?;
    node.set_attribute("aria-label", label).map_err(|_| "cheat selection label")?;
    append(&wrapper, &node)?;
    append(parent, &wrapper)?;
    node.dyn_into().map_err(|_| "cheat selection is not a select".to_owned())
}

fn number(
    document: &Document,
    parent: &HtmlElement,
    label: &str,
    value: &str,
    minimum: f64,
    maximum: f64,
    small: bool,
) -> Result<HtmlInputElement, String> {
    let wrapper = element(document, "label", if small { "cheat-menu-small-number" } else { "" })?;
    text(document, &wrapper, "span", "", label)?;
    let node = element(document, "input", "")?;
    node.set_attribute("aria-label", label).map_err(|_| "cheat number label")?;
    let input: HtmlInputElement = node.dyn_into().map_err(|_| "cheat number is not an input")?;
    input.set_type("number");
    input.set_min(&minimum.to_string());
    input.set_max(&maximum.to_string());
    input.set_value(value);
    append(&wrapper, input.unchecked_ref())?;
    append(parent, &wrapper)?;
    Ok(input)
}

fn submit_button(
    document: &Document,
    parent: &HtmlElement,
    label: &str,
) -> Result<HtmlElement, String> {
    let button = element(document, "button", "")?;
    button.set_attribute("type", "submit").map_err(|_| "cheat form submit type")?;
    button.set_text_content(Some(label));
    append(parent, &button)?;
    Ok(button)
}

fn options(
    document: &Document,
    select: &HtmlSelectElement,
    rows: &[(String, String)],
) -> Result<(), String> {
    select.set_text_content(None);
    for (value, label) in rows {
        let option = element(document, "option", "")?;
        option.set_attribute("value", value).map_err(|_| "cheat option identity")?;
        option.set_text_content(Some(label));
        select.append_child(&option).map_err(|_| "cheat catalog option")?;
    }
    Ok(())
}
