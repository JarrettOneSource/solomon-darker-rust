use super::*;
use egui::accesskit::{Action, ActionData, ActionRequest, Role, TreeId};

fn range_frame(
    context: &egui::Context,
    setting: NumericSetting,
    value: &mut u16,
    enabled: bool,
    requests: Vec<ActionRequest>,
) -> egui::accesskit::TreeUpdate {
    let (name, label, _, _) = setting.definition();
    let id = egui::Id::unique(format!("settings-range:{name}"));
    let output = context.run_pass(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 100.0))),
            events: requests.into_iter().map(egui::Event::AccessKitActionRequest).collect(),
            ..Default::default()
        },
        |context| {
            context.root_ui(|ui| {
                ui.add_enabled_ui(enabled, |ui| {
                    let response = ui.interact(
                        Rect::from_min_size(Pos2::ZERO, Vec2::new(250.0, 40.0)),
                        id,
                        egui::Sense::click_and_drag(),
                    );
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(Role::Slider, response.enabled(), label)
                    });
                    setting.accessible_value(ui, &response, value);
                });
            });
        },
    );
    let update = output.platform_output.accesskit_update.clone().unwrap();
    output.drop_without_applying_deltas();
    update
}

fn request(action: Action, data: Option<ActionData>) -> ActionRequest {
    ActionRequest {
        action,
        target_tree: TreeId::ROOT,
        target_node: egui::Id::unique("settings-range:music").accesskit_id(),
        data,
    }
}

#[test]
fn settings_ranges_expose_their_numeric_value_and_adjustment_bounds() {
    for setting in [
        NumericSetting::Sound,
        NumericSetting::Music,
        NumericSetting::Fov,
        NumericSetting::UiScale,
        NumericSetting::LightQuality,
    ] {
        let context = egui::Context::default();
        context.enable_accesskit();
        let (name, _, min, max) = setting.definition();
        let mut value = min + 1;
        let update = range_frame(&context, setting, &mut value, true, Vec::new());
        let tree = accesskit_consumer::Tree::new(update, true);
        let node = tree
            .state()
            .node_by_tree_local_id(
                egui::Id::unique(format!("settings-range:{name}")).accesskit_id(),
                TreeId::ROOT,
            )
            .unwrap();
        assert_eq!(node.role(), Role::Slider);
        assert_eq!(node.numeric_value(), Some(f64::from(min + 1)));
        assert_eq!(node.min_numeric_value(), Some(f64::from(min)));
        assert_eq!(node.max_numeric_value(), Some(f64::from(max)));
        assert_eq!(node.numeric_value_step(), Some(1.0));
        for action in [Action::SetValue, Action::Increment, Action::Decrement] {
            assert!(node.data().supports_action(action), "{name} must support {action:?}");
        }
    }
}

#[test]
fn native_range_actions_adjust_the_same_clamped_setting_and_respect_disabled_controls() {
    let context = egui::Context::default();
    context.enable_accesskit();
    let mut value = 50;
    for (actions, expected) in [
        (vec![request(Action::SetValue, Some(ActionData::NumericValue(24.6)))], 25),
        (vec![request(Action::Increment, None)], 26),
        (vec![request(Action::Decrement, None)], 25),
        (vec![request(Action::SetValue, Some(ActionData::NumericValue(-20.0)))], 0),
        (vec![request(Action::Decrement, None)], 0),
        (vec![request(Action::SetValue, Some(ActionData::NumericValue(150.0)))], 100),
        (vec![request(Action::Increment, None)], 100),
        (vec![request(Action::SetValue, Some(ActionData::NumericValue(f64::NAN)))], 100),
        (vec![request(Action::SetValue, Some(ActionData::Value("bad".into())))], 100),
    ] {
        range_frame(&context, NumericSetting::Music, &mut value, true, actions);
        assert_eq!(value, expected);
    }
    range_frame(
        &context,
        NumericSetting::Music,
        &mut value,
        false,
        vec![request(Action::SetValue, Some(ActionData::NumericValue(20.0)))],
    );
    assert_eq!(value, 100);
}
