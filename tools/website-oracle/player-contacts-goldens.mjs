import { execFileSync } from 'node:child_process'
import assert from 'node:assert/strict'
// Complete native contact host with real entity, enemy, secondary and painter owners.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture, fixturesDir } from './oracle.mjs'
import { setup, snapshot } from './player-entity-fixture.mjs'
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const contacts = await loadKernel('core-server/player-contact-system.ts')
const construction = await loadKernel('core-server/boneyard-world-construction.ts')
const secondary = await loadKernel('core-kernels/native-secondary-abilities.ts')
const painter = await loadKernel('core-kernels/native-world-manager-order.ts')
const character = await loadKernel('core-kernels/player-character.ts')
const enemyWork = await loadKernel('core-server/enemies/work.ts')
const enemyConstruction = await loadKernel('core-server/enemies/construction.ts')
const enemySchema = await loadKernel('core-kernels/boneyard-wave-schema.ts')
const stateFixture = JSON.parse(readFileSync(resolve(fixturesDir, 'secondary-state-goldens.json'), 'utf8'))
const golemInitial = stateFixture.journeys.find(row => row.name.includes('golem')).initial
const position = { x: 321.25, y: 245.5 }
const loaded = {
  choice: { id: 'contact-oracle', source: 'custom', name: 'Contact Oracle' },
  name: 'Contact Oracle', runId: 'oracle', seed: '00112233445566778899aabbccddeeff',
  sourceSha256: '', geometrySha256: '',
  scene: { name: 'Contact Oracle', environmentMode: 0,
    bounds: { x: 0, y: 0, w: 1200, h: 1200 }, spawn: { ...position, facingDeg: 45 },
    objects: [], sprites: [], roads: [], fences: [], terrain: [], solomonDig: null },
}
const packet = patch => ({ actorId: 7, eventId: 19, playerId: 'local',
  physicalDamage: 0, magicDamage: 0, coldSlowTicks: 0, dazzleTicks: 0,
  poisonDamage: 0, poisonDuration: 0, ...patch })
