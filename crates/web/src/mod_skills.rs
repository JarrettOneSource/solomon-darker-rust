//! Source `mod-ui/ModSkillBook.tsx` and `ModSkillPicker.tsx`. Both surfaces borrow
//! the accepted runtime and the session's already verified image URLs.

use std::cell::RefCell;
use std::rc::Rc;

use solomon_sim::modding::LuaObject;
use solomon_sim::modding::assets::GameModAsset;
use solomon_sim::modding::ui::{ModSkillIcon, project_mod_skill_book, project_mod_skill_picker};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{Document, Element, Event, HtmlElement};

use crate::mod_presentation_assets::ModPresentationAssets;
use crate::ui::{append, element, javascript_string};

pub struct ModSkillsSnapshot<'a> {
    pub runtime: Option<&'a LuaObject>,
    pub player_id: &'a str,
    /// Source mounts the learned panel inside the actual native Skills surface.
    pub skills_open: bool,
    /// Source mounts the mod offer panel only beside an actual native offer.
    pub native_offer_sequence: Option<u64>,
    pub assets: &'a [GameModAsset],
    pub images: Option<&'a ModPresentationAssets>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ModSkillAction {
    QuickbarBind { content_id: String, slot: f64, clear: bool },
    SkillChoose { content_id: String, mod_offer_sequence: f64, native_offer_sequence: u64 },
}

#[derive(Clone, PartialEq)]
struct SkillRow {
    key: String,
    tag: &'static str,
    name: String,
    detail: Option<String>,
    description: Option<String>,
    icon_url: Option<String>,
    subskill: bool,
    disabled: bool,
    action: Option<ModSkillAction>,
}

impl SkillRow {
    fn text(key: &str, tag: &'static str, name: &str) -> Self {
        Self {
            key: key.to_owned(),
            tag,
            name: name.to_owned(),
            detail: None,
            description: None,
            icon_url: None,
            subskill: false,
            disabled: false,
            action: None,
        }
    }
}

struct RenderedRow {
    content: SkillRow,
    element: HtmlElement,
}

struct Surface {
    root: HtmlElement,
    rows: Vec<RenderedRow>,
}

pub struct ModSkillsUi {
    document: Document,
    book: Surface,
    picker: HtmlElement,
    choices: Surface,
    actions: Rc<RefCell<Vec<ModSkillAction>>>,
    current: Rc<RefCell<Vec<(String, ModSkillAction)>>>,
    click: Closure<dyn FnMut(Event)>,
}

