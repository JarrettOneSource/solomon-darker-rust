#!/usr/bin/env node
// Current Source clocks are recorded independently; no previous expected values are inputs.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const wraith = await loadKernel('core-kernels/native-wraith-flight.ts')
const store = await loadKernel('core-server/boneyard-enemy-store.ts')
const work = await loadKernel('core-server/enemies/work.ts')
const construction = await loadKernel('core-server/enemies/construction.ts')
const managers = await loadKernel('core-kernels/native-world-manager-order.ts')
const portal = await loadKernel('core-kernels/native-survival-portal.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], {
  cwd: websiteDir,
  encoding: 'utf8',
}).trim()

const flightValues = state => [
  state.baseFlybySpeed,
  state.contactCooldownTicks,
  state.currentSpeed,
  state.currentTurnGain,
  state.flybyTicksRemaining,
  state.restingSpeed,
  state.targetTurnGain,
]

const pathValues = state => [
  state.baseTurnRate, state.flankAngleDeg, state.flankRadius, state.flankTicksRemaining,
  state.reorientationTicksRemaining, Number(state.routePreviousVector !== null),
  state.routePreviousVector?.x ?? 0, state.routePreviousVector?.y ?? 0,
  state.routeRefreshTicksRemaining, state.routeTicksRemaining, state.routeWaypointIndex,
  Number(state.routeWaypoints !== null), state.routeWaypoints?.[0].x ?? 0,
  state.routeWaypoints?.[0].y ?? 0, state.routeWaypoints?.[1].x ?? 0,
  state.routeWaypoints?.[1].y ?? 0, state.speedFactor, state.stalledMovementTicks,
  state.turnFactor, state.wanderHeadingDeg,
]

const rngValues = rng => [rng.indexA, rng.indexB, ...rng.words]

const wraithClocks = []
for (const chaseSpeed of [.1, 1, 7.75]) {
  for (const restingSample of [0, .375, .99999]) {
    for (const initialSample of [0, .99999]) {
      const flybyOffset = restingSample === 0 ? 0 : 600
      const initial = wraith.createNativeWraithFlightState(
        chaseSpeed, restingSample, initialSample, flybyOffset,
      )
      let state = initial
      const frames = []
      for (let tick = 0; tick < 1000; tick += 1) {
        const admitted = tick < 100 || tick >= 150
        state = wraith.stepNativeWraithFlightClock(state, admitted)
        frames.push({ admitted, expected: flightValues(state) })
      }
      wraithClocks.push({
        chaseSpeed, restingSample, initialSample, flybyOffset,
        initial: flightValues(initial), frames,
      })
    }
  }
}

const wraithWorlds = []
for (const view of ['visible', 'unlit', 'offscreen', 'basic']) {
  for (const hasTarget of [false, true]) {
    const seed = `current-wraith:${view}:${hasTarget}`
    const order = managers.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 0, transient: 0 } })
    const player = {
      alive: true, collisionRadius: 25, connected: true, eligible: hasTarget,
      position: { x: 210, y: 280 }, velocityPerTick: { x: .75, y: -.25 },
    }
    const context = {
      tick: 0, players: { hero: player }, resolveSpawnIntents: () => [],
      registerWorldPainter: order.register,
      navigation: { isPathClear: () => true, findRoute: () => null },
      resolveMovement: request => request.requestedPosition,
      nativeVisibility: () => ({ admitted: view !== 'unlit', intensity: view === 'unlit' ? 0 : 1 }),
      projectileWorldBlocked: () => false,
      lightAt: () => view === 'unlit' ? 0 : 1,
      nativeMovementView: {
        arenaBounds: { x: 0, y: 0, w: 1000, h: 1000 },
        cameras: [view === 'offscreen' ? { x: 2000, y: 2000, w: 500, h: 500 }
          : { x: 0, y: 0, w: 1000, h: 1000 }],
        enhancedEffects: view !== 'basic',
      },
    }
    const initial = work.createEnemyWork(store.createBoneyardEnemyStore(seed, 50), context, true)
    initial.actors.push(...construction.materializeSpawnIntents(initial, context, [{
      enemyToken: 'WRAITH', nativeTypeId: 1007, flags: [], id: 1,
      locationPolicy: 'anywhere', position: { x: 150, y: 250 }, spawnTick: 0, waveOrdinal: 31,
    }]))
    let world = work.finishEnemyStore(initial, 0)
    const frames = []
    for (let tick = 1; tick <= 600; tick += 1) {
      context.tick = tick
      player.position = { x: 210 + tick * .75, y: 280 - tick * .25 }
      context.abilityEffects = { [world.actors[0].id]: {
        timeScale: tick >= 110 && tick < 160 ? 0 : tick >= 480 && tick < 500 ? .35 : 1,
        disruptedTicks: tick >= 240 && tick < 260 ? 1 : 0,
      } }
      world = store.stepBoneyardEnemyStore(world, context).store
      const actor = world.actors[0]
      frames.push({
        tick,
        movement: [actor.position.x, actor.position.y, actor.headingDeg,
          actor.lastMovementTick ?? -1, actor.nextMovementTick],
        flight: flightValues(actor.brain),
        recipient: actor.brain.collisionRecipient,
        path: pathValues(actor.path),
        rng: world.rngState,
        steering: rngValues(world.steeringRngState),
      })
    }
    wraithWorlds.push({ seed, view, hasTarget, frames })
  }
}

