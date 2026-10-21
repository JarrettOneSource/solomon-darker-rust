// Live Weld and secondary actors, admitted by the unmodified public Game tick. Save/restore
// then advances that same accepted owner through later real ticks, preserving all RNGs.
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, writeFixture } from './oracle.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const saves = await loadKernel('save/game-save-document.ts')
const skills = await loadKernel('core-kernels/player-skill-runtime.ts')
const { nativeWeldBuild, nativeSkillCategory } = await loadKernel('core-kernels/player-progression.ts')
const { boneyardGeometrySha256 } = await loadKernel('host/project-boneyard.ts')
const { createEnemyWork, finishEnemyStore } = await loadKernel('core-server/enemies/work.ts')
const { materializeSpawnIntents } = await loadKernel('core-server/enemies/construction.ts')
const { createNativeWorldManagerOrder } = await loadKernel('core-kernels/native-world-manager-order.ts')
const { damagePlayerEntity } = await loadKernel('core-server/player-entity-store.ts')
const playerId = 'spell-continuation-owner'
const scene = { name: 'Spell Continuation Arena', environmentMode: 0,
  bounds: { x: 0, y: 0, w: 2000, h: 2000 }, spawn: { x: 1000, y: 1000, facingDeg: 0 },
  objects: [], sprites: [], roads: [], fences: [], terrain: [], solomonDig: null }
const loadedBoneyard = { scene, choice: { id: 'spell-save-arena', name: scene.name, source: 'default' },
  sourceSha256: createHash('sha256').update(JSON.stringify(scene)).digest('hex'),
  geometrySha256: boneyardGeometrySha256(scene), seed: '00000000000000000000000000000000', runId: 'spell-save-arena' }
function fresh(rankIds, weldId = null, element = 'fire') {
  let state = game.enterBoneyardWorld(game.createGameSimulation({ [playerId]: {
    displayName: 'Spell Continuation', element, discipline: 'arcane',
  } }, { gameRngSeed: 17, combatRngSeed: 91 }), loadedBoneyard)
  const store = state.playerEntities
  const book = structuredClone(store.skillBooks[0])
  for (const id of rankIds) {
    book.permanentRanks[id] = 1
    if (!book.learnedSkillOrder.includes(id)) book.learnedSkillOrder.push(id)
  }
  if (weldId !== null) { book.weldBuildId = weldId; book.weldComponentRanks = null; book.primarySkillId = 52 }
  const refreshed = skills.refreshPlayerSkillRuntime(store.skillRuntimes[0], book, store.statBooks[0], store.economies[0])
  state = { ...state, playerEntities: { ...store, skillBooks: [refreshed.skillBook], skillRuntimes: [refreshed.runtime],
    progressions: [{ ...store.progressions[0], currentMana: 100000, maximumMana: 100000 }] } }
  return state
}
const cases = []
function save(name, state, version = 37, mutate = () => {}) {
  const document = JSON.parse(saves.createGameSaveDocument({ state, playerId, loadedBoneyard,
    integrity: 'global-clean', mods: [], modState: {} }))
  document.schemaVersion = version
  mutate(document.continuation.simulation)
  try {
    const restored = saves.restoreGameSaveDocument(JSON.stringify(document))
    const frames = [{ ticks: 0, simulation: restored.state }]
    let future = restored.state
    for (let tick = 1; tick <= 8; tick++) {
      future = game.stepGameSimulationTick(future, {})
      if (tick === 1 || tick === 8) frames.push({ ticks: tick, simulation: future })
    }
    cases.push({ name, document, accepted: true, frames })
  } catch (error) { cases.push({ name, document, accepted: false, error: error.message }) }
}
const input = (primary, quickbar = null) => ({ [playerId]: { movement: { x: 0, y: 0 },
  aim: { x: 1000, y: 1500 }, cast: { primary, quickbar }, viewportWidth: 1600, viewportHeight: 900 } })
