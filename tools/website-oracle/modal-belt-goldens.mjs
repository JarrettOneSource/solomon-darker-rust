// Current public Inventory belt commands: immediate constructors never step the world.
import { loadKernel, writeFixture } from './oracle.mjs'
const game = await loadKernel('core-server/game-simulation.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const secondary = await loadKernel('core-kernels/native-secondary-abilities.ts')
const profile = await loadKernel('core-kernels/player-progression.ts')
const enemies = await loadKernel('core-server/boneyard-enemy-store.ts')
const { createNativeWorldManagerOrder } = await loadKernel('core-kernels/native-world-manager-order.ts')
const { BONEYARD_WAVE_ENEMY_TYPES } = await loadKernel('core-kernels/boneyard-wave-schema.ts')
const loaded = { choice: { id: 'mod:combat-fixture', modId: 'combat-fixture', modName: 'Combat Fixture', name: 'Combat Fixture', source: 'mod' },
  geometrySha256: 'd'.repeat(64), sourceSha256: 'c'.repeat(64), runId: 'modal-belt', seed: 'modal-belt-seed',
  scene: { bounds: { x: 0, y: 0, w: 500, h: 500 }, environmentMode: 2, fences: [], name: 'Combat fixture', objects: [], roads: [],
    solomonDig: null, spawn: { facingDeg: 180, x: 250, y: 250 }, sprites: [], terrain: [] } }
const primary = Array.from({ length: 83 }, (_, id) => id).find(id => profile.nativeSkillCategory(id) === 1 && id !== 17)
const concentration = Array.from({ length: 83 }, (_, id) => id).find(id => profile.nativeSkillCategory(id) === 3)
const specs = [
  ...[12, 21, 23, 35, 48, 49, 51, 77, 78, 79].map(skillId => ({ skillId })),
  { skillId: 49, aim: { x: 350, y: 250 }, repeat: true },
  { skillId: 48, aim: { x: 350, y: 250 } },
  { skillId: 35, cooldown: 50 }, { skillId: 35, mana: 0 },
  { skillId: primary, hub: true }, { skillId: concentration, hub: true },
  { skillId: 35, hub: true }, { skillId: 35, unlearned: true },
  { skillId: 35, slot: 24 }, { skillId: 35, slot: 23 },
]
function snapshot(state) {
  const progression = game.getPlayerProgression(state, 'caster'), character = game.getPlayerCharacter(state, 'caster')
  const { actors, events, firewalkerGeometrySequence, nextActorId, nextEventId, players, rng, targetEffects } = state.secondaryAbilities
  return { tick: state.tick, position: character.position, headingIndex: character.headingIndex,
    mana: progression.currentMana, primarySkillId: game.getPlayerSkillBook(state, 'caster').primarySkillId,
    concentrations: [state.playerEntities.skillRuntimes[0].concentrationSkillIdA, state.playerEntities.skillRuntimes[0].concentrationSkillIdB],
    secondary: { actors, events, firewalkerGeometrySequence, nextActorId, nextEventId, players, rng, targetEffects },
    flashes: state.screenFlashes, painters: state.worldManagerOrder,
    enemies: state.world.kind !== 'boneyard' ? [] : state.world.enemies.actors.map(actor => ({ id: actor.id,
      position: actor.position, heading: actor.headingDeg, wanderHeading: actor.path.wanderHeadingDeg, dampened: actor.dampened })),
    enemyEvents: state.world.kind !== 'boneyard' ? [] : state.world.enemyEvents.map(event => ({ actorId: event.actorId, eventId: event.eventId, type: event.type, tick: event.tick })),
  }
}
const cases = []
for (const spec of specs) {
  let state = game.createGameSimulation({ caster: { discipline: 'body', displayName: 'Modal caster', element: 'fire' } }, { gameRngSeed: 17 })
  if (!spec.hub) state = game.enterBoneyardWorld(state, loaded)
  const sourceBook = game.getPlayerSkillBook(state, 'caster'), permanentRanks = [...sourceBook.permanentRanks], effectiveRanks = [...sourceBook.effectiveRanks]
  if (!spec.unlearned) { permanentRanks[spec.skillId] = 1; effectiveRanks[spec.skillId] = 1 }
  const skillBook = { ...sourceBook, permanentRanks, effectiveRanks,
    learnedSkillOrder: spec.unlearned || sourceBook.learnedSkillOrder.includes(spec.skillId) ? sourceBook.learnedSkillOrder : [...sourceBook.learnedSkillOrder, spec.skillId] }
  state = { ...state, playerEntities: entities.replacePlayerEconomy({ ...state.playerEntities, skillBooks: [skillBook] }, 'caster', game.getPlayerEconomy(state, 'caster')) }
  // A stale or out-of-range entry is an actual trust-boundary input to the public operation.
  const belt = Array.from({ length: 24 }, () => null); belt[Math.min(spec.slot ?? 0, 23)] = { kind: 'skill', skillId: spec.skillId }
  state = { ...state, playerEntities: { ...state.playerEntities, belts: [belt], progressions: state.playerEntities.progressions.map(player => ({ ...player,
    currentMana: spec.mana ?? 10000, maximumMana: 10000, currentHealth: 1000, maximumHealth: 1000 })) },
    secondaryAbilities: { ...state.secondaryAbilities, players: { caster: { ...secondary.createNativeSecondaryPlayerState(), globalCooldownTicks: spec.cooldown ?? 0 } } } }
  if (spec.skillId === 77 || spec.skillId === 51) {
    const order = createNativeWorldManagerOrder(state.worldManagerOrder), tokens = ['SKELETON', 'SKELETONARCHER', 'SKELETONMAGE', 'ZOMBIE']
    const store = enemies.stepBoneyardEnemyStore(state.world.enemies, { players: {}, tick: state.tick,
      projectileWorldBlocked: () => false, resolveMovement: request => request.position, registerWorldPainter: order.register,
      resolveSpawnIntents: () => tokens.map((enemyToken, index) => ({ enemyToken, flags: [], id: index + 1, locationPolicy: 'anywhere',
        nativeTypeId: BONEYARD_WAVE_ENEMY_TYPES[enemyToken], position: { x: 300, y: 160 + index * 60 }, spawnTick: state.tick, waveOrdinal: 1 })) }).store
    state = { ...state, worldManagerOrder: order.state(), world: { ...state.world, enemies: { ...store,
      actors: store.actors.map(actor => ({ ...actor, headingDeg: 180, nextMovementTick: Number.MAX_SAFE_INTEGER })) } } }
  }
  const initial = snapshot(state), inputs = { book: game.getPlayerSkillBook(state, 'caster'), secondary: initial.secondary }, rows = []
  for (let i = 0; i < (spec.repeat ? 2 : 1); i++) {
    const result = game.applyGameSimulationHubAction(state, 'caster', { type: 'activate-belt-slot', slot: spec.slot ?? 0, aim: spec.aim ?? null })
    state = result.state
    rows.push({ accepted: result.accepted, reason: result.reason, expected: snapshot(state) })
  }
  cases.push({ spec, inputs, initial, rows })
}
writeFixture('modal-belt-goldens.json', 'modal-belt-goldens.mjs', 'current public applyGameSimulationHubAction::activate-belt-slot, exact immediate skill selection/cast/contact/flash boundary', { sourceRevision: 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83', cases })
