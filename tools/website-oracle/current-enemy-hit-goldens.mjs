#!/usr/bin/env node
// The actual current damage/store boundary owns independent visual and movement latches.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const store = await loadKernel('core-server/boneyard-enemy-store.ts')
const work = await loadKernel('core-server/enemies/work.ts')
const construction = await loadKernel('core-server/enemies/construction.ts')
const damage = await loadKernel('core-server/enemies/damage.ts')
const managers = await loadKernel('core-kernels/native-world-manager-order.ts')
const etherDrain = await loadKernel('core-kernels/native-ether-drain.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], {
  cwd: websiteDir,
  encoding: 'utf8',
}).trim()

function snapshot(world, tick) {
  const actor = world.actors[0]
  return {
    tick,
    health: actor.currentHealth,
    hit: actor.hitFeedback,
    reaction: actor.hitReactionTimer,
    position: actor.position,
    nextMovementTick: actor.nextMovementTick,
    lastMovementTick: actor.lastMovementTick,
    rng: world.rngState,
  }
}

const cases = []
for (const [token, nativeTypeId] of [['SKELETON', 1001], ['SKELETONARCHER', 1002], ['SKELETONMAGE', 1003]]) {
  for (const hitStrength of [0, 1]) {
    for (const suppressHitReaction of [false, true]) {
      const seed = `current-reaction:${token}:${hitStrength}:${suppressHitReaction}`
      const order = managers.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 0, transient: 0 } })
      const context = {
        tick: 0,
        players: { hero: {
          alive: true, collisionRadius: 25, connected: true, eligible: true,
          position: { x: 450, y: 280 }, velocityPerTick: { x: 0, y: 0 },
        } },
        resolveSpawnIntents: () => [],
        registerWorldPainter: order.register,
        navigation: { isPathClear: () => true, findRoute: () => null },
        resolveMovement: request => request.requestedPosition,
        projectileWorldBlocked: () => false,
      }
      const initial = work.createEnemyWork(store.createBoneyardEnemyStore(seed, 50), context, true)
      initial.actors.push(...construction.materializeSpawnIntents(initial, context, [{
        enemyToken: token, nativeTypeId, flags: [], id: 1,
        locationPolicy: 'anywhere', position: { x: 150, y: 250 }, spawnTick: 0, waveOrdinal: 31,
      }]))
      let world = work.finishEnemyStore(initial, 0)
      const result = damage.damageBoneyardEnemy(world, {
        actorId: world.actors[0].id, amount: .125, tick: 0,
        sourcePlayerId: 'hero', hitStrength, suppressHitReaction,
        registerWorldPainter: order.register,
      })
      world = result.store
      const frames = [snapshot(world, 0)]
      for (let tick = 1; tick <= 30; tick += 1) {
        context.tick = tick
        world = store.stepBoneyardEnemyStore(world, context).store
        frames.push(snapshot(world, tick))
      }
      cases.push({ seed, token, hitStrength, suppressHitReaction,
        result: { accepted: result.accepted, healthDamage: result.healthDamage,
          killed: result.killed, specialDeathAccepted: result.specialDeathAccepted }, frames })
    }
  }
}

const captureFamilies = ['SKELETON', 'SKELETONARCHER', 'SKELETONMAGE', 'ZOMBIE', 'DEMON',
  'SPIDER', 'IMP', 'WRAITH', 'MAGGOT', 'COFFIN', 'COCOON', 'PORTAL', 'DIREFACULTY',
  'DEMONSKULL', 'HEARTMONGER'].map(family => [family, etherDrain.nativeEtherDrainCapturesFamily(family)])
const captureCases = []
for (const [token, nativeTypeId, flags] of [
  ['SKELETON', 1001, []], ['SKELETONARCHER', 1002, []], ['SKELETONMAGE', 1003, []],
  ['ZOMBIE', 1006, ['FLAG_ROTTEN']], ['DEMON', 1009, ['FLAG_DEATHIMPS']],
  ['SPIDER', 2057, []], ['IMP', 1004, ['FLAG_SPLIT']], ['WRAITH', 1007, []],
]) {
  const seed = `current-capture:${token}`
  const order = managers.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 0, transient: 0 } })
  const context = {
    tick: 0, players: {}, resolveSpawnIntents: () => [], registerWorldPainter: order.register,
    resolveMovement: request => request.requestedPosition, projectileWorldBlocked: () => false,
  }
  const initial = work.createEnemyWork(store.createBoneyardEnemyStore(seed, 50), context, true)
  initial.actors.push(...construction.materializeSpawnIntents(initial, context, [{
    enemyToken: token, nativeTypeId, flags, id: 1, locationPolicy: 'anywhere',
    position: { x: 150, y: 250 }, spawnTick: 0, waveOrdinal: 31,
  }]))
  let world = work.finishEnemyStore(initial, 0)
  const amount = world.actors[0].currentHealth + 1
  const result = damage.damageBoneyardEnemy(world, {
    actorId: world.actors[0].id, amount, tick: 0, sourcePlayerId: 'hero',
    etherDrainCapture: true, suppressHurtSound: true, registerWorldPainter: order.register,
  })
  world = result.store
  const captured = world.actors[0].etherDrainCaptured === true
  const spiderPhaseCaptured = world.actors[0].brain.family === 'spider' && world.actors[0].brain.phase === 'captured'
  context.tick = 1
  const stepped = store.stepBoneyardEnemyStore(world, context)
  world = stepped.store
  captureCases.push({ seed, token, flags, amount,
    damage: [Number(result.accepted), result.healthDamage, Number(result.killed), Number(result.specialDeathAccepted)],
    captured, spiderPhaseCaptured,
    actorIds: world.actors.map(actor => actor.id), nextActorId: world.nextActorId,
    effectCount: world.deathEffects.length, projectileCount: world.projectiles.length,
    events: stepped.events.map(event => [event.type, event.count ?? null, event.output ?? null]),
    retiredIds: stepped.retired.map(row => row.actorId),
    rewards: stepped.rewards.map(row => [row.actorId, row.eventId, row.experience, row.playerId]),
    rng: world.rngState, steering: [world.steeringRngState.indexA, world.steeringRngState.indexB, ...world.steeringRngState.words],
    painter: [order.state().nextRegistrationOrdinal.actor, order.state().nextRegistrationOrdinal.transient],
  })
}

writeFixture('current-enemy-hit-goldens.json', 'current-enemy-hit-goldens.mjs',
  'enemies/damage.ts, actor-update.ts and public stepBoneyardEnemyStore',
  { sourceRevision, cases, captureFamilies, captureCases })