for (let id = 1000; id <= 1009; id++) {
  const build = nativeWeldBuild(id)
  let state = fresh([52, ...build.componentSkillIds], id)
  for (let tick = 1; tick <= 65; tick++) {
    state = game.stepGameSimulationTick(state, input(true))
    if (tick === 25 || tick === 65) save(`weld-${id}-held-${tick}`, state)
  }
  state = game.stepGameSimulationTick(state, input(false))
  save(`weld-${id}-released`, state)
  if (id === 1007) save('legacy-meteor-root-33', state, 33, simulation => {
    for (const actor of simulation.primarySpells.transients) if (actor.kind === 'weld-meteor') {
      actor.position = actor.landingPosition; delete actor.landingPosition
    }
  })
}
for (let id = 8; id <= 79; id++) {
  if (nativeSkillCategory(id) !== 2) continue
  let state = fresh([id])
  state.playerEntities = { ...state.playerEntities, belts: [[{ kind: 'skill', skillId: id },
    null, null, { kind: 'health-potion' }, { kind: 'mana-potion' }, null, null, null]] }
  state = game.stepGameSimulationTick(state, input(false, 0))
  save(`secondary-${id}`, state)
}

let air = fresh([29], null, 'air')
for (let tick = 0; tick < 70; tick++) air = game.stepGameSimulationTick(air, input(true))
if (!air.primarySpells.transients.some(actor => actor.kind === 'air-hurricane')) {
  throw new Error('held Air must create the actual Hurricane actor')
}
save('air-hurricane-held', air)

let water = fresh([33, 34, 36, 37, 38, 39], null, 'water')
for (let tick = 0; tick < 201; tick++) water = game.stepGameSimulationTick(water, input(true))
save('water-harden-and-modifiers-held', water)
water = game.stepGameSimulationTick(water, input(false))
if (!water.primarySpells.transients.some(actor => actor.kind === 'harden-burst')) {
  throw new Error('releasing formed Harden must create actual breakup actors')
}
save('water-harden-breakup', water)

let staff = fresh([])
const order = createNativeWorldManagerOrder(staff.worldManagerOrder)
const spawnContext = { tick: staff.tick, registerWorldPainter: order.register }
const work = createEnemyWork(staff.world.enemies, spawnContext, true)
const births = JSON.parse(readFileSync(resolve(fixturesDir, 'enemy-construction-goldens.json')))
const skeleton = births.cases[0].intents.find(intent => intent.enemyToken === 'SKELETON')
work.actors = materializeSpawnIntents(work, { ...spawnContext,
  players: { [playerId]: { alive: true, connected: true, eligible: true, collisionRadius: 25,
    position: { x: 1000, y: 1000 }, velocityPerTick: { x: 0, y: 0 } } },
  resolveMovement: request => request.requestedPosition,
}, [{ ...skeleton, position: { x: 1000, y: 965 }, spawnTick: staff.tick }])
staff = { ...staff, worldManagerOrder: order.state(),
  world: { ...staff.world, enemies: finishEnemyStore(work, staff.tick) } }
let admittedAction = false
let admittedContact = false
for (let tick = 0; tick < 100; tick++) {
  staff = game.stepGameSimulationTick(staff, { [playerId]: {
    movement: { x: 0, y: -1 }, aim: null, cast: { primary: false, quickbar: null },
    viewportWidth: 1600, viewportHeight: 900,
  } })
  if (!admittedAction && staff.primarySpells.transients.some(actor => actor.kind === 'player-staff-melee')) {
    save('automatic-staff-melee', staff)
    admittedAction = true
  }
  if (staff.primarySpells.transients.some(actor => actor.kind === 'player-staff-contact')) {
    save('automatic-staff-contact', staff)
    admittedContact = true
    break
  }
}
if (!admittedAction || !admittedContact) throw new Error('ordinary movement must admit Staff action and contact')

let dying = fresh([])
dying = game.addPlayerCharacter(dying, 'living-companion', {
  displayName: 'Companion', element: 'water', discipline: 'mind',
})
dying = { ...dying, playerEntities: damagePlayerEntity(dying.playerEntities, playerId, 10000, dying.tick) }
for (let tick = 0; tick < 496; tick++) dying = game.stepGameSimulationTick(dying, {})
if (dying.run.phase !== 'active' || dying.playerEntities.progressions[0].lifeState !== 'dying') {
  throw new Error('death continuation must begin before the actual GameOver transition')
}
save('dying-before-game-over', dying)
save('legacy-dying-weapon-registration-22', dying, 22)
writeFixture('save-spell-migration-goldens.json', 'tools/website-oracle/save-spell-migration-goldens.mjs',
  'Website/frontend/src/game/save/game-save-document.ts public live Weld/secondary restore + future Game ticks', { cases })
