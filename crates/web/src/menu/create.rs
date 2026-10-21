//! Character creation art from Website `renderer/create-menu-renderer.ts`.

use std::cell::RefCell;
use std::rc::Rc;

use solomon_sim::element_vfx::{self, ElementVfxBlend, ElementVfxDraw};
use solomon_sim::menu::create_name::{
    finalize_create_wizard_name, layout_create_wizard_name, validate_create_wizard_name,
};
use solomon_sim::menu::{self, CreateHandPose};
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextLayout};
use solomon_sim::profile::create::NATIVE_CREATE_ENTRY_FADE_MS;
use solomon_sim::profile::{WizardDiscipline, WizardElement};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{Document, Event, HtmlElement, HtmlInputElement};

use crate::ui::text::paint_text_layout;
use crate::ui::{StockAtlas, append, element, image, style};

pub struct CreateView {
    pub root: HtmlElement,
    pub name: HtmlInputElement,
    pub message: HtmlElement,
    name_glyphs: HtmlElement,
    accepted_name: Rc<RefCell<String>>,
    _name_listener: Closure<dyn FnMut(Event)>,
    action_stage: HtmlElement,
    name_stage: HtmlElement,
    dice_stage: HtmlElement,
    randomize_loadout: HtmlElement,
    wheel: HtmlElement,
    left_hand: HtmlElement,
    right_hand: HtmlElement,
    elements: Vec<(WizardElement, HtmlElement, VfxView)>,
    disciplines: Vec<(WizardDiscipline, HtmlElement)>,
    held: VfxView,
    element_prompt: HtmlElement,
    discipline_prompt: HtmlElement,
    stars: Vec<HtmlElement>,
    bad_guys: StockAtlas,
    last_tick: Option<u64>,
    document: Document,
}

