#!/usr/bin/env node
// Boss controllers and private spell states run directly from the pinned Website.
import { loadKernel, writeFixture } from './oracle.mjs'

const native = await loadKernel('core-kernels/native-rng.ts')
const faculty = await loadKernel('core-kernels/native-faculty.ts')
const actions = await loadKernel('core-kernels/native-faculty-actions.ts')
const spells = await loadKernel('core-kernels/native-faculty-spells.ts')
const bossSpells = await loadKernel('core-kernels/native-boss-spell.ts')
const guided = await loadKernel('core-kernels/native-guided-missile.ts')
const paths = await loadKernel('core-kernels/native-enemy-pathfinding.ts')
const demonSkull = await loadKernel('core-kernels/native-demon-skull.ts')
const rngRecord = rng => [rng.indexA, rng.indexB, ...rng.words]

const facultyActions = []
for (const kind of ['throw', 'two-hand', 'lightning']) {
  for (const speed of [0, .5, 1, 4]) {
    const seed = 71
    const created = actions.createNativeFacultyAction(kind, native.createNativeRng(seed))
    let action = created.action
    const samples = []
    for (let tick = 0; tick < (speed === 0 ? 3 : 700); tick += 1) {
      const stepped = actions.stepNativeFacultyAction(action, speed)
      samples.push(stepped)
      action = stepped.action
      if (stepped.complete) break
    }
    facultyActions.push({ kind, seed, speed, initial: created.action,
      rng: rngRecord(created.rng), samples })
  }
}

const facultyControllers = []
for (let primary = 0; primary < 4; primary += 1) {
  for (const memberCount of [1, 4]) {
    const seed = 71 + primary * 7 + memberCount
    const created = faculty.createNativeFaculty(native.createNativeRng(seed))
    const registered = faculty.registerNativeFaculty(created.state, created.rng, memberCount)
    const createdPath = paths.createNativeEnemyPathState(registered.rng)
    let rng = createdPath.rngState
    let path = createdPath.state
    let state = { ...registered.state, secondaryCooldown: -1, completedDispatches: 1, primaryPoll: 0 }
    const samples = []
    const initial = state
    for (let tick = 1; tick <= 2600; tick += 1) {
      const orbit = faculty.advanceNativeFacultyOrbit(path, state.orbitSign, rng)
      path = orbit.path
      const context = {
        advanceAction: tick % 400 < 350,
        actorTimeScale: tick % 500 < 30 ? 0 : tick % 500 < 150 ? .5 : 1,
        lineOfSight: tick % 700 < 600,
        targetDistanceSquared: tick % 600 < 50 ? null : tick % 600 < 100 ? 10000 : 180 ** 2,
        tick,
      }
      const stepped = faculty.stepNativeFaculty(state, orbit.rng, primary, context)
      rng = stepped.rng
      state = stepped.state
      if (tick <= 4 || tick % 50 === 0 || stepped.marker || stepped.dispatch !== null) {
        samples.push({ tick, context, marker: stepped.marker, dispatch: stepped.dispatch,
          state, path, rng: rngRecord(rng) })
      }
    }
    facultyControllers.push({ primary, memberCount, seed, initial, samples })
  }
}

const darkFireballs = []
for (const ring of [false, true]) {
  for (const targetDistance of [null, 100, 250, 600]) {
    const seed = 91
    const created = spells.createNativeDarkFireballs({ x: 125, y: 240 }, 77, ring,
      targetDistance, native.createNativeRng(seed))
    const trajectories = created.spells.map(initial => {
      let state = initial
      const samples = []
      for (let tick = 1; tick <= 450; tick += 1) {
        const stepped = spells.stepNativeDarkFireball(state, tick % 20 === 0 ? .5 : 1)
        state = stepped.spell
        if (tick <= 2 || tick % 15 === 0 || stepped.impact || state.remainingTicks <= 0) {
          samples.push({ tick, impact: stepped.impact, state })
        }
        if (stepped.impact || state.remainingTicks <= 0) break
      }
      return { initial, samples }
    })
    darkFireballs.push({ ring, targetDistance, seed, rng: rngRecord(created.rng), trajectories })
  }
}