const cases = []
// Keep the existing contact-store view explicit. Current warp geometry and camera
// proposals have their own complete motion fixtures; this fixture retains every
// actor, event, owner clock, status and random word it has always compared.
function secondaryContactSnapshot(state) {
  const { actors, events, firewalkerGeometrySequence, nextActorId, nextEventId,
    players, rng, targetEffects } = state
  return { actors, events, firewalkerGeometrySequence, nextActorId, nextEventId,
    players, rng, targetEffects }
}
function record(name, spec, patches, options = {}) {
  const tick = options.tick ?? 201
  const playerEntities = setup(spec)
  let world = construction.createBoneyardWorld(loaded)
  if (options.sourceActor) {
    // Construct a real parent actor so omitted and explicitly null sources have
    // different reflection behavior through the public contact host.
    world.enemies.nextActorId = 7
    const context = { tick: 1, players: {}, resolveMovement: request => request.requestedPosition }
    const work = enemyWork.createEnemyWork(world.enemies, context, true)
    work.actors.push(...enemyConstruction.materializeSpawnIntents(work, context, [{
      enemyToken: 'SPIDER', flags: [], id: 7, locationPolicy: 'anywhere', nativeTypeId: enemySchema.BONEYARD_WAVE_ENEMY_TYPES.SPIDER,
      position: { x: 361.25, y: 245.5 }, spawnTick: 1, waveOrdinal: 31,
    }]))
    world.enemies = enemyWork.finishEnemyStore(work, world.enemies.lastStepTick)
  }
  world.enemies.nextEventId = 83
  world.enemies.nextDeathEffectId = 71
  world.playerOuchDeadlineTick = options.deadline ?? 0
  if (options.protected) world.tutorial = { damageProtection: true }
  if (options.dialogue) world.enemies.bossNarration = { ...world.enemies.bossNarration, pending: [{}] }
  let abilities = options.golem ? structuredClone(golemInitial) : secondary.createNativeSecondarySimulation(options.seed ?? 917)
  if (options.golemAge !== undefined) abilities.actors = abilities.actors.map(actor => ({ ...actor, ageTicks: options.golemAge }))
  abilities.players = { local: { ...secondary.createNativeSecondaryPlayerState(), ...options.secondaryPlayer } }
  const wizard = { ...character.createPlayerCharacter({ displayName: 'Oracle', element: 'ether', discipline: 'arcane' }, loaded.scene.spawn),
    position, headingIndex: 3 }
  const order = painter.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 300, transient: 500 } })
  const damage = patches.map((patch, index) => packet({ eventId: 19 + index, ...patch }))
  const initialEnemyEvents = world.enemies.events
  const result = contacts.applyPlayerContacts({ world, playerEntities, secondaryAbilities: abilities }, { local: wizard }, damage, tick, undefined, order.register)
  const e = result.world.enemies
  assert.deepEqual(e.events, initialEnemyEvents, `${name}: contact feedback preserves the incoming enemy event queue`)
  cases.push({ name, spec, damage, options: { ...options, tick }, initialSecondary: abilities,
    expected: { player: snapshot(result.playerEntities), heading: result.resolvedPlayers.local.headingIndex,
      blindness: result.playerEntities.lightings[0].blindnessTicksRemaining,
      ouchDeadline: result.world.playerOuchDeadlineTick,
      counters: [e.nextActorId, e.nextEventId, e.nextDeathEffectId], enemyRng: e.rngState, steeringRng: e.steeringRngState,
      webbed: e.webbedPlayers,
      enemies: e.actors.map(actor => ({ id: actor.id, position: actor.position, registration: actor.lightRegistration })),
      particles: e.deathEffects.map(effect => ({ id: effect.id, entry: effect.entry, position: effect.position,
        velocity: effect.velocity, height: effect.height, verticalVelocity: effect.verticalVelocity, tint: effect.tint })),
      secondary: secondaryContactSnapshot(result.secondaryAbilities), painterOrder: order.state(),
      outbox: { chips: result.hardenChips, damage: result.appliedPlayerDamage,
        reflections: result.reflectedEnemyDamage, deflections: [...result.deflectPitchesByEventId], sounds: result.playerDamageSoundEvents } } })
}
const base = { combat: { currentHealth: 80, currentMana: 10 } }
for (const patch of [
  { physicalDamage: 9 }, { magicDamage: 9 }, { poisonContactDamage: 9 },
  { physicalDamage: 9, magicDamage: 3, poisonContactDamage: 5 },
  { coldSlowTicks: 30, poisonDamage: 3, poisonDuration: 4, dazzleTicks: 7 },
  { physicalDamage: 9, suppressHitResponse: true, suppressFlash: true },
  { physicalDamage: 90 }, { manaDamageMaximumFraction: 0.5 }, { tragicCircle: true },
]) record(`base-${cases.length}`, base, [patch])
record('status-once-per-batch', base, Array(3).fill({ coldSlowTicks: 30, poisonDamage: 3, poisonDuration: 4 }))
record('already-statused', { ...base, combat: { ...base.combat, coldSlowTicksRemaining: 5, poisonTicksRemaining: 4 } }, [{ coldSlowTicks: 30, poisonDamage: 3 }])
record('poison-immunity', { ...base, potionClocks: { poisonImmunityTicksRemaining: 1 } }, [{ poisonContactDamage: 9, poisonDamage: 3, poisonDuration: 4 }])
record('poison-resistance', { ...base, ranks: [[70, 3]], runtime: { concentrationSkillIdA: 70 } }, [{ poisonContactDamage: 9, poisonDamage: 3, poisonDuration: 4 }])
record('drinker-mana-drain', { ...base, perks: [15] }, [{ tragicCircle: true }, { manaDamageMaximumFraction: 0.5 }])
record('dead-circle', { combat: { currentHealth: -1, lifeState: 'dying' } }, [{ tragicCircle: true }])
for (const health of [10, 25, 35, 45, 80]) for (const options of [{}, { deadline: 201 }, { dialogue: true }]) {
  record(`ouch-${health}-${cases.length}`, { combat: { currentHealth: health } }, [{ physicalDamage: 2 }], options)
}
for (const options of [{ magicShieldAbsorb: 15, magicShieldMaximum: 15 }, { stoneskinTicksRemaining: 20 }]) {
  for (const patch of [{ physicalDamage: 9, coldSlowTicks: 5 }, { physicalDamage: 25, poisonContactDamage: 3 }, { manaDamageMaximumFraction: 0.5 }, { webbedStrength: 30 }, { crowBlindChancePercent: 100 }]) {
    record(`interception-${cases.length}`, base, [patch], { secondaryPlayer: options })
  }
}
record('web-cocoon', base, [{ webbedStrength: 30 }, { webbedStrength: 40 }, { webbedStrength: 20 }, { webbedStrength: 10 }])
record('tutorial-protection', base, [{ physicalDamage: 20, crowBlindChancePercent: 100 }], { protected: true })
record('unknown-player', base, [{ playerId: 'missing', physicalDamage: 20 }])
for (let seed = 0; seed < 32; seed++) {
  record(`deflect-${seed}`, { ...base, ranks: [[68, 5]], runtime: { concentrationSkillIdA: 68 } }, [{ physicalDamage: 5,
    source: { position: { x: 361.25, y: 245.5 }, collisionRadius: 15, reflectableActorId: 7 }, crowBlindChancePercent: 100 }], { seed })
  record(`flash-${seed}`, { ...base, ranks: [[53, 3]] }, [{ physicalDamage: 1 }], { seed })
  record(`harden-${seed}`, { ...base, runtime: { harden: { coating: 1, armor: 2 } } }, [{ physicalDamage: 1 }], { seed })
}
for (const patch of [{ physicalDamage: 10 }, { physicalDamage: 10, magicDamage: 20 }, { tragicCircle: true }]) {
  record(`golem-${cases.length}`, base, [{ playerId: 'golem:1', ...patch,
    source: { position: { x: 140, y: 100 }, collisionRadius: 15, reflectableActorId: 7 } }], { golem: true, golemAge: 400 })
}
for (const [name, source] of [['inferred', undefined], ['cleared', null]]) {
  record(`parent-source-${name}`, {
    ...base, ranks: [[68, 5]], runtime: { concentrationSkillIdA: 68 },
  }, [{ physicalDamage: 5, ...(source === undefined ? {} : { source }) }], {
    seed: 3, sourceActor: true,
  })
}
writeFixture('player-contacts-goldens.json', 'tools/website-oracle/player-contacts-goldens.mjs',
  'Current Website core-server/player-contact-system.ts; explicit contact-state view', { sourceRevision, cases })