impl CreateView {
    pub async fn new(document: &Document) -> Result<Self, String> {
        let create = StockAtlas::load("Create").await?;
        let ui = StockAtlas::load("UI").await?;
        let bad_guys = StockAtlas::load("BadGuys").await?;
        install_tints(document)?;
        let root = element(document, "section", "menu-screen create-screen")?;
        root.set_attribute("aria-label", "New wizard loadout selection")
            .map_err(|_| "Create label")?;
        let action_stage = element(document, "div", "create-action-stage")?;
        let wheel = create.sprite(document, 7, "create-wheel")?;
        append(&action_stage, &wheel)?;
        let mut stars = Vec::new();
        for index in 0..50 {
            let star =
                create.sprite(document, if index % 5 == 0 { 4 } else { 8 }, "create-star")?;
            append(&action_stage, &star)?;
            stars.push(star);
        }
        let left_hand = image(document, "create-hand-raised", "create-hand create-hand-left")?;
        let right_hand = image(document, "create-hand-fist", "create-hand create-hand-right")?;
        append(&action_stage, &left_hand)?;
        let mut elements = Vec::new();
        for (element_id, record) in [
            (WizardElement::Earth, 13),
            (WizardElement::Ether, 9),
            (WizardElement::Fire, 10),
            (WizardElement::Water, 12),
            (WizardElement::Air, 11),
        ] {
            let button = action_button(
                document,
                element_id.as_str(),
                &format!("element:{}", element_id.as_str()),
                "create-element",
            )?;
            button.set_hidden(true);
            if element_id == WizardElement::Earth {
                button
                    .set_attribute("data-game-default-focus", "true")
                    .map_err(|_| "preferred element")?;
            }
            let vfx = VfxView::new(document)?;
            append(&button, &vfx.root)?;
            append(&button, &create.sprite(document, record, "create-element-glyph")?)?;
            let record = create.record(record)?;
            button
                .style()
                .set_property("width", &format!("{}px", record.width))
                .map_err(|_| "element width")?;
            button
                .style()
                .set_property("height", &format!("{}px", record.height))
                .map_err(|_| "element height")?;
            append(&action_stage, &button)?;
            elements.push((element_id, button, vfx));
        }
        let held = VfxView::new(document)?;
        held.root.set_class_name("create-held-vfx");
        append(&action_stage, &held.root)?;
        append(&action_stage, &right_hand)?;
        let mut disciplines = Vec::new();
        for (discipline, record) in [
            (WizardDiscipline::Arcane, 0),
            (WizardDiscipline::Body, 1),
            (WizardDiscipline::Mind, 5),
        ] {
            let button = action_button(
                document,
                discipline.as_str(),
                &format!("discipline:{}", discipline.as_str()),
                "create-discipline",
            )?;
            button.set_hidden(true);
            if discipline == WizardDiscipline::Arcane {
                button
                    .set_attribute("data-game-default-focus", "true")
                    .map_err(|_| "preferred discipline")?;
            }
            append(&button, &create.sprite(document, record, "create-discipline-glyph")?)?;
            append(&action_stage, &button)?;
            disciplines.push((discipline, button));
        }
        let element_prompt = create.sprite(document, 3, "create-prompt")?;
        let discipline_prompt = create.sprite(document, 2, "create-prompt")?;
        append(&action_stage, &element_prompt)?;
        append(&action_stage, &discipline_prompt)?;
        append(&root, &action_stage)?;
        let name_stage = element(document, "div", "create-name-stage")?;
        let name_frame = element(document, "div", "create-name-frame")?;
        let field = element(document, "span", "create-name-field")?;
        append(&name_frame, &field)?;
        let record = ui.record(80)?;
        let rail = ui.slice(
            document,
            80,
            [f64::from(record.width - 1), 0.0, 1.0, 76.0],
            [72.0, 0.0, 340.0, 76.0],
        )?;
        append(&name_frame, &rail)?;
        append(&name_frame, &ui.sprite(document, 80, "create-name-end-left")?)?;
        append(&name_frame, &ui.sprite(document, 80, "create-name-end-right")?)?;
        append(&name_frame, &image(document, "create-text-name-caption", "create-name-caption")?)?;
        let name_glyphs = element(document, "span", "create-name-value")?;
        append(&name_frame, &name_glyphs)?;
        let name: HtmlInputElement = element(document, "input", "create-name-input")?
            .dyn_into()
            .map_err(|_| "wizard name input")?;
        name.set_type("text");
        name.set_max_length(64);
        name.set_value("Helvidius");
        name.set_attribute("aria-label", "Wizard name").map_err(|_| "wizard name label")?;
        name.set_attribute("autocomplete", "off").map_err(|_| "wizard name autocomplete")?;
        name.set_attribute("spellcheck", "false").map_err(|_| "wizard name spellcheck")?;
        name.set_attribute("autocapitalize", "characters")
            .map_err(|_| "wizard name capitalization")?;
        name.set_attribute("aria-describedby", "create-name-validation")
            .map_err(|_| "wizard name description")?;
        name_frame.append_child(&name).map_err(|_| "wizard name input attachment")?;
        let clear =
            action_button(document, "Clear wizard name", "clear-name", "create-name-clear")?;
        // assets.ts calls this textNameClear, but its retained file is the name caret.
        append(&clear, &image(document, "create-text-name-caret", "create-name-clear-image")?)?;
        append(&name_frame, &clear)?;
        append(&name_stage, &name_frame)?;
        let message = element(document, "div", "create-validation")?;
        message.set_attribute("role", "status").map_err(|_| "Create validation status")?;
        message.set_id("create-name-validation");
        append(&name_stage, &message)?;
        append(&root, &name_stage)?;
        let back = action_button(document, "Back", "create-back", "create-back")?;
        append(&back, &ui.sprite(document, 42, "")?)?;
        append(&root, &back)?;
        let randomize = action_button(
            document,
            "Randomize wizard name",
            "randomize-name",
            "create-name-randomize",
        )?;
        append(&randomize, &create.sprite(document, 6, "create-name-randomize-art")?)?;
        append(&name_stage, &randomize)?;
        let dice_stage = element(document, "div", "create-dice-stage")?;
        let randomize_loadout = action_button(
            document,
            "Randomize wizard loadout",
            "randomize-loadout",
            "create-randomize",
        )?;
        randomize_loadout
            .set_attribute("title", "Randomize choices and keep your name")
            .map_err(|_| "Create dice description")?;
        randomize_loadout
            .set_attribute("data-game-create-randomize", "true")
            .map_err(|_| "Create dice semantics")?;
        append(&randomize_loadout, &create.sprite(document, 6, "create-randomize-art")?)?;
        append(&dice_stage, &randomize_loadout)?;
        append(&root, &dice_stage)?;
        let accepted_name = Rc::new(RefCell::new(name.value()));
        paint_wizard_name(document, &name, &name_glyphs, &name.value())?;
        let input_name = name.clone();
        let input_glyphs = name_glyphs.clone();
        let input_message = message.clone();
        let input_document = document.clone();
        let input_accepted = accepted_name.clone();
        let name_listener = Closure::<dyn FnMut(Event)>::new(move |_| {
            let next = input_name.value();
            let rejection = validate_create_wizard_name(&next);
            if !next.is_empty()
                && let Some(reason) = rejection
            {
                input_name.set_value(&input_accepted.borrow());
                input_message.set_text_content(Some(reason.message()));
                return;
            }
            *input_accepted.borrow_mut() = next.clone();
            input_message.set_text_content(
                finalize_create_wizard_name(&next).err().map(|reason| reason.message()),
            );
            if let Err(error) =
                paint_wizard_name(&input_document, &input_name, &input_glyphs, &next)
            {
                web_sys::console::error_1(&error.into());
            }
        });
        name.add_event_listener_with_callback("input", name_listener.as_ref().unchecked_ref())
            .map_err(|_| "wizard name edit listener")?;
        Ok(Self {
            root,
            name,
            message,
            name_glyphs,
            accepted_name,
            _name_listener: name_listener,
            action_stage,
            name_stage,
            dice_stage,
            randomize_loadout,
            wheel,
            left_hand,
            right_hand,
            elements,
            disciplines,
            held,
            element_prompt,
            discipline_prompt,
            stars,
            bad_guys,
            last_tick: None,
            document: document.clone(),
        })
    }

