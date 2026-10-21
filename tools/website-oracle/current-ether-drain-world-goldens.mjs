#!/usr/bin/env node
// Actual accepted Game/loot constructors and the public ordered force transaction.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const placement = await loadKernel('core-server/boneyard-world-placement.ts')
const loot = await loadKernel('core-server/boneyard-loot-store.ts')
const painter = await loadKernel('core-kernels/native-world-manager-order.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], {
  cwd: websiteDir, encoding: 'utf8',
}).trim()
const loaded = {
  choice: { id: 'empty', name: 'Empty', source: 'default' },
  sourceSha256: 'c'.repeat(64), geometrySha256: 'b'.repeat(64),
  runId: 'drain-forces', seed: 'charms',
  scene: {
    bounds: { x: 0, y: 0, w: 500, h: 500 }, environmentMode: 2,
    fences: [], name: 'Forces', objects: [], roads: [], solomonDig: null,
    spawn: { facingDeg: 180, x: 250, y: 250 }, sprites: [], terrain: [],
  },
}
const configurations = {
  owner: { discipline: 'arcane', displayName: 'Owner', element: 'ether' },
  peer: { discipline: 'arcane', displayName: 'Peer', element: 'fire' },
}

function setup() {
  const session = game.enterBoneyardWorld(game.createGameSimulation(configurations, {
    gameRngSeed: 17, combatRngSeed: 91,
  }), loaded)
  const players = Object.fromEntries(Object.entries(
    entities.playerCharacterRecords(session.playerEntities),
  ).map(([id, actor], index) => [id, {
    ...actor, position: { x: index === 0 ? 90 : 140, y: 250 },
  }]))
  const order = painter.createNativeWorldManagerOrder(session.worldManagerOrder)
  const specs = [
    { activationDelayTicks: 0, amount: 100, id: 1, kind: 'gold', nativeTypeId: 2012,
      phase: 0, position: { x: 120, y: 300 }, source: 'script' },
    { activationDelayTicks: 0, amount: 10, id: 2, kind: 'gold', nativeTypeId: 2012,
      phase: 0, position: { x: 200, y: 300 }, source: 'script' },
  ]
  const born = loot.spawnBoneyardLootSpecs(session.world.loot, specs, 0, order.register)
  return { world: { ...session.world, loot: born.store }, players,
    secondary: session.secondaryAbilities, order, specs }
}

function reference(source, index, wrong = false) {
  const actor = source.world.loot.actors[index]
  return { kind: 'loot', id: actor.id,
    registrationOrdinal: actor.painterRegistration.registrationOrdinal + (wrong ? 99 : 0) }
}
function contact(target, x, y, consume = false) {
  return { target, delta: { x, y }, consume, amount: 0, damageFlags: 0x10a,
    hitStrength: 0, magicContact: false, ownerId: 'owner', sourceActorId: 99 }
}

const scenarios = [
  { name: 'ordered-player-pairs', build: () => [
    contact({ kind: 'player', id: 'owner' }, 80, 0),
    contact({ kind: 'player', id: 'peer' }, -30, 0),
  ] },
  { name: 'rounded-loose-sweep', build: source => [
    contact(reference(source, 0), .0000101, .0000202),
    contact(reference(source, 1), 900, 0),
  ] },
  { name: 'consume-matching-registration', build: source => [
    contact(reference(source, 0), 0, 0, true),
    contact(reference(source, 0), 0, 0, true),
    contact(reference(source, 1), -20, 0),
  ] },
  { name: 'reused-id-rejects-old-registration', build: source => [
    contact(reference(source, 0, true), 100, 0, true),
    contact(reference(source, 1, true), -40, 0),
  ] },
  { name: 'player-without-collision-body', inactiveOwner: true, build: () => [
    contact({ kind: 'player', id: 'owner' }, 80, 0),
    contact({ kind: 'player', id: 'peer' }, -30, 0),
  ] },
  { name: 'empty-contacts', build: () => [] },
]
const cases = scenarios.map(scenario => {
  const source = setup()
  const contacts = scenario.build(source)
  const combat = Object.fromEntries(Object.keys(source.players).map(id => [id, {
    alive: true, eligible: true, movementScale: 1,
    collisionEnabled: id !== 'owner' || !scenario.inactiveOwner,
  }]))
  const moved = placement.applyBoneyardEtherDrainForces(
    source.world, source.players, source.secondary, contacts, combat,
  )
  return {
    name: scenario.name, inactiveOwner: scenario.inactiveOwner ?? false,
    specs: source.specs, contacts,
    expected: {
      consumed: moved.consumedFieldActorIds,
      players: Object.entries(moved.players).map(([id, player]) => ({ id, position: player.position })),
      loot: moved.world.loot.actors.map(actor => ({ id: actor.id, position: actor.position,
        registration: actor.painterRegistration.registrationOrdinal })),
      rng: moved.world.loot.sharedRng, order: source.order.state(),
    },
  }
})
writeFixture('current-ether-drain-world-goldens.json',
  'tools/website-oracle/current-ether-drain-world-goldens.mjs',
  'core-server/boneyard-world-placement.ts applyBoneyardEtherDrainForces over actual Game/loot constructors',
  { sourceRevision, loaded, cases })
