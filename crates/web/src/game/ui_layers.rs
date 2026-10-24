//! Current application stacking contexts from c140's body append sites and CSS.
//! Native inner painters retain fa08 geometry. Every entry is supplied by its real
//! producer; registration never creates a hidden Area or changes its input admission.

use super::Game;
use crate::fullscreen::FullscreenControl;
use crate::mobile_editor::MobileUiEditor;
use crate::mod_skills::ModSkillsUi;
use crate::mod_world_overlays::ModWorldOverlays;
use crate::shared_gameplay::SharedGameplayUi;
use crate::social::SocialUi;

impl Game {
    pub(super) fn configure_ui_layers(&self) -> Result<(), String> {
        let [world_notices, hub_notices] = self.notifications.layer_ids();
        let mut layers = vec![
            world_notices,
            crate::runtime_ui::layer_id(),
            self.hud.layer_id(),               // 10
            ModWorldOverlays::powerup_layer(), // 18, in actual append order
            ModWorldOverlays::shop_layer(),
            ModWorldOverlays::enemy_layer(), // 19
            self.input.layer_id(),           // 20, before College and authored effects
            self.college_overlay.layer_id(),
            ModWorldOverlays::effect_layer(),
        ];
        layers.extend(self.panels.layer_ids()); // Complete book subtree at 30.
        layers.extend([
            self.mod_minimap.layer_id(), // 36
            self.mod_panels.layer_id(),  // 37
            self.mod_skill_quickbar.layer_id(),
            self.tutorial_ui.layer_id(), // 40
            hub_notices,                 // 45
        ]);

        // Both menu subtrees stay inside body50 even when their local levels are large.
        let scene = [
            crate::menu::scene_layer(),
            MobileUiEditor::layer_id(crate::menu::scene_layer()),
            crate::menu::library_layer(),
        ];
        let overlay =
            [crate::menu::overlay_layer(), MobileUiEditor::layer_id(crate::menu::overlay_layer())];
        if self
            .menu
            .overlay_stack_level()
            .is_some_and(|level| level < self.menu.scene_stack_level())
        {
            layers.extend(overlay);
            layers.extend(scene);
        } else {
            layers.extend(scene);
            layers.extend(overlay);
        }
        layers.push(SharedGameplayUi::pause_layer()); // 55
        layers.extend(SocialUi::fixed_layers()); // Complete Social subtree at 60.
        let fullscreen_above_guidance = self.fullscreen.above_orientation_guidance()?;
        if !fullscreen_above_guidance {
            layers.extend(FullscreenControl::fixed_layers()); // 70
        }
        layers.extend(self.skill_picker.layer_ids()); // 80, before authored scene.
        layers.extend([
            self.mod_overlays.scene_layer(),
            SharedGameplayUi::resume_layer(), // 90
            self.observer_ui.layer_id(),      // 120
        ]);
        if let Some(desktop) = &self.desktop_bridge {
            layers.push(desktop.layer_id()); // 10000, desktop-only producer.
        }
        if let Some(hosted) = &self.hosted {
            layers.push(hosted.deployment_update.layer_id()); // 10000, hosted producer.
        }
        layers.extend([
            self.spectator_ui.layer_id(),     // 10001
            ModSkillsUi::picker_layer(),      // 10002
            ModSkillsUi::book_layer(),        // 10004
            self.game_over.layer_id(),        // 20000
            self.panels.notebox_layer_id(),   // 31000, independently body-mounted.
            crate::menu::orientation_layer(), // 100000, appended before Cheats.
            self.cheats.layer_id(),
        ]);
        if fullscreen_above_guidance {
            layers.extend(FullscreenControl::fixed_layers()); // Coarse portrait100001.
        }
        layers.push(self.match_loading_view.layer_id()); // 200000

        // Cloud owns the application's one actual NativeUiDialog/showModal instance.
        // Its mounted top layer is above every numeric document context.
        layers.push(crate::menu::cloud_dialog_layer());
        self.gpu_ui.context().set_fixed_layer_order(&layers);
        Ok(())
    }
}