    pub fn restart(&mut self) {
        self.last_tick = None;
        self.message.set_text_content(
            finalize_create_wizard_name(&self.name.value()).err().map(|reason| reason.message()),
        );
    }

    pub fn resize(&self, width: f64, height: f64) -> Result<(), String> {
        // A portrait phone crops the side art instead of shrinking the central picker
        // into unreadable labels. Desktop keeps the original complete 1600×900 stage.
        let stage_width = if width < 650.0 { 900.0 } else { 1600.0 };
        let scale = (width / stage_width).min(height / 900.0);
        let name_scale = if width < 650.0 { ((width - 24.0) / 484.0).min(1.0) } else { scale };
        let name_top = if width < 650.0 { 64.0 } else { 0.0 };
        style(
            &self.action_stage,
            &format!(
                "left:{}px;bottom:0;transform:scale({scale});--create-scale:{scale}",
                (width - 1600.0 * scale) / 2.0
            ),
        )?;
        style(
            &self.dice_stage,
            &format!("left:{}px;top:0;transform:scale({scale})", width - 1600.0 * scale),
        )?;
        style(
            &self.name_stage,
            &format!(
                "left:{}px;top:{name_top}px;transform:scale({name_scale})",
                (width - 1600.0 * name_scale) / 2.0
            ),
        )
    }

    pub fn set_name(&mut self, value: &str) -> Result<(), String> {
        if let Some(reason) = validate_create_wizard_name(value)
            && !value.is_empty()
        {
            return Err(reason.message().to_owned());
        }
        self.name.set_value(value);
        *self.accepted_name.borrow_mut() = value.to_owned();
        self.message.set_text_content(
            finalize_create_wizard_name(value).err().map(|reason| reason.message()),
        );
        paint_wizard_name(&self.document, &self.name, &self.name_glyphs, value)
    }

    pub fn finalized_name(&self) -> Result<String, String> {
        let name = self.name.value();
        finalize_create_wizard_name(&name)
            .map(str::to_owned)
            .map_err(|reason| reason.message().to_owned())
    }

