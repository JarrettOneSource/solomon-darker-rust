//! Stock terminal-run overlay from `GameOverOverlay.tsx` and `boneyard.css`.
//! The session owns every clock and admission check; this view only emits Continue.

use std::cell::RefCell;
use std::rc::Rc;

use solomon_sim::Vec2;
use solomon_sim::game_run::presentation::{game_over_presentation, solomon_riff_presentation};
use solomon_sim::game_run::{GameRunLifecycle, GameRunPhase};
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextSpec};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{Document, Event, HtmlElement};

use crate::menu::navigation::MenuNavigation;
use crate::ui::{append, element, style};

#[derive(Clone, Debug)]
pub struct ContinueGameOver {
    pub run_id: String,
    pub event_id: u64,
}

/// Screen coordinates and camera scale of the local wizard's final position.
#[derive(Clone, Copy)]
pub struct GameOverAnchor {
    pub position: Vec2,
    pub zoom: f64,
}

pub struct GameOverOverlay {
    document: Document,
    root: HtmlElement,
    button: HtmlElement,
    riff: HtmlElement,
    entry: HtmlElement,
    exit: HtmlElement,
    game: HtmlElement,
    over: HtmlElement,
    prompt: HtmlElement,
    prompt_scale: Option<f64>,
    accepted: Rc<RefCell<Option<ContinueGameOver>>>,
    actions: Rc<RefCell<Vec<ContinueGameOver>>>,
    navigation: MenuNavigation,
    _click: Closure<dyn FnMut(Event)>,
}

impl GameOverOverlay {
    pub fn attach(document: &Document) -> Result<Self, String> {
        let root = element(document, "div", "game-over-overlay")?;
        root.set_hidden(true);
        let button = element(document, "button", "boneyard-game-over")?;
        button.set_attribute("type", "button").map_err(|_| "Game Over button type")?;
        button
            .set_attribute("data-game-default-focus", "true")
            .map_err(|_| "Game Over default focus")?;
        let riff = element(document, "i", "game-over-solomon-riff")?;
        let entry = element(document, "i", "game-over-black game-over-entry-black")?;
        let exit = element(document, "i", "game-over-black game-over-exit-black")?;
        let game = element(document, "img", "game-over-word game-over-word-game")?;
        let over = element(document, "img", "game-over-word game-over-word-over")?;
        for (image, file) in [(&game, "game-over-game"), (&over, "game-over-over")] {
            image
                .set_attribute("src", &format!("assets/stock/{file}.png"))
                .map_err(|_| "Game Over title art")?;
            image.set_attribute("alt", "").map_err(|_| "Game Over image description")?;
        }
        let prompt = element(document, "span", "game-over-prompt")?;
        for child in [&riff, &entry, &game, &over, &prompt, &exit] {
            child.set_attribute("aria-hidden", "true").map_err(|_| "Game Over decoration")?;
            append(&button, child)?;
        }
        append(&root, &button)?;
        append(&document.body().ok_or("Game Over document body")?, &root)?;
        let accepted = Rc::new(RefCell::new(None::<ContinueGameOver>));
        let actions = Rc::new(RefCell::new(Vec::new()));
        let current = accepted.clone();
        let pending = actions.clone();
        let click = Closure::new(move |event: Event| {
            event.stop_propagation();
            if let Some(action) = current.borrow().as_ref() {
                pending.borrow_mut().push(action.clone());
            }
        });
        button
            .add_event_listener_with_callback("click", click.as_ref().unchecked_ref())
            .map_err(|_| "Game Over click listener")?;
        Ok(Self {
            document: document.clone(),
            root,
            button,
            riff,
            entry,
            exit,
            game,
            over,
            prompt,
            prompt_scale: None,
            accepted,
            actions,
            navigation: MenuNavigation::new(),
            _click: click,
        })
    }

    pub fn take_actions(&mut self) -> Vec<ContinueGameOver> {
        std::mem::take(&mut *self.actions.borrow_mut())
    }

