//! Source HUD z10 / Social z60 stays fixed across book and HUD lifetimes.

use egui::{Context, Event, Id, LayerId, Modifiers, Order, Pos2, Rect, Sense, pos2, vec2};

use super::{SocialUi, layers_above_hud};

#[derive(Default)]
struct Journey {
    book: bool,
    party_clicks: usize,
    inventory_clicks: usize,
}

fn pass(ctx: &Context, journey: &mut Journey, ordered: bool, events: Vec<Event>) {
    let viewport = Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0));
    let hud = LayerId::new(Order::Middle, Id::unique("game-hud"));
    let book = journey.book;
    ctx.run_pass(
        egui::RawInput { screen_rect: Some(viewport), events, ..Default::default() },
        |ctx| {
            if !book {
                crate::player_panels::chrome::native_area(
                    ctx,
                    hud,
                    egui::emath::TSTransform::IDENTITY,
                )
                .show(ctx, |ui| {
                    ui.set_min_size(viewport.size());
                    let inventory = ui.interact(
                        Rect::from_min_size(pos2(350.0, 10.0), vec2(40.0, 35.0)),
                        Id::unique("hud:inventory"),
                        Sense::click(),
                    );
                    if inventory.clicked() {
                        journey.inventory_clicks += 1;
                        journey.book = true;
                    }
                });
            } else {
                egui::Area::new(Id::unique("player-panels"))
                    .order(Order::Foreground)
                    .fixed_pos(Pos2::ZERO)
                    .show(ctx, |ui| {
                        ui.set_min_size(viewport.size());
                        if ui.input_mut(|input| {
                            input.consume_key(Modifiers::NONE, egui::Key::Escape)
                        }) {
                            journey.book = false;
                        }
                    });
            }
            if ordered {
                layers_above_hud(ctx, hud);
            }
            let layer = SocialUi::party_layer();
            egui::Area::new(layer.id)
                .order(layer.order)
                .fixed_pos(pos2(11.0, 174.0))
                .constrain(false)
                .show(ctx, |ui| {
                    let origin = ui.min_rect().min;
                    ui.set_min_size(vec2(236.0, 92.0));
                    let header = ui.interact(
                        Rect::from_min_size(origin, vec2(236.0, 50.0)),
                        Id::unique("social/party"),
                        Sense::click(),
                    );
                    if header.clicked() {
                        journey.party_clicks += 1;
                    }
                });
        },
    )
    .drop_without_applying_deltas();
}

fn click(ctx: &Context, journey: &mut Journey, ordered: bool, pos: Pos2) {
    for pressed in [true, false] {
        pass(
            ctx,
            journey,
            ordered,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        // The canonical dispatcher materializes committed layout between native events.
        pass(ctx, journey, ordered, Vec::new());
    }
}

#[test]
fn reappearing_hud_keeps_party_clickable_after_inventory_closes() {
    for ordered in [false, true] {
        let ctx = Context::default();
        let mut journey = Journey::default();
        // First Area sizing pass followed by the ordinary committed layout.
        pass(&ctx, &mut journey, ordered, Vec::new());
        pass(&ctx, &mut journey, ordered, Vec::new());
        click(&ctx, &mut journey, ordered, pos2(129.0, 199.0));
        assert_eq!(journey.party_clicks, 1, "fresh Party receives actual pointer events");
        click(&ctx, &mut journey, ordered, pos2(370.0, 25.0));
        assert_eq!(journey.inventory_clicks, 1);
        assert!(journey.book);
        pass(
            &ctx,
            &mut journey,
            ordered,
            vec![Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert!(!journey.book);
        // Empty commit and final layout retire the book and make HUD visible again.
        pass(&ctx, &mut journey, ordered, Vec::new());
        pass(&ctx, &mut journey, ordered, Vec::new());
        click(&ctx, &mut journey, ordered, pos2(129.0, 199.0));
        assert_eq!(
            journey.party_clicks,
            if ordered { 2 } else { 1 },
            "Source layer relation preserves Party; unlinked full-screen HUD blocks it"
        );
    }
}
