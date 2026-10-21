#!/usr/bin/env node
// Accepted snapshot edges through the unchanged source synchronizers. RecordingAudio
// replaces playback only; source cue selection, counter handling and loop mixing run intact.
import { loadKernel, writeFixture } from './oracle.mjs'

const { PrimarySpellAudioSynchronizer } = await loadKernel('primary-spell-audio.ts')
const native = await loadKernel('game-audio-native.ts')
const skills = await loadKernel('air-water-skill-audio.ts')
const { nativeBoneyardEnemyAmbientRequests } = await loadKernel('boneyard-enemy-ambient-audio.ts')
const { nativeWeldCastSoundCues, nativeWeldLoopCues } = await loadKernel('weld-primary-audio-contract.ts')
const clone = value => structuredClone(value)
const worldKey = 'boneyard:audio-run'

class RecordingAudio {
  events = []
  loops = []
  playSound(cue, options = {}) { this.events.push({ cue, kind: 'sound', playbackRate: options.playbackRate ?? 1, volume: options.volume ?? 1 }) }
  playStream(cue, options = {}) { this.events.push({ cue, kind: 'stream', playbackRate: options.playbackRate ?? 1, volume: options.volume ?? 1 }) }
  startLoop(cue, owner, options = {}) { this.loops.push({ action: 'start', cue, owner, playbackRate: options.playbackRate ?? 1, volume: options.volume ?? 1 }) }
  stopLoop(cue, owner) { this.loops.push({ action: 'stop', cue, owner }) }
  drain() {
    const output = { events: this.events, loops: this.loops }
    this.events = []; this.loops = []
    return output
  }
}
function snapshot(element = 'ether') {
  return {
    players: { local: { config: { element }, position: { x: 100, y: 200 }, footstepTick: 0,
      progression: { selectedPrimarySkillId: { ether: 8, fire: 16, air: 24, water: 32, earth: 40 }[element], weldBuildId: null },
      primaryCast: { castSequence: 0, emissionSequence: 0, fizzleSequence: 0, etherBlastChargeCueSequence: 0,
        channelActive: false, underpowered: false, lastWeldPlaybackRate: null, lastWeldSoundVariant: null } } },
    world: { kind: 'boneyard', runId: 'audio-run' },
    primarySpells: { projectiles: [], transients: [] },
    secondaryAbilities: { players: {}, actors: [], events: [], nextEventId: 1 },
  }
}
function scenario(name, initial, updates) {
  const audio = new RecordingAudio()
  const sync = new PrimarySpellAudioSynchronizer(audio, 'local', initial)
  const frames = [{ snapshot: clone(initial), expected: audio.drain() }]
  let state = clone(initial)
  for (const update of updates) {
    update(state)
    sync.update(clone(state))
    frames.push({ snapshot: clone(state), expected: audio.drain() })
  }
  sync.destroy()
  return { name, frames, destroy: audio.drain() }
}
const scenarios = []
for (const element of ['ether', 'fire', 'air', 'earth', 'water']) {
  const initial = snapshot(element)
  if (element === 'earth') initial.primarySpells.projectiles.push({ kind: 'earth', id: 1, ownerId: 'local',
    worldKey, phase: 'held', charge: 0.5, position: { x: 130, y: 200 } })
  scenarios.push(scenario(element, initial, [
    state => { state.players.local.primaryCast.castSequence = 1; state.players.local.primaryCast.channelActive = true },
    () => {},
    state => { const cast = state.players.local.primaryCast; cast.emissionSequence = 3; cast.fizzleSequence = 1;
      cast.etherBlastChargeCueSequence = 2; cast.underpowered = true },
    state => { state.players.local.primaryCast.channelActive = false;
      for (const spell of state.primarySpells.projectiles) spell.phase = 'flight' },
    state => { state.primarySpells.projectiles = [] },
  ]))
}
const weld = []
for (let buildId = 1000; buildId <= 1009; buildId++) {
  const variant = buildId === 1002 ? 1 : buildId === 1009 ? 2 : null
  weld.push({ buildId, variant, sounds: nativeWeldCastSoundCues(buildId, variant), loops: nativeWeldLoopCues(buildId) })
  const initial = snapshot()
  initial.players.local.progression = { selectedPrimarySkillId: 52, weldBuildId: buildId }
  scenarios.push(scenario(`weld-${buildId}`, initial, [
    state => { Object.assign(state.players.local.primaryCast, { castSequence: 2, channelActive: true,
      lastWeldPlaybackRate: 1.125, lastWeldSoundVariant: variant }) },
    state => { Object.assign(state.players.local.primaryCast, { emissionSequence: 2, underpowered: true }) },
    state => { state.players.local.primaryCast.channelActive = false },
  ]))
}
const common = (kind, id, extra) => ({ kind, id, ownerId: 'local', worldKey, ...extra })
const initial = snapshot()
scenarios.push(scenario('transient-birth-and-counter-edges', initial, [
  state => { state.primarySpells.transients = [
    common('ether-blast', 1, { origin: { x: 150, y: 200 } }),
    common('fire-impact', 2, { origin: { x: 300, y: 200 } }),
    common('fire-explosion', 3, { origin: { x: 800, y: 200 }, soundPitch: 0.975, presentation: 'fire' }),
    common('fire-explosion', 4, { origin: { x: 300, y: 200 }, soundPitch: 1.05, presentation: 'steam' }),
    common('earth-impact', 5, { origin: { x: 400, y: 200 }, charge: 0.75 }),
    common('ether-impact', 6, { origin: { x: 300, y: 200 } }),
    common('fire-good-imp', 7, { position: { x: 300, y: 200 }, bounceSoundSequence: 2,
      bounceSoundIndex: 3, bounceSoundPitch: 1.03125, contactSoundSequence: 1,
      contactSoundIndex: 2, contactSoundPitch: 0.96875, contactOrigin: { x: 400, y: 200 } }),
    common('water-hail', 8, { position: { x: 300, y: 200 }, bounceSoundSequence: 2, bounceSoundIndex: 1, bounceSoundPitch: 0.9 }),
    common('player-staff-contact', 9, { origin: { x: 400, y: 200 }, swooshPitch: 0.95,
      impactSoundPitches: [0.9, 1.1], pikeBreakSoundIndexes: [1], procSound: 'critical-hit', procSoundPitches: [1, 1.125] }),
    common('weld-meteor', 10, { position: { x: 700, y: 200 }, phase: 'fall', underpowered: false,
      impactSoundPitch: null, impactThrowFirePitch: null }),
    common('weld-persistent', 11, { position: { x: 100, y: 200 }, buildId: 1008, phase: 'held' }),
    ...[1001, 1002, 1006, 1009].map((buildId, index) => common('weld-impact', 20 + index, {
      buildId, position: { x: 500, y: 200 }, impactSoundPitch: 1.125, impactSoundVariant: 2,
      boulderTerminalCharge: buildId === 1006 ? 0.5 : null })),
  ]; state.secondaryAbilities.actors = [common('mindblast-burst', 30, { position: { x: 300, y: 200 } })] },
  () => {},
  state => { const imp = state.primarySpells.transients.find(actor => actor.id === 7)
    imp.bounceSoundSequence += 2; imp.contactSoundSequence += 2
    state.primarySpells.transients.find(actor => actor.id === 8).bounceSoundSequence += 3
    Object.assign(state.primarySpells.transients.find(actor => actor.id === 10), {
      phase: 'impact', impactSoundPitch: 0.875, impactThrowFirePitch: 1.03125 })
    state.primarySpells.transients.find(actor => actor.id === 11).phase = 'flight' },
  state => { state.primarySpells.transients = []; state.secondaryAbilities.actors = [] },
]))
const secondaryCues = ['dampen', 'golem-die', 'golem-provoke', 'leviathan-roar', 'mindstar', 'planewalker-off',
  'planewalker-on', 'prismatic-shock', 'quake-crack-small', 'quake-cracks', 'set-trap', 'stoneskin-on', 'thunder',
  'trap', 'magic-missile', 'fizzle', 'comet-loop', 'electric-loop', 'earthquake-loop', 'low-fire-loop',
  'plane-cross-loop', 'rainfall-loop', 'steady-wind-loop']
