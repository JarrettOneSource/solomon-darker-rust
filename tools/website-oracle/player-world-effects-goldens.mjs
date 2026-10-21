// Shared death presentation and the level-up world sparkle program; no gameplay RNG.
import { loadKernel, writeFixture } from './oracle.mjs'

const burst = await loadKernel('renderer/player-death-burst-presentation.ts')
const weapon = await loadKernel('renderer/player-death-weapon-presentation.ts')
const level = await loadKernel('renderer/level-up-presentation.ts')
const bursts = []
for (const [runId, playerId, deathEpoch] of [
  ['run', 'hero', 1], ['two', 'remote', 3], ['réveil', '魔法😀', 2],
]) {
  const trigger = {
    runId, playerId, deathEpoch,
    key: `${runId}:${playerId}:${deathEpoch}`,
    position: { x: 100, y: 250 },
  }
  for (const age of [0, 1, 5, 9, 10]) {
    bursts.push({ trigger, age, layers: burst.playerDeathBurstLayers(trigger, age) })
  }
}
// Source retains the fractional presentation tick at the frame-three crossing.
for (const presentationTick of [301.25, 310.25, 310.75]) {
  const trigger = bursts[0].trigger
  const startedPresentationTick = 300.75
  const age = presentationTick - startedPresentationTick
  bursts.push({ trigger, age, startedPresentationTick, presentationTick,
    layers: burst.playerDeathBurstLayers(trigger, age) })
}
const weapons = []
for (const kind of ['staff', 'wand']) {
  for (const headingIndex of [0, 5, 12, 23]) {
    const trigger = {
      deathEpoch: 2, headingIndex, playerId: 'remote', runId: 'run',
      weapon: { kind, selector: kind === 'staff' ? 4 : 0 },
    }
    for (const age of [0, 2, 20, 79, 159, 298, 500]) {
      weapons.push({ trigger, age, sample: weapon.playerDeathWeaponSample(trigger, age) })
    }
  }
}
const levelUps = []
for (const presentationId of [1, 72, 4294967297]) {
  for (const elapsedMs of [-1, 0, 9.9, 10, 590, 1000, 1790, 1800, 2380, 2390]) {
    const playerScreenY = presentationId === 72 ? 0 : 320
    levelUps.push({
      presentationId, elapsedMs, playerScreenY,
      frame: level.nativeLevelUpPresentationFrame(presentationId, elapsedMs, playerScreenY),
    })
  }
}
// Actual tracker epochs cover late joins, clock rewind, repeat deaths, removal and new runs.
const observation = (id, epoch, tick, x = 100) => [id, {
  position: { x, y: 250 },
  progression: { deathEpoch: epoch, deathTick: tick },
}]
const inputs = [
  ['run', [observation('hero', 0, 0)]],
  ['run', [observation('hero', 1, 158)]],
  ['run', [observation('hero', 1, 159)]],
  ['run', [observation('hero', 1, 155)]],
  ['run', [observation('hero', 1, 159), observation('late', 2, 200)]],
  ['run', [observation('hero', 2, 160, 150), observation('late', 2, 201)]],
  ['run', []],
  ['run', [observation('hero', 2, 200)]],
  ['next', [observation('hero', 3, 200)]],
  ['next', [observation('hero', 4, 159, 200)]],
]
const snapshot = (runId, players) => ({
  world: { kind: 'boneyard', runId },
  players: Object.fromEntries(players),
})
const tracker = new burst.PlayerDeathBurstCrossingTracker(snapshot(...inputs[0]))
const crossings = inputs.map(([runId, players]) => ({
  runId,
  players: players.map(([id, player]) => ({ id, ...player })),
  triggers: tracker.update(snapshot(runId, players)),
}))
writeFixture('player-world-effects-goldens.json', 'player-world-effects-goldens.mjs', [
  'renderer/player-death-burst-presentation.ts',
  'renderer/player-death-weapon-presentation.ts',
  'renderer/level-up-presentation.ts',
], { bursts, weapons, levelUps, crossings })
