//! Protected mod media has one retained URL per displayed source. The host owns
//! fetch policy; the GPU owns decoding and textures. Stale completions are dropped.
use super::super::widgets;
use crate::{gpu_ui::UiFrame, hosted::OwnedImageUrl, menu::HostedMenuAction};
use egui::{Align2, Rect, Ui};
use std::collections::{BTreeMap, BTreeSet};
enum Image {
    Pending(u64),
    Ready(OwnedImageUrl),
    Failed,
}
#[derive(Default)]
pub(super) struct CloudImages {
    entries: BTreeMap<String, Image>,
    used: BTreeSet<String>,
    next_request: u64,
    retired: Vec<String>,
    actions: Vec<HostedMenuAction>,
}
impl CloudImages {
    pub fn begin_paint(&mut self) {
        self.used.clear();
    }
    pub fn finish_paint(&mut self, frame: &UiFrame<'_>) {
        for (source, image) in &self.entries {
            if !self.used.contains(source)
                && let Image::Ready(url) = image
            {
                self.retired.push(url.url().to_owned());
            }
        }
        self.entries.retain(|url, _| self.used.contains(url));
        self.actions.retain(|action| match action {
            HostedMenuAction::LoadModImage { url, .. } => self.used.contains(url),
            _ => true,
        });
        self.drain_retired(frame);
    }
    pub fn drain_retired(&mut self, frame: &UiFrame<'_>) {
        for path in self.retired.drain(..) {
            frame.forget_image(&path);
        }
    }
    pub fn clear(&mut self) {
        for image in self.entries.values() {
            if let Image::Ready(url) = image {
                self.retired.push(url.url().to_owned());
            }
        }
        self.entries.clear();
        self.used.clear();
        self.actions.clear();
    }
    pub fn complete(
        &mut self,
        id: u64,
        result: Result<OwnedImageUrl, String>,
    ) -> Result<bool, String> {
        let source = self
            .entries
            .iter()
            .find_map(|(s, state)| matches!(state,Image::Pending(p)if *p==id).then(|| s.clone()));
        let Some(source) = source else { return Ok(false) };
        self.entries.insert(source, result.map_or(Image::Failed, Image::Ready));
        Ok(true)
    }
    pub fn take_actions(&mut self) -> Vec<HostedMenuAction> {
        std::mem::take(&mut self.actions)
    }
    pub fn paint(
        &mut self,
        frame: &UiFrame<'_>,
        ui: &Ui,
        source: Option<&str>,
        alt: &str,
        rect: Rect,
        contain: bool,
    ) {
        ui.painter().rect_filled(rect, 0.0, widgets::color(0x090806));
        let Some(source) = source else {
            missing(ui, rect, alt);
            return;
        };
        self.used.insert(source.to_owned());
        if !self.entries.contains_key(source) {
            self.next_request += 1;
            self.entries.insert(source.to_owned(), Image::Pending(self.next_request));
            self.actions.push(HostedMenuAction::LoadModImage {
                request_id: self.next_request,
                url: source.to_owned(),
            });
        }
        let Some(Image::Ready(url)) = self.entries.get(source) else {
            if matches!(self.entries.get(source), Some(Image::Failed)) {
                missing(ui, rect, alt);
            }
            return;
        };
        if frame.image_error(url.url()).is_some() {
            self.retired.push(url.url().to_owned());
            self.entries.insert(source.to_owned(), Image::Failed);
            missing(ui, rect, alt);
            return;
        }
        let size = frame.image_size(url.url());
        let dest = if contain {
            size.map_or(rect, |s| {
                Rect::from_center_size(
                    rect.center(),
                    s * (rect.width() / s.x).min(rect.height() / s.y),
                )
            })
        } else {
            size.map_or(rect, |s| {
                Rect::from_center_size(
                    rect.center(),
                    s * (rect.width() / s.x).max(rect.height() / s.y),
                )
            })
        };
        frame.image(
            &ui.painter().with_clip_rect(rect.intersect(ui.clip_rect())),
            url.url(),
            dest,
            None,
            egui::Color32::WHITE,
        );
    }
}
fn missing(ui: &Ui, rect: Rect, _alt: &str) {
    widgets::label(
        ui,
        "NO\nIMAGE",
        rect.center(),
        Align2::CENTER_CENTER,
        "Palatino",
        14.0,
        widgets::color(0xa99a70),
    );
}
