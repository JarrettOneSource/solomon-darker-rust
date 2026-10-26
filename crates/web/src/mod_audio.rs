//! Authored world audio and component sound notifications.
//! The accepted runtime owns actors/events. This session-scoped observer only remembers
//! heard IDs and owned audio loops, using the existing GameAudio context.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use crate::platform::spawn_local;
use solomon_sim::audio::AudioOptions;
use solomon_sim::modding::assets::GameModAsset;
use solomon_sim::modding::presentation::{
    project_mod_enemies, project_mod_powerup_events, project_mod_spell_effects,
};
use solomon_sim::modding::{LuaObject, LuaValue};
use std::sync::Arc;

use crate::audio::GameAssetAudio;
use crate::mod_content::{GameModContentLoadError, ModContentCache};

#[derive(Clone)]
struct ModAudioSource {
    key: String,
    bytes: Arc<[u8]>,
    url: String,
}

pub struct ModAudioSources {
    sources: BTreeMap<String, ModAudioSource>,
}

impl ModAudioSources {
    /// Retain verified encoded bytes for platform decoders. Browser music also uses
    /// the same-origin URL when an accepted runtime loop starts playback.
    pub async fn load(
        cache: &ModContentCache,
        assets: &[GameModAsset],
    ) -> Result<Self, GameModContentLoadError> {
        let mut sources = BTreeMap::new();
        for asset in assets {
            if asset.kind != "audio" || sources.contains_key(&asset.sha256) {
                continue;
            }
            let bytes = Arc::from(cache.bytes(asset).await?);
            let url = cache.content_url(asset);
            sources.insert(
                asset.sha256.clone(),
                ModAudioSource { key: source_key(asset), bytes, url },
            );
        }
        Ok(Self { sources })
    }

    fn source(&self, asset: &GameModAsset) -> Result<ModAudioSource, String> {
        self.sources.get(&asset.sha256).cloned().ok_or_else(|| {
            format!("mod audio asset is unavailable: {}:{}", asset.mod_id, asset.path)
        })
    }
}

#[derive(Clone)]
struct DesiredAudio {
    owner: String,
    bus: String,
    source: ModAudioSource,
    volume: f64,
}

pub struct ModAudioDriver {
    audio: GameAssetAudio,
    sources: ModAudioSources,
    active: Rc<RefCell<Vec<DesiredAudio>>>,
    desired: Rc<RefCell<Vec<DesiredAudio>>>,
    world_active: Rc<Cell<bool>>,
    heard: BTreeSet<String>,
    enemy_heard: BTreeSet<String>,
    effects_seen: BTreeSet<i64>,
    powerup_heard: BTreeSet<(i64, i64)>,
}

impl ModAudioDriver {
    #[must_use]
    pub fn new(audio: GameAssetAudio, sources: ModAudioSources) -> Self {
        Self {
            audio,
            sources,
            active: Rc::new(RefCell::new(Vec::new())),
            desired: Rc::new(RefCell::new(Vec::new())),
            world_active: Rc::new(Cell::new(true)),
            heard: BTreeSet::new(),
            enemy_heard: BTreeSet::new(),
            effects_seen: BTreeSet::new(),
            powerup_heard: BTreeSet::new(),
        }
    }

    /// Call for actual snapshot/runtime notifications, never the interpolated draw copy.
    /// Only a new ModRuntime revision triggers Source ModAudio's global effect/loop effect.
    pub fn update(
        &mut self,
        runtime: Option<&LuaObject>,
        assets: &[GameModAsset],
        tick: u64,
        runtime_updated: bool,
    ) -> Result<(), String> {
        if runtime_updated {
            self.update_global(runtime, assets, tick)?;
        }
        for enemy in project_mod_enemies(runtime, assets) {
            let (Some(sound), Some(event_tick)) = (enemy.sound, enemy.sound_event_tick) else {
                continue;
            };
            if tick as f64 - event_tick as f64 > 20.0 {
                continue;
            }
            let key = format!("{}:{event_tick}:{}", enemy.id, source_key(sound));
            if !self.enemy_heard.insert(key) {
                continue;
            }
            self.play(sound, enemy.sound_volume, true)?;
        }
        if self.enemy_heard.len() > 4096 {
            self.enemy_heard.clear();
        }
        for event in project_mod_powerup_events(runtime, assets) {
            let key = (event.id, event.tick);
            let Some(sound) = event.sound else { continue };
            if self.powerup_heard.contains(&key) || tick as f64 - event.tick as f64 > 20.0 {
                continue;
            }
            self.powerup_heard.insert(key);
            self.play(sound, event.sound_volume, false)?;
        }
        let effects = project_mod_spell_effects(runtime, assets);
        for effect in &effects {
            if !self.effects_seen.insert(effect.id) {
                continue;
            }
            let Some(sound) = effect.sound else { continue };
            if tick as f64 - effect.started_tick as f64 > 20.0 {
                continue;
            }
            self.play(sound, effect.sound_volume, true)?;
        }
        if self.effects_seen.len() > 4096 {
            let live: BTreeSet<_> = effects.iter().map(|effect| effect.id).collect();
            self.effects_seen.retain(|id| live.contains(id));
        }
        Ok(())
    }

