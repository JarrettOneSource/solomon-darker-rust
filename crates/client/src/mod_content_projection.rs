//! `client/game-client-session.ts`'s retained ModContent projection.
//! Catalogue metadata is replaced on equal/newer revision and reset per session.

use solomon_sim::modding::LuaObject;
use solomon_sim::modding::projection::ModContentProjection;

#[derive(Default)]
pub struct ModContentState {
    current: Option<ModContentProjection>,
}

impl ModContentState {
    pub fn reset(&mut self) {
        self.current = None;
    }

    #[must_use]
    pub fn current(&self) -> Option<&ModContentProjection> {
        self.current.as_ref()
    }

    /// Source decodes a message before checking its revision. Equal revisions
    /// still replace the projection; their powerup/status rows can differ.
    pub fn receive(&mut self, source: &LuaObject) -> Result<bool, String> {
        let next = ModContentProjection::read(source)?;
        if self.current.as_ref().is_some_and(|current| next.revision < current.revision) {
            return Ok(false);
        }
        self.current = Some(next);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solomon_sim::modding::LuaValue;

    fn projection(revision: f64, digest: char) -> LuaObject {
        let mut source = LuaObject::new();
        for name in ["boasts", "content", "powerups", "statuses"] {
            source.insert(name.into(), LuaValue::Array(Vec::new()));
        }
        source.insert("manifestSha256".into(), LuaValue::String(digest.to_string().repeat(64)));
        source.insert("revision".into(), LuaValue::Number(revision));
        source
    }

    #[test]
    fn content_revisions_replace_equal_reject_older_and_reset_per_session() {
        let mut state = ModContentState::default();
        assert!(state.current().is_none());
        assert!(state.receive(&projection(7.0, 'a')).unwrap());
        assert!(!state.receive(&projection(6.0, 'b')).unwrap());
        assert_eq!(state.current().unwrap().manifest_sha256, "a".repeat(64));
        assert!(state.receive(&projection(7.0, 'b')).unwrap());
        assert_eq!(state.current().unwrap().manifest_sha256, "b".repeat(64));

        // Source decodes even a stale message before its revision comparison.
        let mut invalid = projection(6.0, 'a');
        invalid.insert("powerups".into(), LuaValue::Null);
        assert!(state.receive(&invalid).is_err());
        assert_eq!(state.current().unwrap().manifest_sha256, "b".repeat(64));
        state.reset();
        assert!(state.current().is_none());
        assert!(state.receive(&projection(0.0, 'c')).unwrap());
        assert_eq!(state.current().unwrap().manifest_sha256, "c".repeat(64));
    }
}