    pub fn update(
        &mut self,
        selected: Option<WizardElement>,
        phase_elapsed: f64,
        scene_elapsed: f64,
        reduced_motion: bool,
        finalizing: bool,
    ) -> Result<(), String> {
        let input_ready = scene_elapsed >= NATIVE_CREATE_ENTRY_FADE_MS as f64;
        self.root
            .set_attribute("data-input-ready", if input_ready { "true" } else { "false" })
            .map_err(|_| "Create entry admission")?;
        self.randomize_loadout
            .toggle_attribute_with_force(
                "disabled",
                !input_ready || finalizing || self.finalized_name().is_err(),
            )
            .map_err(|_| "Create dice admission")?;
        let tick = (scene_elapsed / 10.0).floor() as u64;
        if self.last_tick == Some(tick) {
            return Ok(());
        }
        self.last_tick = Some(tick);
        let elapsed = phase_elapsed.min(if selected.is_some() {
            menu::CREATE_SELECTION_ANIMATION_MS
        } else {
            menu::CREATE_ENTRY_ANIMATION_MS
        });
        let motion = if selected.is_some() {
            menu::create_selection_motion_at(elapsed)
        } else {
            menu::create_entry_motion_at(elapsed)
        };
        let idle =
            menu::create_hand_idle_offset_at(if reduced_motion { 0.0 } else { scene_elapsed });
        paint_hand(
            &self.left_hand,
            motion.left_pose,
            400.0 + motion.left_offset.x + motion.left_impulse.x + idle.x,
            560.0 + motion.left_offset.y + motion.left_impulse.y + idle.y,
            false,
        )?;
        paint_hand(
            &self.right_hand,
            motion.right_pose,
            1200.0 + motion.right_offset.x + motion.right_impulse.x + idle.x,
            560.0 + motion.right_offset.y + motion.right_impulse.y + idle.y,
            true,
        )?;
        for (element_id, button, vfx) in &mut self.elements {
            button.set_hidden(selected.is_some() || !motion.elements_visible);
            if selected.is_some() || !motion.elements_visible {
                continue;
            }
            let reveal = menu::create_element_reveal_motion_at(*element_id, elapsed);
            button
                .style()
                .set_property("left", &format!("{}px", reveal.position.x))
                .map_err(|_| "element x")?;
            button
                .style()
                .set_property("top", &format!("{}px", reveal.position.y))
                .map_err(|_| "element y")?;
            button
                .style()
                .set_property("opacity", &reveal.opacity.to_string())
                .map_err(|_| "element reveal")?;
            vfx.update(
                &self.document,
                &self.bad_guys,
                &element_vfx::native_element_vfx_plan(
                    *element_id,
                    if reduced_motion { 0.0 } else { tick as f64 },
                    2.0,
                ),
            )?;
        }
        self.held.root.set_hidden(selected.is_none());
        if let Some(element_id) = selected {
            let held = menu::create_selected_element_motion_at(element_id, elapsed);
            style(
                &self.held.root,
                &format!("left:{}px;top:{}px", held.position.x, held.position.y),
            )?;
            self.held.update(
                &self.document,
                &self.bad_guys,
                &element_vfx::native_element_vfx_plan(
                    element_id,
                    if reduced_motion { 0.0 } else { tick as f64 },
                    held.scale * 2.0,
                ),
            )?;
        }
        for (discipline, button) in &self.disciplines {
            button.set_hidden(selected.is_none() || !motion.disciplines_visible);
            if selected.is_none() || !motion.disciplines_visible {
                continue;
            }
            let position = menu::create_discipline_reveal_motion_at(*discipline, elapsed);
            style(button, &format!("left:{}px;top:{}px", position.x, position.y))?;
        }
        self.element_prompt.set_hidden(selected.is_some() || !motion.elements_visible);
        self.discipline_prompt.set_hidden(selected.is_none() || !motion.disciplines_visible);
        self.wheel
            .style()
            .set_property(
                "transform",
                &format!(
                    "translate(-50%, -50%) scale(3) rotate({}deg)",
                    if reduced_motion { 0.0 } else { scene_elapsed / 1000.0 * 10.0 }
                ),
            )
            .map_err(|_| "Create wheel drift")?;
        self.update_stars(selected.is_some() && !reduced_motion, phase_elapsed)?;
        Ok(())
    }