    pub fn update(
        &mut self,
        run: Option<&GameRunLifecycle>,
        anchor: GameOverAnchor,
        now: f64,
    ) -> Result<(), String> {
        let Some(run) = run.filter(|run| run.phase == GameRunPhase::GameOver) else {
            if !self.root.hidden() {
                self.root.set_hidden(true);
                self.accepted.borrow_mut().take();
                self.actions.borrow_mut().clear();
                self.navigation.release();
            }
            return Ok(());
        };
        self.root.set_hidden(false);
        let scale = solomon_sim::viewport::game_viewport(Vec2::new(
            f64::from(self.root.client_width()),
            f64::from(self.root.client_height()),
        ))
        .display_scale;
        if self.prompt_scale != Some(scale) {
            self.prompt.set_text_content(None);
            let spec = NativeUiTextSpec {
                scale,
                tint: 0xd9ba70,
                ..NativeUiTextSpec::new(NativeUiFontName::Menu, "CLICK TO CONTINUE...", 0.0, 0.0)
            };
            append(
                &self.prompt,
                &crate::ui::text::native_text(&self.document, &spec, "game-over-prompt-text")?,
            )?;
            self.prompt
                .style()
                .set_property("top", &format!("calc(100% - {}px)", 50.0 * scale))
                .map_err(|_| "Game Over prompt baseline")?;
            // Source places this overlay inside the scaled native frame. The riff
            // already receives physical anchor/zoom; its title and pen need that scale too.
            for (node, width, height, offset) in
                [(&self.game, 307.0, 119.0, -175.0), (&self.over, 306.0, 120.0, 125.0)]
            {
                style(
                    node,
                    &format!(
                        "width:{}px;height:{}px;top:calc(50% + {}px)",
                        width * scale,
                        height * scale,
                        offset * scale
                    ),
                )?;
            }
            self.prompt_scale = Some(scale);
        }
        let run_id = run.run_id.as_ref().expect("Game Over belongs to an active run");
        let view = game_over_presentation(
            run.game_over_ticks,
            run.game_over_exit_ticks,
            run.game_over_exit_kind,
        );
        let was_ready = self.accepted.borrow().is_some();
        *self.accepted.borrow_mut() = view
            .accepts_input
            .then(|| ContinueGameOver { run_id: run_id.clone(), event_id: run.game_over_event_id });
        if view.accepts_input {
            self.button.remove_attribute("disabled").map_err(|_| "Game Over input enable")?;
            if !was_ready {
                self.button.focus().map_err(|_| "Game Over focus")?;
            }
        } else {
            self.button.set_attribute("disabled", "").map_err(|_| "Game Over input gate")?;
        }
        for (name, value) in [
            ("data-run-id", run_id.clone()),
            ("data-event-id", run.game_over_event_id.to_string()),
            ("data-game-over-ticks", run.game_over_ticks.to_string()),
            ("data-input-ready", view.accepts_input.to_string()),
            (
                "aria-label",
                if view.accepts_input { "Game over. Continue to loadout." } else { "Game over." }
                    .into(),
            ),
        ] {
            self.button.set_attribute(name, &value).map_err(|_| "Game Over state attribute")?;
        }
        for (node, opacity) in [
            (&self.entry, view.entry_fade_alpha),
            (&self.exit, view.exit_fade_alpha),
            (&self.game, view.title_alpha),
            (&self.over, view.title_alpha),
            (&self.prompt, view.prompt_alpha),
        ] {
            node.style()
                .set_property("opacity", &opacity.to_string())
                .map_err(|_| "Game Over presentation alpha")?;
        }
        let riff = solomon_riff_presentation(run.game_over_ticks);
        self.riff.set_hidden(riff.frame_record.is_none());
        if let Some(frame) = riff.frame_record {
            style(
                &self.riff,
                &format!(
                    "left:{}px;top:{}px;background-position:-{}px 0;transform:translate(-50%,-50%) scale({})",
                    anchor.position.x + riff.offset.x * anchor.zoom,
                    anchor.position.y + riff.offset.y * anchor.zoom,
                    (frame - 1) * 200,
                    anchor.zoom,
                ),
            )?;
        }
        self.navigation.update(&self.root, &self.document, run_id, now)?;
        Ok(())
    }
}
