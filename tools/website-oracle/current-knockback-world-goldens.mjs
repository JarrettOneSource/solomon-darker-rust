#!/usr/bin/env node
// Actual area callback: two-pass actor physics followed by immediate committed world roots.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const game = await loadKernel('core-server/game-simulation.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const work = await loadKernel('core-server/enemies/work.ts')
const construction = await loadKernel('core-server/enemies/construction.ts')
const placement = await loadKernel('core-server/boneyard-world-placement.ts')
const model = await loadKernel('core-server/enemies/model.ts')
const manager = await loadKernel('core-kernels/native-world-manager-order.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const bits = value => {
  if (typeof value === 'number') { const b = Buffer.alloc(8); b.writeDoubleBE(value); return `f64:${b.toString('hex')}` }
  if (Array.isArray(value)) return value.map(bits)
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, bits(item)]))
  return value
}
const loaded = { choice: { id: 'empty', name: 'Empty', source: 'default' }, geometrySha256: 'b'.repeat(64),
  sourceSha256: 'c'.repeat(64), runId: 'knockback-world', seed: 'knockback-world', scene: {
    bounds: { x: 0, y: 0, w: 500, h: 500 }, environmentMode: 2, fences: [], name: 'Knockback', objects: [], roads: [],
    solomonDig: null, spawn: { facingDeg: 180, x: 250, y: 250 }, sprites: [], terrain: [],
  } }
const intents = [100, 145, 185].map((x, index) => ({ enemyToken: 'SKELETON', nativeTypeId: 1001, flags: [], id: index + 1,
  locationPolicy: 'anywhere', position: { x, y: 100 }, spawnTick: 0, waveOrdinal: 31 }))
const snapshot = world => ({ actors: world.enemies.actors.map(actor => ({ id: actor.id, position: actor.position,
  radius: model.boneyardEnemyCollisionRadius(actor) })), lantern: world.lanternPosition })
const cases = []
for (const nearbyPlayer of [false, true]) for (const lantern of [false, true]) {
  let state = game.enterBoneyardWorld(game.createGameSimulation({ owner: { element: 'ether', discipline: 'arcane', displayName: 'Owner' } }, { gameRngSeed: 17, combatRngSeed: 91 }), loaded)
  const order = manager.createNativeWorldManagerOrder()
  const ctx = { tick: 0, players: {}, resolveSpawnIntents: () => [], registerWorldPainter: order.register,
    projectileWorldBlocked: () => false, resolveMovement: request => request.requestedPosition }
  const initial = work.createEnemyWork(state.world.enemies, ctx, true)
  initial.actors.push(...construction.materializeSpawnIntents(initial, ctx, intents))
  let world = { ...state.world, enemies: work.finishEnemyStore(initial, 0), lanternPosition: lantern ? { x: 144, y: 116 } : null }
  const players = { owner: { ...entities.playerCharacterAt(state.playerEntities, 'owner'), position: nearbyPlayer ? { x: 143, y: 120 } : { x: 400, y: 400 } } }
  const combat = { owner: { alive: true, collisionEnabled: true, eligible: true, movementScale: 1 } }
  const frames = [{ expected: bits(snapshot(world)) }]
  for (const move of [{ id: 1, delta: { x: 20, y: 0 } }, { id: 2, delta: { x: 20, y: 5 } }, { id: 1, delta: { x: -10, y: 5 } }]) {
    const result = placement.moveBoneyardKnockbackAreaTarget(world, players, move.id, move.delta, 99, combat)
    world = result.world
    frames.push({ move, contacts: result.contactIds, expected: bits(snapshot(world)) })
  }
  cases.push({ nearbyPlayer, lantern, playerPosition: players.owner.position, frames })
}
writeFixture('current-knockback-world-goldens.json', 'tools/website-oracle/current-knockback-world-goldens.mjs',
  'current boneyard-world-placement.ts moveBoneyardKnockbackAreaTarget and actual actor physics over constructed enemies',
  { sourceRevision, intents, cases })