    fn update_stars(&self, active: bool, elapsed: f64) -> Result<(), String> {
        // `CREATE_STARS` and `createStarPresentation` in create-menu-render-contract.ts.
        for (index, star) in self.stars.iter().enumerate() {
            let index = index as f64;
            let progress =
                (elapsed - (index * 0.067 % 1.25) * 1000.0) / ((2.6 + index * 0.19 % 2.1) * 1000.0);
            star.set_hidden(!active || !(0.0..=1.0).contains(&progress));
            if !active || !(0.0..=1.0).contains(&progress) {
                continue;
            }
            let alpha = if progress < 0.12 {
                0.78 * progress / 0.12
            } else if progress < 0.52 {
                0.78 + (0.34 - 0.78) * (progress - 0.12) / 0.4
            } else {
                0.34 * (1.0 - progress) / 0.48
            };
            let large = (index as usize).is_multiple_of(5);
            let scale = (if large { 23.2 / 59.0 } else { 3.52 / 3.0 })
                * (0.55 + index * 0.23 % 0.9)
                * (1.0 + progress * 0.1);
            style(
                star,
                &format!(
                    "left:{}px;top:{}px;opacity:{alpha};transform:scale({scale})",
                    (11.0 + index * 37.0) % 98.0 / 100.0 * 1600.0,
                    (5.0 + index * 29.0) % 92.0 / 100.0 * 900.0 - 117.0 + progress * 396.0
                ),
            )?;
        }
        Ok(())
    }
}

fn action_button(
    document: &Document,
    label: &str,
    action: &str,
    class: &str,
) -> Result<HtmlElement, String> {
    let button = element(document, "button", class)?;
    button.set_attribute("type", "button").map_err(|_| "Create button type")?;
    button.set_attribute("aria-label", label).map_err(|_| "Create button label")?;
    button.set_attribute("data-menu-action", action).map_err(|_| "Create button action")?;
    Ok(button)
}

fn paint_hand(
    hand: &HtmlElement,
    pose: CreateHandPose,
    x: f64,
    y: f64,
    flipped: bool,
) -> Result<(), String> {
    let pose = match pose {
        CreateHandPose::Fist => "fist",
        CreateHandPose::Cupped => "cupped",
        CreateHandPose::Raised => "raised",
    };
    hand.set_attribute("src", &format!("assets/stock/menu/create-hand-{pose}.png"))
        .map_err(|_| "Create hand pose")?;
    style(
        hand,
        &format!(
            "left:{x}px;top:{y}px;transform:translate(-50%, -50%) scaleX({})",
            if flipped { -1 } else { 1 }
        ),
    )
}

pub(super) struct VfxView {
    pub(super) root: HtmlElement,
    sprites: Vec<(HtmlElement, HtmlElement)>,
}

impl VfxView {
    pub(super) fn new(document: &Document) -> Result<Self, String> {
        Ok(Self { root: element(document, "span", "create-element-vfx")?, sprites: Vec::new() })
    }

    pub(super) fn update(
        &mut self,
        document: &Document,
        atlas: &StockAtlas,
        plan: &[ElementVfxDraw],
    ) -> Result<(), String> {
        while self.sprites.len() < plan.len() {
            let sprite = element(document, "span", "element-vfx-sprite")?;
            let crop = element(document, "span", "element-vfx-crop")?;
            append(&sprite, &crop)?;
            append(&self.root, &sprite)?;
            self.sprites.push((sprite, crop));
        }
        for (index, (sprite, crop)) in self.sprites.iter().enumerate() {
            let Some(draw) = plan.get(index) else {
                sprite.set_hidden(true);
                continue;
            };
            sprite.set_hidden(false);
            let records = draw.sprite.records();
            let record = atlas.record(records[draw.frame % records.len()])?;
            let raw = [f64::from(record.width), f64::from(record.height)];
            let [width, height] = draw.sprite.registered_size().unwrap_or(raw);
            let [anchor_x, anchor_y] = draw.anchor.unwrap_or([0.5, 0.5]);
            let blend = if draw.blend == ElementVfxBlend::Add { "plus-lighter" } else { "normal" };
            style(
                sprite,
                &format!(
                    "left:{}px;top:{}px;width:{width}px;height:{height}px;opacity:{};mix-blend-mode:{blend};filter:url('#{}');transform:translate({}%, {}%) rotate({}deg) scale({})",
                    draw.x,
                    draw.y,
                    draw.alpha.clamp(0.0, 1.0),
                    tint_id(draw.color),
                    -anchor_x * 100.0,
                    -anchor_y * 100.0,
                    draw.rotation,
                    draw.scale
                ),
            )?;
            let (left, top) = if draw.sprite.registered_size().is_some() {
                (
                    (width - raw[0]) / 2.0 + f64::from(record.center_x),
                    (height - raw[1]) / 2.0 + f64::from(record.center_y),
                )
            } else {
                (0.0, 0.0)
            };
            style(
                crop,
                &format!(
                    "left:{left}px;top:{top}px;width:{}px;height:{}px;background-image:url('assets/stock/BadGuys.png');background-position:-{}px -{}px",
                    record.width, record.height, record.x, record.y
                ),
            )?;
        }
        Ok(())
    }
}