const stationaryWorlds = []
for (const token of ['COFFIN', 'PORTAL']) {
  const seed = `current-stationary:${token}`
  const order = managers.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 0, transient: 0 } })
  let movementCalls = 0
  let movementRadius = null
  const context = {
    tick: 0, players: {}, resolveSpawnIntents: () => [],
    registerWorldPainter: order.register,
    projectileWorldBlocked: () => false,
    resolveMovement: request => {
      if (request.purpose === 'spawn-placement') return request.requestedPosition
      movementCalls += 1
      movementRadius = request.radius
      return { x: request.requestedPosition.x + 3.25, y: request.requestedPosition.y - 1.5 }
    },
  }
  const phase = Object.values(portal.NATIVE_PORTAL_PROGRAM_BY_SOURCE_SHA256)[0].phases[0]
  const initial = work.createEnemyWork(store.createBoneyardEnemyStore(seed, 50), context, true)
  initial.actors.push(...construction.materializeSpawnIntents(initial, context, [{
    enemyToken: token, nativeTypeId: token === 'COFFIN' ? 1013 : 5021,
    flags: [], id: 1, locationPolicy: 'anywhere', position: { x: 150, y: 250 },
    spawnTick: 0, waveOrdinal: 31,
    ...(token === 'PORTAL' ? { authoredRecipe: portal.nativePortalRecipe(phase) } : {}),
  }]))
  let world = work.finishEnemyStore(initial, 0)
  const frames = []
  for (let tick = 1; tick <= 40; tick += 1) {
    context.tick = tick
    context.abilityEffects = { [world.actors[0].id]: { timeScale: tick >= 5 && tick < 9 || tick >= 25 && tick < 30 ? 0 : 1 } }
    world = { ...world, actors: world.actors.map(actor => ({ ...actor,
      position: { x: actor.position.x + 100, y: actor.position.y + 80 },
    })) }
    movementCalls = 0
    movementRadius = null
    world = store.stepBoneyardEnemyStore(world, context).store
    const actor = world.actors[0]
    const anchor = actor.brain.anchorPosition
    frames.push({ tick, position: [actor.position.x, actor.position.y],
      anchor: anchor === null ? null : [anchor.x, anchor.y],
      movementCalls, movementRadius,
      steering: rngValues(world.steeringRngState), rng: world.rngState,
    })
  }
  stationaryWorlds.push({ token, seed, recipe: token === 'PORTAL' ? portal.nativePortalRecipe(phase) : null, frames })
}

writeFixture('current-enemy-goldens.json', 'current-enemy-goldens.mjs',
  'core-kernels/native-wraith-flight.ts and public stepBoneyardEnemyStore',
  { sourceRevision, wraithClocks, wraithWorlds, stationaryWorlds })
