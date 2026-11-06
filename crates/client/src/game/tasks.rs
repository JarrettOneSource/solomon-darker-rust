//! Async work outlives one frame, and is cancelled by the native client lifetime owner.
use super::Game;
use crate::{boneyard::PreparedBoneyardView, navigation_data, set_status};
use std::{cell::RefCell, rc::Rc};
pub(crate) fn load_pending_world(game: &Rc<RefCell<Game>>) {
    let Some((generation, loaded)) = game.borrow_mut().take_world_load() else {
        return;
    };
    let game = game.clone();
    crate::platform::spawn_local(async move {
        let result = async {
            let prepared = PreparedBoneyardView::load(&loaded).await?;
            let navigation = navigation_data::load_navigation(&loaded).await?;
            Ok::<_, String>((prepared, navigation))
        }
        .await;
        let mut game = game.borrow_mut();
        if !game.world_load_current(generation) {
            return;
        }
        let installed = result
            .and_then(|(prepared, navigation)| game.install_boneyard(loaded, prepared, navigation));
        match installed {
            Ok(()) => set_status(""),
            Err(error) => {
                game.world_load_failed(&error);
                #[cfg(target_arch = "wasm32")]
                if let Err(error) = game.editor_test_failed(&error) {
                    crate::platform::error(&error);
                }
            }
        }
    });
}

pub(crate) fn write_pending_save(game: &Rc<RefCell<Game>>) {
    let pending = game.borrow_mut().take_save();
    if let Some(pending) = pending {
        let game = game.clone();
        crate::platform::spawn_local(async move {
            let result = pending.write().await;
            game.borrow_mut().save_completed(result);
        });
    }
}

pub(crate) fn run_hosted_work(game: &Rc<RefCell<Game>>) {
    for pending in game.borrow_mut().take_hosted_work() {
        let game = game.clone();
        crate::platform::spawn_local(async move {
            let completed = pending.execute().await;
            if let Err(error) = game.borrow_mut().complete_hosted_work(completed) {
                crate::platform::error(&error);
                set_status(&error);
            }
        });
    }
}
