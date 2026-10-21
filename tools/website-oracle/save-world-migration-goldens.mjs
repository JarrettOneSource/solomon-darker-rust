// Real constructor births are saved through the public Game API, then continued through
// actual Game ticks after public restore. No test replacement steps or random samplers.
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, writeFixture } from './oracle.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const saves = await loadKernel('save/game-save-document.ts')
const { boneyardGeometrySha256 } = await loadKernel('host/project-boneyard.ts')
const { createEnemyWork, finishEnemyStore } = await loadKernel('core-server/enemies/work.ts')
const { materializeSpawnIntents } = await loadKernel('core-server/enemies/construction.ts')
const { emitEvent } = await loadKernel('core-server/enemies/events.ts')
const { createNativeWorldManagerOrder } = await loadKernel('core-kernels/native-world-manager-order.ts')
const births = JSON.parse(readFileSync(resolve(fixturesDir, 'enemy-construction-goldens.json')))
const playerId = 'continuation-owner'
const scene = { name: 'Continuation Arena', environmentMode: 0,
  bounds: { x: 0, y: 0, w: 2000, h: 2000 }, spawn: { x: 1000, y: 1000, facingDeg: 0 },
  objects: [], sprites: [], roads: [], fences: [], terrain: [], solomonDig: null }
const loaded = { scene, choice: { id: 'continuation-arena', name: scene.name, source: 'default' },
  sourceSha256: createHash('sha256').update(JSON.stringify(scene)).digest('hex'),
  geometrySha256: boneyardGeometrySha256(scene), seed: '00000000000000000000000000000000', runId: 'continuation-arena' }
let state = game.enterBoneyardWorld(game.createGameSimulation({ [playerId]: {
  displayName: 'Continuation Oracle', element: 'fire', discipline: 'mind',
} }, { gameRngSeed: 17, combatRngSeed: 91 }), loaded)
for (let tick = 0; tick < 40; tick++) state = game.stepGameSimulationTick(state, {})
const order = createNativeWorldManagerOrder(state.worldManagerOrder)
const context = { tick: state.tick, players: { [playerId]: { alive: true, connected: true, eligible: true,
  collisionRadius: 25, position: { x: 1000, y: 1000 }, velocityPerTick: { x: 0, y: 0 } } },
  resolveMovement: request => request.requestedPosition, registerWorldPainter: order.register }
const work = createEnemyWork(state.world.enemies, context, true)
work.actors = materializeSpawnIntents(work, context, births.cases[0].intents
  .map(intent => ({ ...intent, spawnTick: state.tick })))
state = { ...state, worldManagerOrder: order.state(),
  world: { ...state.world, enemies: finishEnemyStore(work, state.tick) } }
const baseline = JSON.parse(saves.createGameSaveDocument({ state, playerId, loadedBoneyard: loaded,
  integrity: 'global-clean', mods: [], modState: {} }))
const cases = []
function record(name, version, mutate = () => {}) {
  const document = structuredClone(baseline)
  document.schemaVersion = version
  const simulation = document.continuation.simulation
  mutate(simulation)
  try {
    const restored = saves.restoreGameSaveDocument(JSON.stringify(document))
    let state = restored.state
    const frames = [{ ticks: 0, simulation: state }]
    for (let tick = 1; tick <= 12; tick++) {
      state = game.stepGameSimulationTick(state, {})
      if ([1, 4, 12].includes(tick)) frames.push({ ticks: tick, simulation: state })
    }
    cases.push({ name, document, accepted: true, frames })
  } catch (error) { cases.push({ name, document, accepted: false, error: error.message }) }
}
record('current-fourteen-family-continuation', 37)
for (const version of [22, 25, 31, 33, 34, 35, 36]) record(`legacy-enemy-world-${version}`, version, state => {
  for (const actor of state.world.enemies.actors) {
    if (version < 27 && actor.brain.family === 'demon') delete actor.brain.articulation
    if (version < 34) {
      delete actor.hitFeedback; delete actor.lethalMagicDamage; delete actor.shadowLateralOffset
      delete actor.config.lootPolicies; delete actor.config.classification
      delete actor.config.onDeathProgram; delete actor.config.recipeName; delete actor.config.recipeUid
    }
  }
  if (version < 34) {
    for (const field of ['demonSkullEncounter', 'bossNarration', 'facultyVoiceController',
      'bossSpells', 'puppetHits', 'detachedCrows', 'featuredBossId']) delete state.world.enemies[field]
  }
  delete state.world.enemyWorldFeedback; delete state.world.hallOfFameRuns
  delete state.world.lanternPosition; delete state.world.enemies.locomotionRngState
})
record('missing-current-demon-articulation', 37, state => {
  delete state.world.enemies.actors.find(actor => actor.brain.family === 'demon').brain.articulation
})
record('invalid-current-enemy-recoil', 37, state => {
  state.world.enemies.actors.find(actor => actor.brain.family === 'skeleton').brain.verticalOffset = 1
})
record('current-optional-enemy-event-targets', 37, state => {
  const work = createEnemyWork(state.world.enemies, context, true)
  const actorId = work.actors[0].id
  emitEvent(work, state.tick, 'attack-marker', actorId)
  emitEvent(work, state.tick, 'attack-marker', actorId, { targetPlayerId: null })
  emitEvent(work, state.tick, 'attack-marker', actorId, { targetPlayerId: playerId })
  state.world.enemyEvents = work.events
  state.world.enemies = finishEnemyStore(work, state.tick)
})
writeFixture('save-world-migration-goldens.json', 'tools/website-oracle/save-world-migration-goldens.mjs',
  'Website/frontend/src/game/save/game-save-document.ts public restore + actual Game future ticks', { cases })