scenarios.push(scenario('secondary-cues-and-loop-mix', snapshot(), [
  state => { state.secondaryAbilities.events = secondaryCues.map((cue, index) => ({
    eventId: index + 1, worldKey, cue, gain: 0.75, pitch: 1.03125, position: { x: 300, y: 200 } }))
    state.secondaryAbilities.nextEventId = secondaryCues.length + 1
    state.secondaryAbilities.actors = ['moving-fire', 'fire-patch', 'storm-cloud', 'acid-rain', 'earthquake',
      'comet', 'electric-burn', 'ether-drain', 'leviathan'].map((kind, index) => common(kind, 40 + index,
        { position: { x: 300 + index * 30, y: 200 }, phase: 0.5, ageTicks: index === 6 ? 0 : 1 }))
    state.secondaryAbilities.players.local = { planewalkerTicksRemaining: 50 } },
  state => { state.secondaryAbilities.actors.find(actor => actor.kind === 'electric-burn').ageTicks = 1 },
  state => { state.secondaryAbilities.actors = []; state.secondaryAbilities.players.local.planewalkerTicksRemaining = 0 },
]))
const enemyEvents = []
for (const type of ['enemy-action-sound', 'enemy-damage-sound', 'enemy-death-sound', 'player-damage-sound', 'player-status-sound', 'attack-marker']) {
  for (const sound of ['fireball-hit', 'poisoned', 'magic-missile-hit']) {
    const input = { type, sound, pitch: 0.875, gainScale: 0.6, sourcePosition: { x: 300, y: 200 } }
    enemyEvents.push({ input, expected: native.nativeEnemyEventSoundRequest(input) })
  }
}
enemyEvents.push({ input: { type: 'player-deflected', deflectPitch: 1.125 },
  expected: native.nativeEnemyEventSoundRequest({ type: 'player-deflected', deflectPitch: 1.125 }) })
