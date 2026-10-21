#!/usr/bin/env python3
"""Copy the pinned Website's existing asset bindings into a closed Rust catalog.

Only authored bindings are generated. Playback, mixing and ownership remain readable Rust.
No imported TypeScript or JavaScript runs in the shipped game.
"""
import os
import json
import re
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parents[2]
website = Path(os.environ.get("SOLOMON_WEBSITE_DIR", root.parent / "Website"))
source = (website / "frontend/src/game/game-audio-assets.ts").read_text()
imports = dict(re.findall(r"import (\w+) from '([^']+)'", source))
rows = []
for group, body in re.findall(r"  (loops|music|sounds|streams): \{(.*?)\n  \}", source, re.S):
    for line in body.splitlines():
        line = line.strip().rstrip(",")
        if not line:
            continue
        if ":" in line:
            cue, binding = map(str.strip, line.split(":", 1))
            cue = cue.strip("'")
        else:
            cue = binding = line
        path = imports[binding].split("assets/", 1)[1]
        path = path.replace("game/audio/", "")
        variant = "".join(word.capitalize() for word in cue.split("-"))
        kind = {"loops": "Loop", "music": "Music", "sounds": "Sound", "streams": "Stream"}[group]
        rows.append((variant, cue, path, kind))
assert len(set(row[0] for row in rows)) == len(rows)
header = '''//! Generated stock bindings from Website `game-audio-assets.ts`.
//! Regenerate with tools/website-oracle/audio-catalog.py. Assets stay outside git.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioKind { Sound, Stream, Loop, Music }

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
#[repr(usize)]
pub enum AudioCue {
'''
text = header + "".join(f"    {variant},\n" for variant, *_ in rows) + "}\n\n"
text += '''pub struct AudioDefinition {
    pub cue: AudioCue,
    pub id: &'static str,
    pub path: &'static str,
    pub kind: AudioKind,
}

pub const AUDIO_CATALOG: &[AudioDefinition] = &[
'''
for variant, cue, path, kind in rows:
    text += f'    AudioDefinition {{ cue: AudioCue::{variant}, id: "{cue}", path: "{path}", kind: AudioKind::{kind} }},\n'
text += '''];

impl AudioCue {
    #[must_use]
    pub fn definition(self) -> &'static AudioDefinition { &AUDIO_CATALOG[self as usize] }
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        AUDIO_CATALOG.iter().find(|definition| definition.id == id).map(|definition| definition.cue)
    }
    #[must_use]
    pub fn as_str(self) -> &'static str { self.definition().id }
    #[must_use]
    pub fn maximum_voices(self) -> Option<usize> {
        match self {
            Self::FireballHit | Self::ThrowFire | Self::Harden | Self::IceShatter | Self::HailBounce0 | Self::HailBounce1 | Self::HailBounce2 | Self::HailBounce3 => Some(10),
            _ => None,
        }
    }
}
'''
out = root / "crates/sim/src/audio/catalog.rs"
out.parent.mkdir(parents=True, exist_ok=True)
out.write_text(text)
native = (website / "frontend/src/game/game-audio-native.ts").read_text()
limits_body = re.search(r"NATIVE_SOUND_MAXIMUM_VOICES.*?Object.freeze\(\{(.*?)\}\)", native, re.S).group(1)
limits = dict((key.strip("'"), int(count))
              for key, count in re.findall(r"([\w'-]+):\s*(\d+)", limits_body))
fixture = {
    "source": "game-audio-assets.ts and game-audio-native.ts",
    "generator": "tools/website-oracle/audio-catalog.py",
    "sourceRevision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=website, text=True).strip(),
    "bindings": [{"id": cue, "path": path, "kind": kind, "maximumVoices": limits.get(cue)}
                 for _, cue, path, kind in rows],
}
fixture_path = root / "crates/sim/tests/fixtures/website/current-audio-catalog-goldens.json"
fixture_path.write_text(json.dumps(fixture, ensure_ascii=False, separators=(",", ":")) + "\n")
print(f"wrote {len(rows)} stock audio bindings")