    fn update_global(
        &mut self,
        runtime: Option<&LuaObject>,
        assets: &[GameModAsset],
        tick: u64,
    ) -> Result<(), String> {
        let loops = rows(runtime, "audio_loops");
        let mut next: Vec<DesiredAudio> = Vec::new();
        for row in &loops {
            let owner = text(row.get("owner"));
            let Some(asset) = source_for(row, assets) else { continue };
            if owner.is_empty() {
                continue;
            }
            let volume = loops
                .iter()
                .find(|row| text(row.get("owner")) == owner)
                .map_or(0.0, |row| finite(row.get("volume")));
            let wanted = DesiredAudio {
                owner: owner.into(),
                bus: text(row.get("bus")).into(),
                source: self.sources.source(asset)?,
                volume,
            };
            if let Some(index) = next.iter().position(|entry| entry.owner == owner) {
                // Map.set keeps the first insertion position and the final source value;
                // Source volume deliberately comes from the first owner row.
                next[index] = wanted;
            } else {
                next.push(wanted);
            }
        }
        *self.desired.borrow_mut() = next.clone();
        let previous = std::mem::take(&mut *self.active.borrow_mut());
        for current in previous {
            if next.iter().any(|wanted| {
                wanted.owner == current.owner
                    && wanted.source.key == current.source.key
                    && wanted.bus == current.bus
            }) {
                // Source ignores a volume-only update while this same loop remains active.
                self.active.borrow_mut().push(current);
            } else {
                stop(&self.audio, &current)?;
            }
        }
        for wanted in next {
            if self.active.borrow().iter().any(|current| current.owner == wanted.owner) {
                continue;
            }
            if wanted.bus == "music" {
                self.audio.start_asset_music(
                    &wanted.owner,
                    wanted.source.url.as_str(),
                    wanted.source.bytes.clone(),
                    wanted.volume,
                )?;
                self.active.borrow_mut().push(wanted);
                continue;
            }
            let audio = self.audio.clone();
            let desired = self.desired.clone();
            let active = self.active.clone();
            spawn_local(async move {
                // Source silently ignores individual authored audio decode failures.
                if audio.load_asset(&wanted.source.key, &wanted.source.bytes).await.is_err() {
                    return;
                }
                if !desired.borrow().iter().any(|current| {
                    current.owner == wanted.owner && current.source.key == wanted.source.key
                }) {
                    return;
                }
                // The source completion checks the source only, not its bus or volume.
                match audio.start_asset_loop(
                    &wanted.owner,
                    &wanted.source.key,
                    options(wanted.volume),
                ) {
                    Ok(()) => {
                        let mut active = active.borrow_mut();
                        if let Some(index) =
                            active.iter().position(|current| current.owner == wanted.owner)
                        {
                            active[index] = wanted;
                        } else {
                            active.push(wanted);
                        }
                    }
                    Err(error) => report(&error),
                }
            });
        }
        for event in rows(runtime, "presentation_events") {
            let sequence =
                format!("{}:{}", text(event.get("mod_id")), integer(event.get("sequence")));
            let Some(asset) = source_for(event, assets) else { continue };
            if self.heard.contains(&sequence)
                || tick as f64 - integer(event.get("tick")) as f64 > 20.0
            {
                continue;
            }
            self.heard.insert(sequence);
            self.play(asset, finite(event.get("volume")), false)?;
        }
        Ok(())
    }

    fn play(&self, asset: &GameModAsset, volume: f64, guard_unload: bool) -> Result<(), String> {
        let source = self.sources.source(asset)?;
        let audio = self.audio.clone();
        let active = self.world_active.clone();
        spawn_local(async move {
            if audio.load_asset(&source.key, &source.bytes).await.is_err() {
                return;
            }
            if guard_unload && !active.get() {
                return;
            }
            // Enemy/effect components guard unmount. Source global/pickup callbacks do not.
            if let Err(error) = audio.play_asset(&source.key, options(volume)) {
                report(&error);
            }
        });
        Ok(())
    }
}

impl Drop for ModAudioDriver {
    fn drop(&mut self) {
        self.world_active.set(false);
        self.desired.borrow_mut().clear();
        for current in self.active.borrow_mut().drain(..) {
            if let Err(error) = stop(&self.audio, &current) {
                report(&error);
            }
        }
    }
}

fn stop(audio: &GameAssetAudio, current: &DesiredAudio) -> Result<(), String> {
    if current.bus == "music" {
        audio.stop_asset_music(&current.owner)
    } else {
        audio.stop_asset_loop(&current.owner)
    }
}

fn source_key(asset: &GameModAsset) -> String {
    format!("/api/game/content/{}", asset.sha256)
}

fn options(volume: f64) -> AudioOptions {
    AudioOptions { volume, ..AudioOptions::default() }
}

fn rows<'a>(runtime: Option<&'a LuaObject>, key: &str) -> Vec<&'a LuaObject> {
    let mut rows = Vec::new();
    if let Some(values) = runtime.and_then(|runtime| runtime.get(key)).and_then(LuaValue::as_array)
    {
        for value in values {
            if let Some(value) = value.as_object() {
                rows.push(value);
            }
        }
    }
    rows
}

fn source_for<'a>(row: &LuaObject, assets: &'a [GameModAsset]) -> Option<&'a GameModAsset> {
    let mod_id = row.get("mod_id").and_then(LuaValue::as_str)?;
    let path = row.get("path").and_then(LuaValue::as_str)?;
    assets
        .iter()
        .find(|asset| asset.mod_id == mod_id && asset.path == path && asset.kind == "audio")
}

fn text(value: Option<&LuaValue>) -> &str {
    value.and_then(LuaValue::as_str).unwrap_or("")
}
fn finite(value: Option<&LuaValue>) -> f64 {
    value.and_then(LuaValue::as_number).filter(|value| value.is_finite()).unwrap_or(0.0)
}
fn integer(value: Option<&LuaValue>) -> i64 {
    let value = finite(value);
    if value.fract() == 0.0 && value.abs() <= 9_007_199_254_740_991.0 { value as i64 } else { 0 }
}
fn report(error: &str) {
    crate::platform::error(error);
}
