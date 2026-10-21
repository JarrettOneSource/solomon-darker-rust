#!/usr/bin/env node
// Genuine shared Game ticks: console skill grant, belt input, actual loot/enemy births.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const character = await loadKernel('core-kernels/player-character.ts')
const loot = await loadKernel('core-server/boneyard-loot-store.ts')
const painter = await loadKernel('core-kernels/native-world-manager-order.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], {
  cwd: websiteDir, encoding: 'utf8',
}).trim()
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
const loaded = {
  choice: { id: 'empty', name: 'Empty', source: 'default' },
  sourceSha256: 'c'.repeat(64), geometrySha256: 'b'.repeat(64),
  runId: 'drain-session', seed: 'charms',
  scene: {
    bounds: { x: 0, y: 0, w: 500, h: 500 }, environmentMode: 2,
    fences: [], name: 'Drain Session', objects: [], roads: [], solomonDig: null,
    spawn: { facingDeg: 180, x: 250, y: 250 }, sprites: [], terrain: [],
  },
}
const configurations = {
  owner: { discipline: 'arcane', displayName: 'Owner', element: 'ether' },
  peer: { discipline: 'arcane', displayName: 'Peer', element: 'fire' },
}
const lootSpecs = [
  { activationDelayTicks: 0, amount: 100, id: 1, kind: 'gold', nativeTypeId: 2012,
    phase: 0, position: { x: 250, y: 250 }, source: 'script' },
  { activationDelayTicks: 0, amount: 10, id: 2, kind: 'gold', nativeTypeId: 2012,
    phase: 0, position: { x: 315, y: 280 }, source: 'script' },
]
const spawnIntents = ['SKELETON', 'ZOMBIE'].map((enemyToken, index) => ({
  enemyToken, nativeTypeId: index === 0 ? 1001 : 1006,
  flags: index === 0 ? ['FLAG_HPDOWN', 'FLAG_SLOW'] : [], id: index + 1,
  locationPolicy: 'anywhere', position: { x: 250 + index * 45, y: 260 },
  spawnTick: 1, waveOrdinal: 1,
}))
const actorFields = ['ageTicks', 'alpha', 'damage', 'enhanced', 'endpoint', 'frame',
  'freezeTicks', 'golem', 'hitTargetIds', 'id', 'kind', 'lifetimeTicks',
  'lightRegistration', 'midpoint', 'miscLightAppendOrdinal', 'ownerId',
  'painterRegistrations', 'phase', 'position', 'presentationRng', 'quantity',
  'radius', 'rank', 'rotationRadians', 'scale', 'skillId', 'slowFactor',
  'targetId', 'variant', 'velocity', 'worldKey', 'etherDrain']
function project(state) {
  const world = state.world
  const players = entities.playerCharacterRecords(state.playerEntities)
  return {
    tick: state.tick,
    players: state.playerEntities.identities.map(({ playerId }, index) => {
      const progression = state.playerEntities.progressions[index]
      return { id: playerId, position: players[playerId].position,
        health: progression.currentHealth, mana: progression.currentMana,
        life: progression.lifeState, deathTick: progression.deathTick,
        corpseConsumed: progression.corpseConsumed }
    }),
    actors: state.secondaryAbilities.actors.map(actor => Object.fromEntries(
      actorFields.filter(key => actor[key] !== undefined).map(key => [key, actor[key]]),
    )),
    events: state.secondaryAbilities.events,
    nextActorId: state.secondaryAbilities.nextActorId,
    nextEventId: state.secondaryAbilities.nextEventId,
    rng: state.secondaryAbilities.rng,
    flashes: state.screenFlashes,
    order: state.worldManagerOrder,
    enemies: world.enemies.actors.map(actor => ({ id: actor.id,
      token: actor.config.enemyToken, position: actor.position, health: actor.currentHealth,
      life: actor.lifeState, captured: actor.etherDrainCaptured ?? false })),
    enemyEvents: world.enemyEvents.map(event => ({ actorId: event.actorId,
      eventId: event.eventId, tick: event.tick, type: event.type })),
    loot: world.loot.actors.map(actor => ({ id: actor.id, kind: actor.kind,
      position: actor.position, activationDelayTicks: actor.activationDelayTicks,
      registration: actor.painterRegistration.registrationOrdinal })),
    lootRng: world.loot.sharedRng,
  }
}
const cases = []
for (const enhancedEffects of [false, true]) {
  let state = game.createGameSimulation(configurations, { gameRngSeed: 17, combatRngSeed: 91 })
  const granted = entities.grantPlayerEntitySkillRanks(state.playerEntities, 'owner', 74, 1, state.gameRng)
  state = { ...state, playerEntities: granted.store, gameRng: granted.rng }
  state = game.bindGameSimulationPlayerSkillQuickbar(state, 'owner', 74, 0)
  if (!state) throw new Error('actual Source rejected the granted skill belt binding')
  state = game.enterBoneyardWorld(state, loaded)
  state = { ...state, enhancedEffects, playerEntities: entities.replacePlayerCharacterRecords(
    state.playerEntities, Object.fromEntries(Object.entries(
      entities.playerCharacterRecords(state.playerEntities),
    ).map(([id, player], index) => [id, { ...player, position: { x: index === 0 ? 90 : 140, y: 250 } }])),
  ) }
  const order = painter.createNativeWorldManagerOrder(state.worldManagerOrder)
  const born = loot.spawnBoneyardLootSpecs(state.world.loot, lootSpecs, 0, order.register)
  state = { ...state, worldManagerOrder: order.state(), world: { ...state.world, loot: born.store } }
  const frames = [{ tick: 0, expected: bits(project(state)) }]
  for (let tick = 1; tick <= 220; tick++) {
    const input = { ...character.createIdlePlayerCharacterInput(), aim: { x: 250, y: 250 },
      cast: { primary: false, quickbar: tick === 1 ? 0 : null } }
    state = game.stepGameSimulationTick(state, { owner: input }, {
      enemySpawnIntents: tick === 1 ? spawnIntents : [],
    })
    if (tick <= 15 || tick % 5 === 0) frames.push({ tick, expected: bits(project(state)) })
  }
  cases.push({ enhancedEffects, frames })
}
writeFixture('current-ether-drain-session-goldens.json',
  'tools/website-oracle/current-ether-drain-session-goldens.mjs',
  'core-server/game-simulation.ts actual EtherDrain secondary/force/damage/capture phases',
  { sourceRevision, loaded, lootSpecs, spawnIntents, cases })
console.log(`recorded ${cases.length} shared Session journeys / 440 ticks`)