fn tint_id(color: [f64; 3]) -> String {
    let [r, g, b] = color.map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8);
    format!("element-tint-{r}-{g}-{b}")
}

fn install_tints(document: &Document) -> Result<(), String> {
    let namespace = Some("http://www.w3.org/2000/svg");
    let svg =
        document.create_element_ns(namespace, "svg").map_err(|_| "element tint definitions")?;
    svg.set_attribute("width", "0").map_err(|_| "element tint width")?;
    svg.set_attribute("height", "0").map_err(|_| "element tint height")?;
    svg.set_attribute("style", "position:absolute").map_err(|_| "element tint geometry")?;
    for color in [
        [1.0, 1.0, 1.0],
        [0.5, 0.75, 0.75],
        [0.5, 0.65, 0.5],
        [0.75, 0.95, 0.75],
        [1.0, 0.5, 1.0],
        [1.0, 0.5, 0.0],
        [1.0, 0.75, 1.0],
    ] {
        let filter =
            document.create_element_ns(namespace, "filter").map_err(|_| "element tint filter")?;
        filter.set_attribute("id", &tint_id(color)).map_err(|_| "element tint id")?;
        filter
            .set_attribute("color-interpolation-filters", "sRGB")
            .map_err(|_| "element tint colorspace")?;
        let matrix = document
            .create_element_ns(namespace, "feColorMatrix")
            .map_err(|_| "element tint matrix")?;
        let [r, g, b] = color.map(|channel| (channel * 255.0).round() / 255.0);
        matrix.set_attribute("type", "matrix").map_err(|_| "element tint type")?;
        matrix
            .set_attribute("values", &format!("{r} 0 0 0 0  0 {g} 0 0 0  0 0 {b} 0 0  0 0 0 1 0"))
            .map_err(|_| "element tint channels")?;
        filter.append_child(&matrix).map_err(|_| "attach element tint matrix")?;
        svg.append_child(&filter).map_err(|_| "attach element tint")?;
    }
    document
        .body()
        .ok_or("page has no body")?
        .append_child(&svg)
        .map_err(|_| "attach element tints")?;
    Ok(())
}

fn paint_wizard_name(
    document: &Document,
    input: &HtmlInputElement,
    node: &HtmlElement,
    value: &str,
) -> Result<(), String> {
    if finalize_create_wizard_name(value).is_err() {
        input.set_attribute("aria-invalid", "true").map_err(|_| "wizard name invalid state")?;
        node.set_text_content(None);
        return Ok(());
    }
    input.remove_attribute("aria-invalid").map_err(|_| "wizard name valid state")?;
    if value.is_empty() {
        node.set_text_content(None);
        return Ok(());
    }
    let mut name = layout_create_wizard_name(value);
    for glyph in &mut name.glyphs {
        glyph.tint = 0xd8ba70;
    }
    paint_text_layout(
        document,
        node,
        &NativeUiTextLayout {
            font: NativeUiFontName::Heading,
            glyphs: name.glyphs,
            lines: Vec::new(),
            width: name.width,
            height: name.height,
            unsupported_code_points: Vec::new(),
        },
    )
}
