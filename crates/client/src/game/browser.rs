//! Browser canvas construction, saved-session admission and event ownership.

use std::cell::RefCell;
use std::rc::Rc;

use solomon_assets::save_document;
use solomon_graphics::Renderer;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;

use super::{Game, NativeGame, editor_test};
use crate::{
    set_status,
    storage::{self, SaveStore},
};

impl Game {
    /// Resource construction retires before saved-session parsing begins.
    pub fn load() -> impl std::future::Future<Output = Result<Rc<RefCell<Self>>, String>> {
        Box::pin(async {
            let window = web_sys::window().ok_or("no window")?;
            let document = window.document().ok_or("no document")?;
            let playtime = if editor_test::configured(&document)? {
                None
            } else {
                Some(storage::PlaytimeRecorder::start(&window)?)
            };
            let canvas: HtmlCanvasElement = document
                .get_element_by_id("game")
                .ok_or("the page has no #game canvas")?
                .dyn_into()
                .map_err(|_| "#game is not a canvas")?;

            let renderer = Renderer::for_canvas(&canvas)?;
            let surface = crate::platform::Surface::from_canvas(canvas);
            let mut game = Self::load_resources(surface, renderer, playtime).await?;
            game.initialize_session().await?;
            Ok(Rc::new(RefCell::new(game)))
        })
    }

    async fn initialize_session(&mut self) -> Result<(), String> {
        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;
        self.initialize_settings()?;
        if editor_test::configured(&document)? {
            self.load_editor_test(&window).await?;
            return Ok(());
        }
        match storage::read_hall() {
            Ok(entries) => self.hall_entries = entries,
            Err(error) => Self::show_storage_error(&error),
        }
        if let Some(mount) = crate::hosted::WebsiteMount::configured(&window)? {
            self.load_hosted(mount, None).await?;
            return Ok(());
        }
        if crate::network::native_host_requested()? {
            self.native = Some(NativeGame::inspect()?);
            self.menu.show_title(false)?;
            set_status("Checking saved game…");
            return Ok(());
        }
        let mut no_saved_game = false;
        match SaveStore::open().await {
            Ok(store) => {
                let stored = store.read().await;
                self.save_store = Some(store);
                match stored {
                    Ok(Some(record)) => {
                        self.save_revision = record.revision;
                        let restored = match save_document::decode(&record.document) {
                            Ok(checkpoint) => self.restore_checkpoint(checkpoint).await,
                            Err(error) => Err(error),
                        };
                        if let Err(error) = restored {
                            self.save_error = Some(error.clone());
                            Self::show_storage_error(&error);
                        }
                    }
                    Ok(None) => {
                        no_saved_game = true;
                    }
                    Err(error) => {
                        self.save_error = Some(error.clone());
                        Self::show_storage_error(&error);
                    }
                }
            }
            Err(error) => {
                self.save_error = Some(error.clone());
                Self::show_storage_error(&error);
            }
        }
        self.menu.show_title(self.has_game)?;
        if no_saved_game && !storage::tutorial_declined().unwrap_or(true) {
            self.menu.offer_tutorial()?;
        }
        Ok(())
    }

    pub(crate) fn attach_browser_input(
        game: &Rc<RefCell<Self>>,
        window: &web_sys::Window,
    ) -> Result<(), String> {
        let clock = window.document().ok_or("The game document is unavailable.")?.timeline();
        let owner = Rc::downgrade(game);
        game.borrow_mut().gpu_ui.set_native_dispatch_handler(move || {
            // Native events and rAF use one document animation clock. A performance
            // sample can be newer than the next rAF timestamp; inactive documents
            // have no animation time and leave input queued until they become active.
            let Some(time) = clock.current_time() else {
                return Ok(());
            };
            let game = owner.upgrade().ok_or("The game is no longer available for input.")?;
            game.borrow_mut().dispatch_ui_input(time)
        });
        Ok(())
    }

    pub fn retain_save_listener(
        &mut self,
        listener: wasm_bindgen::closure::Closure<dyn FnMut(web_sys::Event)>,
    ) {
        self.save_listeners.push(listener);
    }
}
