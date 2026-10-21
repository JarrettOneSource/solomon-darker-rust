// Interleaved real participants and Golem packets must retain one global source epoch.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture, fixturesDir } from './oracle.mjs'
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
import { setup, snapshot } from './player-entity-fixture.mjs'
const contacts = await loadKernel('core-server/player-contact-system.ts')
const construction = await loadKernel('core-server/boneyard-world-construction.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const secondary = await loadKernel('core-kernels/native-secondary-abilities.ts')
const painter = await loadKernel('core-kernels/native-world-manager-order.ts')
const stateFixture = JSON.parse(readFileSync(resolve(fixturesDir, 'secondary-state-goldens.json'), 'utf8'))
const golemInitial = stateFixture.journeys.find(row => row.name.includes('golem')).initial
const loaded = {
  choice: { id: 'contact-oracle', source: 'custom', name: 'Contact Oracle' },
  name: 'Contact Oracle', runId: 'oracle', seed: '00112233445566778899aabbccddeeff',
  sourceSha256: '', geometrySha256: '',
  scene: { name: 'Contact Oracle', environmentMode: 0,
    bounds: { x: 0, y: 0, w: 1200, h: 1200 }, spawn: { x: 321.25, y: 245.5, facingDeg: 45 },
    objects: [], sprites: [], roads: [], fences: [], terrain: [], solomonDig: null },
}
const packet = patch => ({ actorId: 7, eventId: 19, playerId: 'alice',
  physicalDamage: 0, magicDamage: 0, coldSlowTicks: 0, dazzleTicks: 0,
  poisonDamage: 0, poisonDuration: 0, ...patch })
const cases = []
function record(name, players, patches, seed = 917) {
  let store = entities.createPlayerEntityStore()
  const characters = {}
  for (const [index, input] of players.entries()) {
    const source = setup(input.spec)
    const character = { ...entities.playerCharacterAt(source, 'local'),
      position: input.position, headingIndex: input.heading }
    store = entities.importPlayerEntity(store, source, 'local', input.id,
      { managerLane: 'actor', registrationOrdinal: index + 1 }, character)
    characters[input.id] = character
  }
  let world = construction.createBoneyardWorld(loaded)
  world.enemies.nextEventId = 83
  world.enemies.nextDeathEffectId = 71
  const abilities = { ...secondary.createNativeSecondarySimulation(seed),
    actors: structuredClone(golemInitial.actors).map(actor => ({ ...actor, ageTicks: 400, ownerId: 'alice' })), nextActorId: 2,
    players: Object.fromEntries(players.map(input => [input.id,
      { ...secondary.createNativeSecondaryPlayerState(), ...input.secondary }])) }
  const order = painter.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 300, transient: 500 } })
  const damage = patches.map((patch, index) => packet({ eventId: 19 + index, ...patch }))
  const result = contacts.applyPlayerContacts({ world, playerEntities: store, secondaryAbilities: abilities }, characters, damage, 201, undefined, order.register)
  const e = result.world.enemies
  cases.push({ name, players, damage, initialSecondary: abilities, expected: {
    players: Object.fromEntries(result.playerEntities.identities.map(({ playerId }, index) => [playerId, {
      player: snapshot(result.playerEntities, index), heading: result.resolvedPlayers[playerId].headingIndex,
      blindness: result.playerEntities.lightings[index].blindnessTicksRemaining,
    }])),
    world: { ouchDeadline: result.world.playerOuchDeadlineTick,
      counters: [e.nextActorId, e.nextEventId, e.nextDeathEffectId], enemyRng: e.rngState, steeringRng: e.steeringRngState,
      webbed: e.webbedPlayers,
      enemies: e.actors.map(actor => ({ id: actor.id, position: actor.position, registration: actor.lightRegistration })),
      particles: e.deathEffects.map(effect => ({ id: effect.id, entry: effect.entry, position: effect.position,
        velocity: effect.velocity, height: effect.height, verticalVelocity: effect.verticalVelocity, tint: effect.tint })),
    },
    secondary: result.secondaryAbilities, painterOrder: order.state(),
    outbox: { chips: result.hardenChips, damage: result.appliedPlayerDamage,
      reflections: result.reflectedEnemyDamage, deflections: [...result.deflectPitchesByEventId], sounds: result.playerDamageSoundEvents },
  } })
}
const alice = { id: 'alice', position: { x: 321.25, y: 245.5 }, heading: 3,
  spec: { combat: { currentHealth: 80, currentMana: 10 }, ranks: [[68, 5]], runtime: { concentrationSkillIdA: 68 } } }
const bob = { id: 'bob', position: { x: 440.5, y: 310.25 }, heading: 15,
  spec: { combat: { currentHealth: 35, currentMana: 50 }, ranks: [[53, 3]], runtime: { harden: { armor: 2, coating: 1 } } } }
const golemContact = { playerId: 'golem:1', physicalDamage: 10, magicDamage: 3,
  source: { position: { x: 140, y: 100 }, collisionRadius: 15, reflectableActorId: 7 } }
const ordered = [
  { playerId: 'bob', physicalDamage: 4, crowBlindChancePercent: 100 },
  golemContact,
  { playerId: 'alice', physicalDamage: 4, crowBlindChancePercent: 100,
    source: { position: { x: 360, y: 245 }, collisionRadius: 15, reflectableActorId: 7 } },
  { playerId: 'bob', coldSlowTicks: 30, poisonDamage: 3, poisonDuration: 4 },
  { playerId: 'alice', poisonDamage: 4, poisonDuration: 3 },
  { playerId: 'alice', coldSlowTicks: 10 },
  { playerId: 'bob', coldSlowTicks: 30, poisonDamage: 3, poisonDuration: 4 },
  { playerId: 'missing', physicalDamage: 10 },
  { playerId: 'golem:missing', physicalDamage: 10 },
]
for (let seed = 0; seed < 16; seed++) {
  record(`interleaved-${seed}`, [alice, bob], ordered, seed)
  record(`reversed-participants-${seed}`, [bob, alice], ordered, seed)
}
const ordinary = [alice, bob].map(input => ({ ...input, spec: { combat: { currentHealth: 80 } } }))
record('independent-web-births', ordinary, [
  { playerId: 'alice', webbedStrength: 20 }, { playerId: 'bob', webbedStrength: 30 },
  { playerId: 'alice', webbedStrength: 40 }, { playerId: 'bob', webbedStrength: 10 },
  golemContact, { playerId: 'alice', webbedStrength: 10 }, { playerId: 'bob', webbedStrength: 20 },
])
record('independent-status-admission', ordinary, [
  { playerId: 'bob', coldSlowTicks: 10 }, { playerId: 'alice', coldSlowTicks: 10 },
  { playerId: 'bob', coldSlowTicks: 10 }, { playerId: 'alice', poisonDamage: 2, poisonDuration: 3 },
  { playerId: 'bob', poisonDamage: 2, poisonDuration: 3 }, { playerId: 'alice', poisonDamage: 2, poisonDuration: 3 },
])
record('golem-once-with-two-players', ordinary, [golemContact, { ...golemContact, tragicCircle: true }])
record('golem-once-with-no-players', [], [golemContact, { ...golemContact, tragicCircle: true }])
writeFixture('player-contact-multiplayer-goldens.json', 'tools/website-oracle/player-contact-multiplayer-goldens.mjs',
  'Current Website core-server/player-contact-system.ts global ordered multi-player contacts', { sourceRevision, cases })
