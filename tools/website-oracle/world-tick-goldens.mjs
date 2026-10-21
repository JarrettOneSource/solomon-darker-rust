#!/usr/bin/env node
// Real movement/loot/enemy/wave/reward composition, including deferred projectile painters.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const { STOCK_TUTORIAL_BONEYARD } = await loadKernel('host/stock-tutorial-boneyard.ts')
const { createBoneyardWorld, prepareBoneyardWorldNavigation } = await loadKernel('core-server/boneyard-world-construction.ts')
const { stepBoneyardWorldTick } = await loadKernel('core-server/boneyard-world.ts')
const { spawnPlayerCharacterInBoneyard } = await loadKernel('core-server/boneyard-world-placement.ts')
const { damageBoneyardEnemy } = await loadKernel('core-server/enemies/damage.ts')
const { createIdlePlayerCharacterInput } = await loadKernel('core-kernels/player-character.ts')
const { createNativeWorldManagerOrder, createDeferredNativeWorldManagerRegistrations } = await loadKernel('core-kernels/native-world-manager-order.ts')
const { NATIVE_LOOT_DEFAULT_MODIFIERS } = await loadKernel('core-kernels/native-loot.ts')
const tutorial = await loadKernel('core-kernels/native-tutorial.ts')
const navigation = await loadKernel('core-server/boneyard-enemy-navigation.ts')
const escapeNine = JSON.parse(readFileSync(resolve(fixturesDir, 'solomon-escape-goldens.json'), 'utf8')).cases[9]

const reg = registration => registration === null ? null : [registration.managerLane, registration.registrationOrdinal]
const point = position => [position.x, position.y]
const solomon = world => world.encounter === null ? null : ({
  phase: world.encounter.phase,
  position: point(world.encounter.position),
  headingDeg: world.encounter.headingDeg,
  escapeTarget: world.encounter.escapeTarget === null ? null : point(world.encounter.escapeTarget),
  escapeCollisionSourceIds: world.encounter.escapeCollisionSourceIds,
})
function snapshot(world, player, order, result) {
  const store = world.enemies
  return {
    player: [point(player.position), point(player.velocity), player.headingIndex, player.gaitDegrees, player.walkCyclePrimary],
    lantern: world.lanternPosition === null ? null : point(world.lanternPosition),
    feedback: world.enemyWorldFeedback,
    encounter: world.encounter === null ? null : [world.encounter.phase, world.encounter.runEventId,
      world.encounter.voiceTicksRemaining, world.encounter.rngState],
    solomon: solomon(world),
    gates: world.gateLeaves.map(leaf => [point(leaf.hinge), point(leaf.tip), point(leaf.velocity), leaf.damping]),
    waves: world.waves === null ? null : [world.waves.phase, world.waves.waveOrdinal,
      world.waves.lowPopulationTicks, world.waves.activeGroupIndex, world.waves.spawnCountdown, world.waves.rngState],
    tutorial: world.tutorial,
    actors: store.actors.map(actor => [actor.id, actor.config.enemyToken, point(actor.position), actor.headingDeg,
      actor.currentHealth, actor.lifeState, actor.spawnTick, actor.config.scale, actor.lightRegistration.registrationOrdinal]),
    projectiles: store.projectiles.map(projectile => [projectile.id, projectile.kind, point(projectile.position),
      projectile.ageTicks, projectile.lifetimeTicks, reg(projectile.painterRegistration), reg(projectile.lightRegistration)]),
    counters: [store.nextActorId, store.nextEventId, store.nextProjectileId, store.nextDeathEffectId,
      store.nextNativeCellBindingOrder, store.nextNativeRegistrationOrder],
    enemyRng: [store.rngState, store.steeringRngState, store.locomotionRngState, store.headFacingRngState],
    loot: { sharedRng: world.loot.sharedRng, nextActorId: world.loot.nextActorId, nextItemId: world.loot.nextItemId,
      lastSuccessfulItemLevel: world.loot.lastSuccessfulItemLevel, nextKeyDropLevel: world.loot.nextKeyDropLevel,
      actors: world.loot.actors.map(actor => [actor.id, actor.kind, point(actor.position), actor.ageTicks,
        actor.amount, actor.orbValue, actor.item?.nativeTypeId ?? null, actor.item?.name ?? null, actor.spawnTick,
        reg(actor.painterRegistration)]),
      effects: world.loot.effects.map(effect => [effect.id, effect.ageTicks, effect.entry, effect.alpha]),
    },
    damage: result?.playerDamage.map(damage => [damage.actorId, damage.eventId, damage.playerId,
      damage.physicalDamage, damage.magicDamage, damage.poisonDamage, damage.coldSlowTicks]) ?? [],
    rewards: result?.rewards ?? [],
    pickups: result?.lootPickups.map(pickup => [pickup.actorId, pickup.amount, pickup.bonusKind,
      pickup.item?.nativeTypeId ?? null, pickup.item?.name ?? null, pickup.kind, pickup.orbKind,
      pickup.orbValue, pickup.playerId, point(pickup.sourcePosition), pickup.tick]) ?? [],
    contacts: result?.movementContactsByPlayerId.local ?? [],
    retainedTargets: [world.primarySceneryTargets.length, world.loot.goodies.length],
    order: order.state(),
  }
}

