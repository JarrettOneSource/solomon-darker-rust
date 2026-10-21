//! Shared hold presentation from Website GameplayPauseMenu/GameplayResumeProgress.
//! The native host owns pause, readiness and time; this view only explains its snapshot.

use solomon_sim::menu::native_pause_menu_reveal;
use solomon_sim::menu::shared::{
    GameplayPausePresentation, gameplay_pause_presentation, gameplay_resume_grace_progress,
};
use web_sys::{Document, HtmlElement};

use crate::ui::{append, element, style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SharedPauseView<'a> {
    pub owner_player_id: &'a str,
    pub owner_display_name: &'a str,
    /// Source labels: Inventory, the Skill Book, the skill selector; None for pause menu.
    pub activity: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResumeGraceView<'a> {
    /// Source hyphen IDs, including party-rejoin-wait and skill-picker-closed.
    pub reason: &'a str,
    pub remaining_ms: Option<u64>,
    pub sequence: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct SharedGameplaySnapshot<'a> {
    pub local_player_id: &'a str,
    pub has_game: bool,
    /// Title/Create/Hall visibility; ordinary pause and player panels do not cover the world.
    pub menu_covers_world: bool,
    pub settings_open: bool,
    pub pause: Option<SharedPauseView<'a>>,
    pub resume_grace: Option<ResumeGraceView<'a>>,
}

struct ResumeClock {
    reason: String,
    remaining_ms: Option<u64>,
    sequence: u64,
    received_at: f64,
}

pub struct SharedGameplayUi {
    pause: HtmlElement,
    dim: HtmlElement,
    pause_panel: HtmlElement,
    pause_label: HtmlElement,
    pause_detail: HtmlElement,
    resume: HtmlElement,
    resume_panel: HtmlElement,
    resume_label: HtmlElement,
    track: HtmlElement,
    fill: HtmlElement,
    pause_key: Option<(String, Option<String>)>,
    pause_received_at: f64,
    resume_clock: Option<ResumeClock>,
}

impl SharedGameplayUi {
    pub fn attach(document: &Document) -> Result<Self, String> {
        let pause = element(document, "div", "shared-gameplay-pause")?;
        pause.set_attribute("role", "status").map_err(|_| "shared pause role")?;
        pause.set_attribute("aria-live", "polite").map_err(|_| "shared pause status")?;
        let dim = element(document, "div", "shared-gameplay-dim")?;
        dim.set_attribute("aria-hidden", "true").map_err(|_| "shared pause dim")?;
        let pause_panel = element(document, "div", "shared-gameplay-waiting")?;
        let pause_label = element(document, "p", "shared-gameplay-pause-label")?;
        let pause_detail = element(document, "span", "shared-gameplay-pause-detail")?;
        append(&pause_panel, &pause_label)?;
        append(&pause_panel, &pause_detail)?;
        append(&pause, &dim)?;
        append(&pause, &pause_panel)?;
        let resume = element(document, "div", "shared-gameplay-resume")?;
        let resume_panel = element(document, "div", "shared-gameplay-resume-panel")?;
        let resume_label = element(document, "span", "shared-gameplay-resume-label")?;
        resume_label.set_attribute("role", "status").map_err(|_| "resume status role")?;
        resume_label.set_attribute("aria-live", "polite").map_err(|_| "resume status")?;
        resume_label.set_attribute("aria-atomic", "true").map_err(|_| "resume status atomic")?;
        let track = element(document, "div", "shared-gameplay-resume-track")?;
        for (name, value) in [
            ("role", "progressbar"),
            ("aria-label", "Resuming gameplay"),
            ("aria-valuemin", "0"),
            ("aria-valuemax", "100"),
        ] {
            track.set_attribute(name, value).map_err(|_| "resume progress semantics")?;
        }
        let fill = element(document, "div", "shared-gameplay-resume-fill")?;
        append(&track, &fill)?;
        append(&resume_panel, &resume_label)?;
        append(&resume_panel, &track)?;
        append(&resume, &resume_panel)?;
        let body = document.body().ok_or("shared gameplay body")?;
        append(&body, &pause)?;
        append(&body, &resume)?;
        pause.set_hidden(true);
        resume.set_hidden(true);
        Ok(Self {
            pause,
            dim,
            pause_panel,
            pause_label,
            pause_detail,
            resume,
            resume_panel,
            resume_label,
            track,
            fill,
            pause_key: None,
            pause_received_at: 0.0,
            resume_clock: None,
        })
    }