const rainOfBones = []
const direFires = []
const guidedMissiles = []
for (const seed of [1, 71, 91]) {
  const rain = spells.createNativeRainOfBones({ x: 125, y: 240 }, native.createNativeRng(seed))
  let rng = rain.rng
  let state = rain.state
  const rainSamples = []
  for (let tick = 1; tick <= 1400; tick += 1) {
    const stepped = spells.stepNativeRainOfBones(state, rng)
    rng = stepped.rng
    state = stepped.state
    if (tick <= 4 || tick % 50 === 0 || stepped.bone !== null || state.alpha <= 0) {
      rainSamples.push({ tick, state, bone: stepped.bone, rng: rngRecord(rng) })
    }
    if (state.alpha <= 0) break
  }
  rainOfBones.push({ seed, initial: rain.state, initialRng: rngRecord(rain.rng), samples: rainSamples })

  const fire = bossSpells.createNativeDireFire(native.createNativeRng(seed))
  rng = fire.rng
  state = fire.state
  const fireSamples = []
  for (let tick = 1; tick <= 250; tick += 1) {
    const stepped = bossSpells.stepNativeDireFire(state, rng)
    rng = stepped.rng
    state = stepped.state
    if (tick <= 4 || tick % 10 === 0 || state.alpha <= 0) {
      fireSamples.push({ tick, state, rng: rngRecord(rng) })
    }
    if (state.alpha <= 0) break
  }
  direFires.push({ seed, initial: fire.state, initialRng: rngRecord(fire.rng), samples: fireSamples })

  const missile = guided.createNativeGuidedMissile(native.createNativeRng(seed), { x: 125, y: 240 }, 359, 1.5)
  state = missile.state
  const missileSamples = []
  for (let tick = 1; tick <= 2100; tick += 1) {
    const target = tick % 200 < 100 ? { x: 500, y: 150 } : null
    const actorTimeScale = tick % 20 === 0 ? .5 : 1
    const stepped = guided.stepNativeGuidedMissile(state, target, actorTimeScale)
    state = stepped.state
    if (tick <= 4 || tick % 50 === 0 || state.remainingTicks <= 0) {
      missileSamples.push({ tick, state, terrainProbe: stepped.terrainProbe })
    }
    if (state.remainingTicks <= 0) break
  }
  guidedMissiles.push({ seed, initial: missile.state, rng: rngRecord(missile.rng), samples: missileSamples })
}

const contacts = []
for (const heading of [0, 17, 90, 179, 359]) {
  const origin = { x: 125, y: 240 }
  for (let x = -100; x <= 1100; x += 25) {
    for (let y = -200; y <= 500; y += 25) {
      const point = { x, y }
      contacts.push({ heading, point, blightning: spells.nativeBlightningContains(origin, heading, point),
        tragicCircle: spells.nativeTragicCircleContains(origin, point) })
    }
  }
}

const demonSkullActions = []
for (const seed of [0, 1, 71, 99991]) {
  for (const kind of ['scream', 'bite', 'eyes', 'mouth', 'spit', 'flair']) {
    demonSkullActions.push({ seed, kind,
      action: demonSkull.createNativeDemonSkullAction(kind, 177, 231, 1.25, seed) })
  }
}
const demonSkullControllers = []
for (const capabilities of [0, 1, 2, 4, 7, 15, 32]) {
  const seed = 71 + capabilities
  const created = demonSkull.createNativeDemonSkull(native.createNativeRng(seed), 1.25, capabilities)
  let state = created.state
  let rng = created.rng
  const samples = []
  let lastActionTick = -1000
  for (let tick = 1; tick <= 2600; tick += 1) {
    const admitted = tick >= 10
    state = demonSkull.prepareNativeDemonSkullTick(state, admitted)
    const context = {
      alternatePlayer: tick % 500 < 100,
      hasAction: tick - lastActionTick < 100,
      headingDeg: tick % 250 < 125 ? 30 : 200,
      healthRatio: tick < 500 ? 1 : tick < 1000 ? .4 : tick < 1500 ? .2 : .1,
      lineOfSight: tick % 300 < 240,
      movementScale: 1.25,
      screamPlaying: tick % 500 < 275,
      targetDistanceSquared: tick % 400 < 50 ? null : tick % 400 < 200 ? 200 ** 2 : 500 ** 2,
    }
    const result = demonSkull.stepNativeDemonSkullController(state, rng, context)
    state = result.state
    rng = result.rng
    if (result.beginAction !== null) {
      lastActionTick = tick
      if (result.beginAction !== 'scream' && result.beginAction !== 'flair') {
        state = {...state, targetSpeed:Math.fround(1.25*4)}
      }
    }
    if (tick <= 12 || tick % 20 === 0 || result.beginAction !== null || result.landing) {
      samples.push({tick, admitted, state, rng:rngRecord(rng), beginAction:result.beginAction,
        incrementAttackSeed:result.incrementAttackSeed, landing:result.landing, screamShake:result.screamShake})
    }
  }
  demonSkullControllers.push({seed, capabilities, initial:created.state, initialRng:rngRecord(created.rng), samples})
}

writeFixture('enemy-boss-goldens.json', 'tools/website-oracle/enemy-boss-goldens.mjs',
  'Website/frontend/src/game/core-kernels/native-faculty.ts, native-faculty-actions.ts, native-faculty-spells.ts, native-boss-spell.ts, native-guided-missile.ts',
  { facultyActions, facultyControllers, darkFireballs, rainOfBones, direFires, guidedMissiles, contacts,
    demonSkullActions, demonSkullControllers })
console.log(`boss kernels: ${facultyActions.length} actions, ${facultyControllers.length} controllers, ${contacts.length} contact points`)