impl ModSkillsUi {
    pub fn new(document: &Document) -> Result<Self, String> {
        let book = element(document, "aside", "mod-skill-book")?;
        book.set_attribute("aria-label", "Mod skills").map_err(|_| "Mod skills label")?;
        let picker = element(document, "aside", "mod-skill-picker")?;
        picker
            .set_attribute("aria-label", "Mod skill choices")
            .map_err(|_| "Mod skill choices label")?;
        let heading = element(document, "h2", "")?;
        heading.set_text_content(Some("Mod skills"));
        append(&picker, &heading)?;
        let choices = element(document, "div", "mod-skill-picker__choices")?;
        append(&picker, &choices)?;
        for (root, source) in [
            (&book, include_str!("../../../web/mod-skill-book.css")),
            (&picker, include_str!("../../../web/mod-skill-picker.css")),
        ] {
            root.set_hidden(true);
            let css = element(document, "style", "mod-skill-styles")?;
            css.set_text_content(Some(source));
            append(root, &css)?;
            append(&document.body().ok_or("Mod skills need the game document")?, root)?;
        }
        let actions = Rc::new(RefCell::new(Vec::new()));
        let current: Rc<RefCell<Vec<(String, ModSkillAction)>>> = Rc::new(RefCell::new(Vec::new()));
        let outbox = actions.clone();
        let selected = current.clone();
        let click = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let button = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
                .and_then(|target| {
                    target.closest("[data-mod-skill-action]").expect("valid mod skill selector")
                });
            let Some(button) = button else { return };
            if button.has_attribute("disabled") {
                return;
            }
            let Some(key) = button.get_attribute("data-mod-skill-action") else { return };
            if let Some((_, action)) = selected.borrow().iter().find(|(id, _)| id == &key) {
                outbox.borrow_mut().push(action.clone());
            }
        });
        for root in [&book, &picker] {
            root.add_event_listener_with_callback("click", click.as_ref().unchecked_ref())
                .map_err(|_| "Could not listen to mod skills")?;
        }
        Ok(Self {
            document: document.clone(),
            book: Surface { root: book, rows: Vec::new() },
            picker,
            choices: Surface { root: choices, rows: Vec::new() },
            actions,
            current,
            click,
        })
    }

    pub fn update(&mut self, snapshot: &ModSkillsSnapshot<'_>) -> Result<(), String> {
        let mut book_rows = Vec::new();
        if snapshot.skills_open
            && let Some(book) = project_mod_skill_book(snapshot.runtime)
        {
            book_rows.push(SkillRow::text("heading", "h2", "Mod skills"));
            if book.learned.is_empty() {
                book_rows.push(SkillRow::text("empty", "p", "No mod skills learned yet."));
            }
            for skill in book.learned {
                let mut row =
                    SkillRow::text(&format!("learned:{}", skill.content_id), "article", skill.name);
                row.detail =
                    Some(format!("Rank {} / {}", number(skill.rank), number(skill.maximum_rank)));
                row.description = Some(skill.description.to_owned());
                row.icon_url = icon_url(snapshot, skill.icon)?;
                row.subskill = skill.subskill;
                book_rows.push(row);
            }
            if !book.spells.is_empty() {
                book_rows.push(SkillRow::text("spells", "h3", "Unlocked spells"));
            }
            for spell in book.spells {
                let mut row =
                    SkillRow::text(&format!("binding:{}", spell.content_id), "button", spell.name);
                let slot = spell.slot.map_or_else(|| "NaN".to_owned(), |slot| number(slot + 1.0));
                row.description = Some(if spell.bound {
                    format!("Remove from slot {slot}")
                } else {
                    format!("Add to slot {slot}")
                });
                row.icon_url = icon_url(snapshot, spell.icon)?;
                row.disabled = spell.slot.is_none();
                row.action = spell.slot.map(|slot| ModSkillAction::QuickbarBind {
                    content_id: spell.content_id.to_owned(),
                    slot,
                    clear: spell.bound,
                });
                book_rows.push(row);
            }
        }
        let mut choice_rows = Vec::new();
        if let Some(native_offer_sequence) = snapshot.native_offer_sequence
            && let Some(offer) = project_mod_skill_picker(snapshot.runtime, snapshot.player_id)
        {
            for skill in offer.skills {
                let mut row =
                    SkillRow::text(&format!("offer:{}", skill.content_id), "button", skill.name);
                row.description = Some(if skill.description.is_empty() {
                    format!("Rank {}", number(skill.next_rank))
                } else {
                    skill.description.to_owned()
                });
                row.icon_url = icon_url(snapshot, skill.icon)?;
                row.action = Some(ModSkillAction::SkillChoose {
                    content_id: skill.content_id.to_owned(),
                    mod_offer_sequence: offer.sequence,
                    native_offer_sequence,
                });
                choice_rows.push(row);
            }
        }
        let mut current = self.current.borrow_mut();
        current.clear();
        for row in book_rows.iter().chain(&choice_rows) {
            if let Some(action) = &row.action {
                current.push((row.key.clone(), action.clone()));
            }
        }
        drop(current);
        self.book.root.set_hidden(book_rows.is_empty());
        self.picker.set_hidden(choice_rows.is_empty());
        update_rows(&self.document, &mut self.book, book_rows, 1)?;
        update_rows(&self.document, &mut self.choices, choice_rows, 0)?;
        Ok(())
    }

    pub fn hide(&mut self) {
        self.book.root.set_hidden(true);
        self.picker.set_hidden(true);
        for row in self.book.rows.drain(..).chain(self.choices.rows.drain(..)) {
            row.element.remove();
        }
        self.current.borrow_mut().clear();
    }

    pub fn reset(&mut self) {
        self.hide();
        self.actions.borrow_mut().clear();
    }

    pub fn take_actions(&mut self) -> Vec<ModSkillAction> {
        std::mem::take(&mut *self.actions.borrow_mut())
    }
}