const cases = []
for (const scenario of ['encounter-waves', 'encounter-template-nine', 'pre-encounter-gate-birth', 'enemy-rewards', 'tutorial-item-wave']) {
  const isTutorial = scenario === 'tutorial-item-wave'
  const isGateBirth = scenario === 'pre-encounter-gate-birth'
  const isEncounter = scenario.startsWith('encounter-')
  const templateIndex = scenario === 'encounter-template-nine' ? 9 : 0
  const template = isTutorial ? STOCK_TUTORIAL_BONEYARD : NATIVE_GENERATED_BONEYARDS[templateIndex]
  const seed = scenario === 'encounter-template-nine' ? escapeNine.seed : `world-tick-${scenario}`
  const loaded = { ...template, choice: { id: isTutorial ? 'stock-tutorial' : 'default-random', source: 'default' }, seed, runId: 'oracle' }
  const order = createNativeWorldManagerOrder()
  const playerLight = order.register('actor')
  let world = createBoneyardWorld(loaded, order.register('actor'), order.register('actor'))
  prepareBoneyardWorldNavigation(world)
  let player = spawnPlayerCharacterInBoneyard({ displayName: 'oracle', element: 'ether', discipline: 'arcane' }, world)
  let gateBirthPosition = null
  if (isGateBirth) {
    const gate = template.scene.fences.find(fence => (fence.segmentCode ?? fence.style ?? 0) === 2)
    assert(gate)
    const [start, end] = gate.points
    const dx = end.x - start.x
    const dy = end.y - start.y
    const length = Math.hypot(dx, dy)
    const middle = { x: (start.x + end.x) / 2, y: (start.y + end.y) / 2 }
    player.position = { x: middle.x + dy / length * 100, y: middle.y - dx / length * 100 }
    gateBirthPosition = { x: middle.x - dy / length * 100, y: middle.y + dx / length * 100 }
  } else if (isEncounter) {
    player.position = scenario === 'encounter-template-nine'
      ? { ...escapeNine.players.local.position }
      : { x: world.encounter.position.x + 30, y: world.encounter.position.y + 50 }
  } else {
    // This exact Tutorial focus is mobile and route-admitted in navigation-goldens.json.
    player.position = isTutorial
      ? { x: 554.3602294921875, y: 234.46218872070312 }
      : { x: 1000, y: 1000 }
    world = { ...world, encounter: null, waves: null, arenaTransition: null }
  }
  if (isTutorial) {
    world = { ...world, tutorial: { ...world.tutorial, introActive: false, introBlend: 1,
      introDelayTicksRemaining: 0, introFade: 0, introMovementTicksRemaining: 0, stage: 5 } }
  }
  const combat = { alive: true, eligible: true, collisionEnabled: true, movementScale: 1,
    advancedUnlocks: new Array(8).fill(false), level: 1, lootModifiers: NATIVE_LOOT_DEFAULT_MODIFIERS,
    ownedRecipeIndexes: [], inventoryHasHealthPotion: false }
  const frames = []
  const solomonFrames = []
  const injected = []
  const initialPlayerPosition = point(player.position)
  const fullBoundsPrepared = []
  const fullMeshPrepared = () => navigation.boneyardNavigationMeshIsPrepared(world.bounds, world.collision, 25)
  if (isGateBirth) {
    fullBoundsPrepared.push([0, fullMeshPrepared()])
    assert.equal(fullMeshPrepared(), false)
  }
  const finalTick = isEncounter ? 2000 : isGateBirth ? 200 : 500
  for (let tick = 1; tick <= finalTick; tick += 1) {
    const selected = world.enemies.actors.find(actor => actor.lifeState === 'alive')
    if (selected && tick % 67 === 0) {
      const request = { actorId: selected.id, amount: 1e9, tick: tick - 1, sourcePlayerId: 'local' }
      world = { ...world, enemies: damageBoneyardEnemy(world.enemies, { ...request, registerWorldPainter: order.register }).store }
      injected.push({ tick, request })
    }
    let external = []
    if (isGateBirth && (tick === 1 || tick === 120)) {
      external = [{ enemyToken: 'ZOMBIE', flags: [], id: tick === 1 ? 1 : 2,
        locationPolicy: 'anywhere', nativeTypeId: 1006, position: gateBirthPosition,
        positionPolicy: 'direct', spawnTick: tick, waveOrdinal: 0 }]
    }
    if (scenario === 'enemy-rewards' && tick === 1) {
      external = ['SKELETON', 'SKELETONARCHER', 'SKELETONMAGE', 'IMP', 'ZOMBIE', 'SPIDER'].map((enemyToken, index) => ({
        enemyToken, flags: [], id: index + 1, locationPolicy: 'near-player', nativeTypeId: [1001, 1002, 1003, 1004, 1006, 2057][index],
        position: { x: 1050 + index * 30, y: 1100 }, positionPolicy: 'direct', spawnTick: tick, waveOrdinal: 0,
      }))
    }
    if (isTutorial) {
      const stepped = tutorial.stepNativeTutorial(world.tutorial, {
        acidRainCastSequence: 1, acidRainLastSkillId: 72,
        cameraLockSafetyClear: tutorial.nativeTutorialCameraLockSafetyClear([
          ...world.enemies.actors.map(actor => ({ position: actor.position, radius: actor.config.collisionRadius })),
          ...world.enemies.maggots.map(actor => ({ position: actor.position, radius: actor.collisionRadius })),
          ...world.loot.actors.filter(actor => actor.kind === 'sack').map(actor => ({ position: actor.position, radius: 15 })),
        ]), currentHealth: 100, enemyCount: world.enemies.actors.length,
        groundSackCount: world.loot.actors.filter(actor => actor.nativeTypeId === 2013).length,
        hasTopLevelNonPotionItem: false, healthPotionCount: 0, level: 1, levelUpPending: false,
        maximumHealth: 100, playerActionIdle: !player.primaryCast.held, playerPosition: player.position,
        primaryCastSequence: player.primaryCast.castSequence, solomonPhase: null, solomonRunEventId: 0, tick,
      })
      world = { ...world, tutorial: stepped.state }
      external = [...stepped.spawnIntents, ...external]
    }
    const deferred = createDeferredNativeWorldManagerRegistrations()
    const input = { ...createIdlePlayerCharacterInput(),
      movement: isGateBirth && tick > 10 && tick <= 110 ? { x: 0, y: 1 } : { x: 0, y: 0 } }
    const result = stepBoneyardWorldTick(world, { local: player }, { local: input },
      { local: combat }, tick, order.register, deferred.register, {}, [], external, undefined,
      world.tutorial !== null && tutorial.nativeTutorialHostileScenePaused(world.tutorial), {
        playerEntities: { identities: [{ playerId: 'local' }], lightings: [{ lightRegistration: playerLight, overlayEffectPhase: 0 }] },
      })
    deferred.commit(order)
    world = result.world
    player = result.players.local
    if (isGateBirth && [1, 120, finalTick].includes(tick)) {
      fullBoundsPrepared.push([tick, fullMeshPrepared()])
      assert.equal(fullMeshPrepared(), true)
    }
    if (world.encounter?.phase === 'escaping'
      || (world.encounter?.phase === 'gone' && solomonFrames.at(-1)?.state.phase !== 'gone')) {
      solomonFrames.push({ tick, state: solomon(world) })
    }
    if (tick === 1 || tick % 25 === 0 || result.rewards.length || result.lootPickups.length) {
      frames.push({ tick, state: snapshot(world, player, order, result) })
    }
  }
  cases.push({ scenario, templateIndex, seed, finalTick, initialPlayerPosition,
    gateBirthPosition, fullBoundsPrepared, injected, solomonFrames, frames })
}
writeFixture('world-tick-goldens.json', 'tools/website-oracle/world-tick-goldens.mjs',
  'Website/frontend/src/game/core-server/boneyard-world.ts with native deferred session painter registrations', {
    sourceRevision: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim(), cases })