const accepted = [0, 27, 30, 35, 72, 76, 99].map(skillId => ({ skillId, expected: skills.nativeAirWaterAcceptedCastAudioRequests(skillId) }))
const footsteps = []
for (const playerId of ['local', 'wizard-🧙']) {
  for (const [previous, current] of [[null, 9], [0, 0], [0, 9], [9, 9], [9, 20], [20, 4294967301]]) {
    const tick = native.newNativeFootstepTick(previous === null ? undefined : { footstepTick: previous }, { footstepTick: current })
    footsteps.push({ playerId, previous, current, expected: tick === undefined ? null : { tick, cue: native.nativeFootstepCue(tick, playerId) } })
  }
}
const lootEvents = [{}, { sound: 'pickup-coin' }, { playbackRate: 1.125 }, { sound: 'pickup-coin', playbackRate: 1.125 }]
  .map(input => { const event = { position: { x: 300, y: 200 }, ...input }
    return { input: event, expected: native.nativeLootEventSoundRequest(event) } })
const ambient = []
for (const state of ['idle', 'death']) {
  for (const token of ['ZOMBIE', 'DIREFACULTY', 'WRAITH', 'COFFIN', 'DEMONSKULL']) {
    const input = { world: { enemies: [{ id: 1, enemyToken: token, position: { x: 100, y: 200 },
      flags: ['FLAG_ROTTEN'], animation: { state }, faculty: { handMask: 3, lightningActive: true } }],
      maggots: Array.from({length:300}, () => ({ ownerCoffinActorId: 1, state: 'crawl' })), bossSpells: [] } }
    ambient.push({ input, expected: nativeBoneyardEnemyAmbientRequests(input, { point: () => 0.8, hit: () => 0.6 }) })
  }
}
for (const kind of ['green-fire', 'mouth-beam-segment', 'ultra-banish', 'rain-of-bones', 'tragic-circle', 'skull-missile', 'dark-fireball']) {
  const input = { world: { enemies: [{ id: 1, enemyToken: 'SKELETON', flags: [], animation: { state: 'idle' },
    position: { x: 100, y: 200 } }], maggots: [], bossSpells: [{ kind, ownerActorId: 1, position: { x: 300, y: 200 }, alpha: 0.75 }] } }
  ambient.push({ input, expected: nativeBoneyardEnemyAmbientRequests(input, { point: () => 0.8, hit: () => 0.6 }) })
}
writeFixture('combat-audio-goldens.json', 'tools/website-oracle/combat-audio-goldens.mjs',
  'Website/frontend/src/game/{primary-spell-audio,air-water-skill-audio,game-audio-native,boneyard-enemy-ambient-audio,weld-primary-audio-contract}.ts',
  { scenarios, enemyEvents, accepted, ambient, weld, footsteps, lootEvents })
