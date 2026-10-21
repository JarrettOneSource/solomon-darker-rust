#!/usr/bin/env node
// Public lethal damage, normal/GameOver epochs and actual owner removal.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const burst = await loadKernel('renderer/player-death-burst-presentation.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], {
  cwd: websiteDir, encoding: 'utf8',
}).trim()
const configs = {
  owner: { discipline: 'arcane', displayName: 'Owner', element: 'ether' },
  peer: { discipline: 'arcane', displayName: 'Peer', element: 'fire' },
}
const loaded = { choice: { id: 'empty', name: 'Empty', source: 'default' },
  geometrySha256: 'b'.repeat(64), sourceSha256: 'c'.repeat(64), runId: 'death-weapons', seed: 'weapons',
  scene: { bounds: { x: 0, y: 0, w: 500, h: 500 }, environmentMode: 2, fences: [],
    name: 'Death weapons', objects: [], roads: [], solomonDig: null,
    spawn: { facingDeg: 180, x: 250, y: 250 }, sprites: [], terrain: [] } }
const bits = value => {
  if (typeof value === 'number') {
    const buffer = Buffer.alloc(8)
    buffer.writeDoubleBE(value)
    return `f64:${buffer.toString('hex')}`
  }
  if (Array.isArray(value)) return value.map(bits)
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, bits(item)]))
  }
  return value
}
const project = state => ({ tick: state.tick, run: state.run,
  players: state.playerEntities.identities.map(({ playerId }, index) => {
    const player = state.playerEntities.progressions[index]
    return { id: playerId, health: player.currentHealth, life: player.lifeState,
      deathAgeTicks: player.deathAgeTicks, deathTick: player.deathTick, deathEpoch: player.deathEpoch,
      corpseConsumed: player.corpseConsumed,
      registration: state.playerEntities.lightings[index].deathWeaponPainterRegistration }
  }),
  weapons: state.world.kind === 'boneyard' ? state.world.deathWeapons : [],
  nextWeaponId: state.world.kind === 'boneyard' ? state.world.nextDeathWeaponId : null,
  order: state.worldManagerOrder, secondaryRng: state.secondaryAbilities.rng,
})
const specs = [
  { name: 'live-peer', dead: ['owner'], removeOwner: false, hub: false },
  { name: 'owner-removal', dead: ['owner'], removeOwner: true, hub: false },
  { name: 'game-over', dead: ['owner', 'peer'], removeOwner: false, hub: false },
  { name: 'hub-without-world-animation', dead: ['owner'], removeOwner: false, hub: true },
]
const cases = []
for (const spec of specs) {
  let state = game.createGameSimulation(configs, { gameRngSeed: 17, combatRngSeed: 91 })
  if (!spec.hub) state = game.enterBoneyardWorld(state, loaded)
  for (const id of spec.dead) state = game.damageGameSimulationPlayer(state, id, 10000, 0)
  const frames = []
  for (let tick = 1; tick <= 600; tick += 1) {
    if (spec.removeOwner && tick === 20) state = game.removePlayerCharacter(state, 'owner')
    state = game.stepGameSimulationTick(state, {})
    if (tick <= 5 || tick % 50 === 0 || [19, 20, 21, 130, 264, 265, 266, 267, 399, 400, 401].includes(tick)) {
      frames.push({ tick, expected: bits(project(state)) })
    }
  }
  cases.push({ spec, frames })
}
// A consumed corpse remains a legal presentation row through the death-frame
// crossing. This pure-view boundary must suppress its one-shot burst as well.
const burstCases = []
for (const corpseConsumed of [false, true]) {
  const observation = deathTick => ({ world: { kind: 'boneyard', runId: 'death-weapons' },
    players: { owner: { position: { x: 250, y: 250 },
      progression: { deathEpoch: 1, deathTick, corpseConsumed } } } })
  const tracker = new burst.PlayerDeathBurstCrossingTracker(observation(263))
  const expected = tracker.update(observation(264))
  burstCases.push({ corpseConsumed, expectedCount: expected.length })
}
writeFixture('current-death-weapon-session-goldens.json',
  'tools/website-oracle/current-death-weapon-session-goldens.mjs',
  'current game-simulation.ts actual death births/normal and GameOver stepping, native-death-animations.ts',
  { sourceRevision, cases, burstCases })