impl Drop for ModSkillsUi {
    fn drop(&mut self) {
        for root in [&self.book.root, &self.picker] {
            root.remove_event_listener_with_callback("click", self.click.as_ref().unchecked_ref())
                .expect("remove installed mod skills listener");
            root.remove();
        }
    }
}

fn update_rows(
    document: &Document,
    surface: &mut Surface,
    rows: Vec<SkillRow>,
    start: u32,
) -> Result<(), String> {
    let mut retained = Vec::new();
    for (index, content) in rows.into_iter().enumerate() {
        let previous = surface.rows.iter().position(|row| row.content.key == content.key);
        let element = if let Some(previous) = previous {
            let previous = surface.rows.remove(previous);
            if previous.content != content {
                fill_row(document, &previous.element, &content)?;
            }
            previous.element
        } else {
            let row = element(document, content.tag, "")?;
            fill_row(document, &row, &content)?;
            row
        };
        // Keep the button node during host refreshes, so a binding update does
        // not discard keyboard focus or reset the user's scroll position.
        let next = surface.root.children().item(start + index as u32);
        if next.as_ref() != Some(element.as_ref()) {
            surface
                .root
                .insert_before(&element, next.as_ref().map(|next| next.as_ref()))
                .map_err(|_| "Mod skill row order")?;
        }
        retained.push(RenderedRow { content, element });
    }
    for row in surface.rows.drain(..) {
        row.element.remove();
    }
    surface.rows = retained;
    Ok(())
}

fn fill_row(document: &Document, row: &HtmlElement, content: &SkillRow) -> Result<(), String> {
    row.set_text_content(None);
    if content.tag == "article" {
        row.set_attribute("data-subskill", if content.subskill { "true" } else { "false" })
            .map_err(|_| "Mod subskill indentation")?;
    }
    if content.tag == "button" {
        row.set_attribute("type", "button").map_err(|_| "Mod skill button type")?;
        row.set_attribute("data-mod-skill-action", &content.key)
            .map_err(|_| "Mod skill action identity")?;
        row.toggle_attribute_with_force("disabled", content.disabled)
            .map_err(|_| "Mod binding availability")?;
    }
    if let Some(url) = &content.icon_url {
        let image = element(document, "img", "")?;
        image.set_attribute("alt", "").map_err(|_| "Mod skill icon label")?;
        image.set_attribute("src", url).map_err(|_| "Mod skill icon URL")?;
        append(row, &image)?;
    }
    if content.tag == "article" || content.tag == "button" {
        let tag = if content.tag == "article" { "strong" } else { "span" };
        let name = element(document, tag, "")?;
        name.set_text_content(Some(&content.name));
        append(row, &name)?;
    } else {
        row.set_text_content(Some(&content.name));
    }
    for (tag, text) in [("span", &content.detail), ("small", &content.description)] {
        if let Some(text) = text {
            let child = element(document, tag, "")?;
            child.set_text_content(Some(text));
            append(row, &child)?;
        }
    }
    Ok(())
}

fn icon_url(
    snapshot: &ModSkillsSnapshot<'_>,
    icon: ModSkillIcon<'_>,
) -> Result<Option<String>, String> {
    // Source first finds an exact-case image descriptor, then uses the whole
    // verified asset. These skill icons do not apply custom sprite-sheet crops.
    let asset = snapshot.assets.iter().find(|asset| {
        asset.mod_id == icon.mod_id && asset.path == icon.path && asset.kind == "image"
    });
    let Some(asset) = asset else { return Ok(None) };
    let images = snapshot.images.ok_or("Mod skill images are not installed")?;
    Ok(Some(images.image(&asset.mod_id, &asset.path)?.url().to_owned()))
}

fn number(value: f64) -> String {
    javascript_string(&JsValue::from_f64(value))
}
