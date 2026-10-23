//! Pinned Website `game-asset-readiness.ts`: insertion-ordered unfinished work.

use std::collections::BTreeSet;

pub const RUNTIME: &str = "Game runtime";
pub const ACCOUNT: &str = "Account and saved game";
pub const FIRST_FRAME: &str = "First game frame";

#[derive(Clone)]
pub struct StartupResource {
    stage: &'static str,
    source: String,
}

impl StartupResource {
    pub fn new(stage: &'static str, source: impl Into<String>) -> Self {
        Self { stage, source: source.into() }
    }
}

pub(super) struct StartupProgress {
    pending: Vec<StartupResource>,
    total: u32,
    stage: &'static str,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Snapshot {
    pub stage: &'static str,
    pub source: String,
    pub completed: u32,
    pub total: u32,
}

impl StartupProgress {
    pub fn new(resources: impl IntoIterator<Item = StartupResource>) -> Result<Self, String> {
        let mut seen = BTreeSet::new();
        let pending: Vec<_> = std::iter::once(StartupResource::new("runtime", RUNTIME))
            .chain(resources)
            .chain([
                StartupResource::new("account", ACCOUNT),
                StartupResource::new("ready", FIRST_FRAME),
            ])
            .filter(|resource| seen.insert(resource.source.clone()))
            .collect();
        let total = u32::try_from(pending.len()).map_err(|_| "Too many startup resources")?;
        Ok(Self { pending, total, stage: "runtime" })
    }

    pub fn enter_phase(&mut self, stage: &'static str) -> Result<(), String> {
        if !self.pending.iter().any(|resource| resource.stage == stage) {
            return Err(format!("Startup phase {stage} has no unfinished resources"));
        }
        self.stage = stage;
        Ok(())
    }

    pub fn complete(&mut self, source: &str) -> Result<(), String> {
        let index = self
            .pending
            .iter()
            .position(|resource| resource.source == source)
            .ok_or_else(|| format!("Startup resource was not pending: {source}"))?;
        self.pending.remove(index);
        Ok(())
    }

    pub fn snapshot(&self) -> Snapshot {
        let resource = self
            .pending
            .iter()
            .find(|resource| resource.stage == self.stage)
            .or_else(|| self.pending.first());
        Snapshot {
            stage: resource.map_or("ready", |resource| resource.stage),
            source: resource.map_or_else(String::new, |resource| resource.source.clone()),
            completed: self.total - self.pending.len() as u32,
            total: self.total,
        }
    }

    pub fn finish(&self) -> Result<(), String> {
        if let Some(resource) = self.pending.first() {
            return Err(format!("Startup still requires {}", resource.source));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_the_first_unfinished_source_after_out_of_order_completions() {
        let mut progress = StartupProgress::new([
            StartupResource::new("interface", "Title.png"),
            StartupResource::new("interface", "NativeUI.png"),
            StartupResource::new("interface", "Buttons.png"),
        ])
        .unwrap();
        progress.enter_phase("interface").unwrap();
        progress.complete("NativeUI.png").unwrap();
        progress.complete("Buttons.png").unwrap();
        assert_eq!(
            progress.snapshot(),
            Snapshot { stage: "interface", source: "Title.png".to_owned(), completed: 2, total: 6 }
        );
        progress.complete("Title.png").unwrap();
        assert_eq!(progress.snapshot().source, RUNTIME);
    }

    #[test]
    fn one_held_font_does_not_hide_eight_other_completed_faces() {
        let mut progress = StartupProgress::new(
            (0..9).map(|index| StartupResource::new("fonts", format!("font-{index}.woff2"))),
        )
        .unwrap();
        progress.enter_phase("fonts").unwrap();
        for index in 1..9 {
            progress.complete(&format!("font-{index}.woff2")).unwrap();
        }
        assert_eq!(
            progress.snapshot(),
            Snapshot { stage: "fonts", source: "font-0.woff2".to_owned(), completed: 8, total: 12 }
        );
    }

    #[test]
    fn deduplicates_planned_sources_and_rejects_duplicate_completion_or_early_handoff() {
        let mut progress = StartupProgress::new([
            StartupResource::new("interface", "Title.png"),
            StartupResource::new("interface", "Title.png"),
        ])
        .unwrap();
        assert_eq!(progress.snapshot().total, 4);
        progress.complete("Title.png").unwrap();
        let previous = progress.snapshot();
        assert!(progress.complete("Title.png").is_err());
        assert_eq!(progress.snapshot(), previous);
        assert!(progress.finish().is_err());
        for source in [RUNTIME, ACCOUNT, FIRST_FRAME] {
            progress.complete(source).unwrap();
        }
        assert!(progress.finish().is_ok());
        assert_eq!(
            progress.snapshot(),
            Snapshot { stage: "ready", source: String::new(), completed: 4, total: 4 }
        );
    }
}
