//! Skills-book view of the accepted Boast selection and progress.

use solomon_sim::economy::{BoastSelection, BoastState};
use solomon_sim::hub::interaction::NATIVE_BOASTS;
use solomon_sim::hud::HudRect;
use solomon_sim::modding::projection::ModContentProjection;
use solomon_sim::native_ui_text::{NativeUiFontName, NativeUiTextSpec, layout_native_ui_text};

use super::chrome::PanelCanvas;
use super::{PlayerPanelSnapshot, PlayerPanels};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BoastStatus {
    Active,
    Achieved,
    Failed,
}

impl BoastStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Achieved => "Achieved",
            Self::Failed => "Failed",
        }
    }

    fn tint(self) -> u32 {
        match self {
            Self::Active => 0xd9ba70,
            Self::Achieved => 0x97c797,
            Self::Failed => 0xe08080,
        }
    }
}

struct BoastProgress<'a> {
    name: &'a str,
    statement: &'a str,
    status: BoastStatus,
}

fn boast_progress<'a>(
    state: &BoastState,
    mods: Option<&'a ModContentProjection>,
) -> Option<BoastProgress<'a>> {
    let (name, statement) = match state.selected.as_ref()? {
        BoastSelection::Native(id) => {
            let boast = NATIVE_BOASTS.iter().find(|boast| boast.id == *id)?;
            (boast.label, boast.statement)
        }
        BoastSelection::Mod(selection) => {
            let boast = mods?.boast(selection)?;
            (boast.name.as_str(), boast.statement.as_str())
        }
    };
    Some(BoastProgress {
        name,
        statement,
        status: if state.failed {
            BoastStatus::Failed
        } else if state.succeeded {
            BoastStatus::Achieved
        } else {
            BoastStatus::Active
        },
    })
}

impl PlayerPanels {
    pub(super) fn draw_boasts(
        &self,
        c: &PanelCanvas<'_, '_>,
        s: &PlayerPanelSnapshot<'_>,
    ) -> Result<(), String> {
        let progress = boast_progress(&s.economy.npc.boast, s.mod_content);
        let (name, statement, status, tint) = if let Some(progress) = progress {
            (progress.name, progress.statement, progress.status.label(), progress.status.tint())
        } else if s.economy.npc.boast.selected.is_some() {
            ("Boast unavailable", "This Boast's content is not installed.", "", 0x808080)
        } else {
            ("No Boast selected", "Choose a Boast from Provokatus in the College.", "", 0x808080)
        };
        let name_layout = layout_native_ui_text(&NativeUiTextSpec {
            tint,
            max_width: Some(560.0),
            line_height: Some(32.0),
            ..NativeUiTextSpec::new(NativeUiFontName::Menu, name, 800.0, 300.0)
        });
        let status_y = 300.0 + name_layout.height + 35.0;
        let statement_y = status_y + if status.is_empty() { 0.0 } else { 45.0 };
        let statement_layout = layout_native_ui_text(&NativeUiTextSpec {
            tint: 0xbfbfbf,
            max_width: Some(560.0),
            line_height: Some(24.0),
            ..NativeUiTextSpec::new(NativeUiFontName::Body, statement, 800.0, statement_y)
        });
        let card =
            HudRect::new(490.0, 240.0, 620.0, statement_y - 240.0 + statement_layout.height + 35.0);
        c.fill(card, 0x100d0a, 0.95);
        c.stroke(card, tint, 2.0);
        c.layout(&name_layout);
        if !status.is_empty() {
            c.shadow_text(&NativeUiTextSpec {
                tint,
                ..NativeUiTextSpec::new(NativeUiFontName::Menu, status, 800.0, status_y)
            });
        }
        c.layout(&statement_layout);
        c.semantic(
            "panel:boast-progress",
            &format!("{name}\n{status}\n{statement}"),
            "status",
            card,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solomon_sim::economy::BoastState;

    #[test]
    fn the_selected_boast_reports_active_achieved_and_failed_progress() {
        let mut boast = BoastState { selected: Some(2.into()), ..Default::default() };
        let progress = boast_progress(&boast, None).expect("selected stock Boast");
        assert_eq!(progress.name, "SECONDARIES ARE SISSY!");
        assert_eq!(progress.status, BoastStatus::Active);
        assert_eq!(progress.status.tint(), 0xd9ba70);
        boast.succeeded = true;
        let progress = boast_progress(&boast, None).unwrap();
        assert_eq!(progress.status, BoastStatus::Achieved);
        assert_eq!(progress.status.tint(), 0x97c797);
        boast.failed = true;
        let progress = boast_progress(&boast, None).unwrap();
        assert_eq!(progress.status, BoastStatus::Failed);
        assert_eq!(progress.status.tint(), 0xe08080);
    }

    #[test]
    fn no_selection_does_not_invent_an_active_boast() {
        assert!(boast_progress(&BoastState::default(), None).is_none());
    }

    #[test]
    fn every_stock_boast_uses_its_own_name_and_statement() {
        for id in 0..=4 {
            let state = BoastState { selected: Some(id.into()), ..Default::default() };
            let progress = boast_progress(&state, None).unwrap();
            assert_eq!(progress.name, NATIVE_BOASTS[id as usize].label);
            assert_eq!(progress.statement, NATIVE_BOASTS[id as usize].statement);
        }
    }

    #[test]
    fn authored_boasts_resolve_the_complete_namespaced_selection() {
        use solomon_sim::modding::content::{ModBoastSelection, PreparedModBoastIcon};
        use solomon_sim::modding::projection::ModBoastProjectionEntry;

        let definition = |mod_id: &str, name: &str| ModBoastProjectionEntry {
            content_id: "1".into(),
            failure_producers: Vec::new(),
            icon: PreparedModBoastIcon::Stock { record: 90, style: 0 },
            instruction: String::new(),
            mod_id: mod_id.into(),
            name: name.into(),
            random_skill_choices: false,
            response: String::new(),
            score_multiplier: 2.0,
            statement: format!("{name} statement"),
            success_wave: 10,
        };
        let mods = ModContentProjection {
            boasts: vec![definition("first", "First Boast"), definition("second", "Second Boast")],
            content: Vec::new(),
            manifest_sha256: String::new(),
            powerups: Vec::new(),
            revision: 1.0,
            statuses: Vec::new(),
        };
        let state = BoastState {
            selected: Some(BoastSelection::Mod(ModBoastSelection {
                content_id: "1".into(),
                mod_id: "second".into(),
            })),
            succeeded: true,
            ..Default::default()
        };
        let progress = boast_progress(&state, Some(&mods)).unwrap();
        assert_eq!(progress.name, "Second Boast");
        assert_eq!(progress.statement, "Second Boast statement");
        assert_eq!(progress.status, BoastStatus::Achieved);
        assert!(boast_progress(&state, None).is_none());
    }
}