    pub fn update(
        &mut self,
        snapshot: SharedGameplaySnapshot<'_>,
        now_ms: f64,
        width: i32,
        height: i32,
    ) -> Result<(), String> {
        let visible = snapshot.has_game && !snapshot.menu_covers_world;
        let pause = snapshot.pause.filter(|pause| {
            visible && !snapshot.settings_open && pause.owner_player_id != snapshot.local_player_id
        });
        self.pause.set_hidden(pause.is_none());
        if let Some(pause) = pause {
            let key = (pause.owner_player_id.to_owned(), pause.activity.map(str::to_owned));
            if self.pause_key.as_ref() != Some(&key) {
                self.pause_key = Some(key);
                self.pause_received_at = now_ms;
            }
            match gameplay_pause_presentation(
                pause.owner_player_id,
                snapshot.local_player_id,
                pause.owner_display_name,
                pause.activity,
            ) {
                GameplayPausePresentation::Waiting { label, detail }
                    if self.pause_label.text_content().as_deref() != Some(&label) =>
                {
                    self.pause_label.set_text_content(Some(&label));
                    self.pause_detail.set_text_content(Some(&detail));
                    self.pause
                        .set_attribute("aria-label", &label)
                        .map_err(|_| "shared pause label")?;
                }
                _ => {}
            }
            let reveal = native_pause_menu_reveal(true, now_ms - self.pause_received_at);
            style(
                &self.dim,
                &format!("background-color:rgb(0 0 0 / {})", f64::from((reveal * 0.85) as f32)),
            )?;
            let scale = fit_panel(width, height, 620.0, 300.0);
            style(
                &self.pause_panel,
                &format!("opacity:{reveal};transform:translate(-50%,-50%) scale({scale})"),
            )?;
        } else {
            self.pause_key = None;
        }
        let grace =
            snapshot.resume_grace.filter(|grace| visible && grace.reason != "skill-picker-closed");
        self.resume.set_hidden(grace.is_none());
        let Some(grace) = grace else {
            self.resume_clock = None;
            return Ok(());
        };
        let same_grace = self.resume_clock.as_ref().is_some_and(|clock| {
            clock.reason == grace.reason
                && clock.sequence == grace.sequence
                && clock.remaining_ms == grace.remaining_ms
        });
        if !same_grace {
            self.resume_clock = Some(ResumeClock {
                reason: grace.reason.to_owned(),
                remaining_ms: grace.remaining_ms,
                sequence: grace.sequence,
                received_at: now_ms,
            });
        }
        let received_at = self.resume_clock.as_ref().expect("received grace").received_at;
        let progress = gameplay_resume_grace_progress(
            grace.remaining_ms.map(|value| value as f64),
            now_ms - received_at,
        );
        self.resume_label.set_text_content(Some(match progress {
            Some(_) => "RESUMING...",
            None if grace.reason == "party-rejoin-wait" => "Waiting for players to rejoin",
            None => "Waiting on players ...",
        }));
        self.track.set_hidden(progress.is_none());
        for (name, value) in [
            (
                "data-gameplay-resume-grace-phase",
                if progress.is_some() { "progress".to_owned() } else { "waiting".to_owned() },
            ),
            ("data-gameplay-resume-grace-reason", grace.reason.to_owned()),
            ("data-gameplay-resume-grace-sequence", grace.sequence.to_string()),
        ] {
            self.resume.set_attribute(name, &value).map_err(|_| "resume snapshot attributes")?;
        }
        if let Some(progress) = progress {
            self.track
                .set_attribute("aria-valuenow", &(progress * 100.0).round().to_string())
                .map_err(|_| "resume progress")?;
            style(&self.fill, &format!("transform:scaleX({progress})"))?;
        }
        let scale = fit_panel(width, height, 428.0, 240.0);
        style(&self.resume_panel, &format!("transform:translate(-50%,-50%) scale({scale})"))
    }
}

fn fit_panel(width: i32, height: i32, panel_width: f64, panel_height: f64) -> f64 {
    ((f64::from(width) - 24.0) / panel_width)
        .min((f64::from(height) - 24.0) / panel_height)
        .clamp(0.1, 1.0)
}

impl Drop for SharedGameplayUi {
    fn drop(&mut self) {
        self.pause.remove();
        self.resume.remove();
    }
}
