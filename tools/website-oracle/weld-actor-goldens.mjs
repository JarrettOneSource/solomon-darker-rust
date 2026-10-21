import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, writeFixture } from './oracle.mjs'
import { addWeldVisualSamples } from './weld-visual-rows.mjs'

const meteor = await loadKernel('core-kernels/native-weld-meteor.ts')
const weld = await loadKernel('core-kernels/native-weld-primary-runtime.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const steam = await loadKernel('core-kernels/native-weld-steam.ts')
const allProfiles = JSON.parse(readFileSync(resolve(fixturesDir, 'primary-profile-goldens.json'), 'utf8')).profiles
const profiles = allProfiles.filter(sample => sample.profile.kind === 'weld' && sample.profile.buildId === 1007)
const cases = []
for (const profile of profiles) {
  for (const underpowered of [false, true]) {
    for (const seed of [17, 731]) {
      const birth = { direction: { x: -0.6, y: 0.8 }, id: 100, origin: { x: 100.25, y: 200.125 },
        ownerId: 'hero', tick: 37, vector: profile.profile.vector.values, worldKey: 'boneyard:weld' }
      let rng = random.createNativeRng(seed)
      let { marker, rng: markerRng } = meteor.spawnNativeWeldMeteorMarker({ ...birth, rng })
      rng = markerRng
      const markerFrames = [{ tick: 0, marker }]
      for (let tick = 1; marker !== null; tick += 1) {
        marker = meteor.stepNativeWeldMeteorMarker(marker)
        markerFrames.push({ tick, marker })
      }
      const spawn = meteor.createNativeWeldMeteorSpawnProgram({ aimDirection: birth.direction,
        center: birth.origin, resolvePosition: candidate => ({ x: Math.max(50, candidate.x), y: Math.min(250, candidate.y) }),
        rng, underpowered, vector: birth.vector })
      rng = spawn.rng
      const actorInput = { ...birth, ...spawn, damage: profile.profile.damageMaximum,
        rng: undefined, underpowered }
      let actor = weld.createNativeWeldMeteor(actorInput)
      const frames = [{ tick: 0, actor, rng }]
      for (let tick = 1; actor !== null && tick <= 700; tick += 1) {
        const previousPhase = actor.phase
        const stepped = weld.stepNativeWeldWorldActor(actor, rng)
        actor = stepped.actor
        rng = stepped.rng
        if (tick <= 2 || tick % 37 === 0 || actor === null || actor.phase !== previousPhase || actor.pulseDue) {
          frames.push({ tick, actor, debris: stepped.debris ?? [], rng })
        }
      }
      cases.push({ name: `${profile.name}-${underpowered}-${seed}`, seed, birth, underpowered,
        damage: profile.profile.damageMaximum, markerFrames, spawn, frames })
    }
  }
}
const steamProfile = allProfiles.filter(sample => sample.profile.kind === 'weld' && sample.profile.buildId === 1005).at(-1).profile
const steamCases = []
for (const seed of [0, 9]) {
  for (const underpowered of [false, true]) {
    for (const obstruction of ['none', 'before', 'ahead']) {
      const input = { direction: { x: -0.6, y: 0.8 }, id: 300, damage: 0.875,
        origin: { x: 100.25, y: 200.125 }, queryOrigin: { x: 110.25, y: 215.125 },
        ownerId: 'hero', tick: 38, underpowered, vector: steamProfile.vector.values, worldKey: 'boneyard:weld' }
      const obstructionPoint = obstruction === 'none' ? () => null
        : obstruction === 'before' ? start => start : (start, end) => ({ x: (start.x + end.x) / 2, y: (start.y + end.y) / 2 })
      const born = steam.spawnNativeWeldSteamActor({ ...input, rng: random.createNativeRng(seed), obstructionPoint })
      let actor = born.actor
      const frames = [{ tick: 0, actor }]
      for (let tick = 1; actor !== null && tick <= 700; tick += 1) {
        actor = steam.stepNativeWeldSteamActor(actor)
        if (tick <= 2 || tick % 17 === 0 || actor === null || actor.contactDue) frames.push({ tick, actor })
      }
      steamCases.push({ seed, input, obstruction, frames, rng: born.rng })
    }
  }
}
const oddInput = { ...steamCases[0].input, tick: 37 }
const oddSteam = steam.spawnNativeWeldSteamActor({ ...oddInput, rng: random.createNativeRng(9) })
const steamDetonations = []
for (const explodeRadius of [0, 20]) {
  for (const fragmentCount of [0, 2]) {
    const input = { explodeDamage: 18, explodeRadius, firstFragmentId: 500, fragmentCount,
      fragmentDamage: 7, origin: { x: 100.25, y: 200.125 }, ownerId: 'hero', privateSeed: 731,
      tick: 51, worldKey: 'boneyard:weld' }
    const result = steam.createNativeWeldSteamDetonation({ ...input, rng: random.createNativeRng(17) })
    steamDetonations.push({ input, result })
  }
}
const debris = await loadKernel('core-kernels/native-weld-boulder-debris.ts')
const managers = await loadKernel('core-kernels/native-world-manager-order.ts')
const persistentProfiles = allProfiles.filter(sample => sample.profile.kind === 'weld'
  && [1006, 1008].includes(sample.profile.buildId))
const persistentInputs = persistentProfiles.flatMap(sample => [false, true].map(weak => ({
  name: `${sample.name}-${weak}`, buildId: sample.profile.buildId, vector: sample.profile.vector.values,
  weak, enhancedEffects: true,
})))
for (const quantity of [3, 4]) {
  const vector = [...persistentProfiles[1].profile.vector.values]
  vector[2] = quantity
  persistentInputs.push({ name: `boulder-${quantity}-split`, buildId: 1006, vector, weak: false, enhancedEffects: true })
}
persistentInputs.push({ name: 'hail-native-effects', buildId: 1008,
  vector: persistentProfiles.at(-1).profile.vector.values, weak: false, enhancedEffects: false })
persistentInputs.push({ name: 'meteor-field', buildId: 1007,
  vector: profiles.at(-1).profile.vector.values, weak: false, enhancedEffects: true })
const persistentCases = []
for (const [caseIndex, input] of persistentInputs.entries()) {
  const birth = { buildId: input.buildId, direction: { x: -0.6, y: 0.8 }, id: 100,
    origin: { x: 100.25, y: 200.125 }, ownerId: 'hero', tick: 37,
    vector: input.vector, worldKey: 'boneyard:weld' }
  const order = managers.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 500, transient: 70 } })
  let actor = weld.createNativeWeldPersistentActor({ ...birth, registerWorldPainter: order.register })
  let rng = random.createNativeRng(17 + caseIndex)
  const frames = [{ tick: 0, actor, rng }]
  let heldTicks = 0
  const castProgressFactor = caseIndex % 2 === 0 ? 1 : 2.6
  for (let tick = 1; tick <= 150; tick += 1) {
    const options = { castProgressFactor, enhancedEffects: input.enhancedEffects, underpowered: input.weak && tick >= 30 }
    const origin = { x: birth.origin.x + tick * 0.125, y: birth.origin.y - tick * 0.25 }
    const direction = tick % 3 === 0 ? { x: 0.8, y: 0.6 } : birth.direction
    const result = weld.updateNativeWeldPersistentActor(actor, origin, direction, rng, options)
    actor = result.actor
    rng = result.rng
    heldTicks = tick
    if ([1, 2, 19, 29, 30, 31, 59, 80, 100, 150].includes(tick) || result.releaseRequested) {
      frames.push({ tick, ...result })
    }
    if (result.releaseRequested) break
  }
  const released = weld.releaseNativeWeldPersistentActor({ actor, firstChildId: 500,
    registerWorldPainter: order.register, rng, tick: birth.tick + heldTicks })
  rng = released.rng
  let actors = released.actors.filter(actor => actor.kind === 'weld-persistent')
  const flight = []
  const blockTick = caseIndex === 0 ? 2000 : 23
  for (let tick = 1; actors.length > 0 && tick <= 1600; tick += 1) {
    const terrainContacts = []
    actors = actors.flatMap(actor => {
      const result = weld.stepNativeWeldWorldActor(actor, rng, () => tick !== blockTick)
      rng = result.rng
      if (result.terrainContact) terrainContacts.push(result.terrainContact)
      return result.actor ? [result.actor] : []
    })
    if ([1, 2, 22, 23].includes(tick) || actors.length === 0) {
      flight.push({ tick, actors, terrainContacts, contactSubsteps: actors.map(actor => actor.buildId === 1008
        ? weld.nativeWeldHailstoneFlightContactSubsteps(actor) : []), rng })
    }
  }
  const breakup = input.buildId === 1006
    ? debris.createNativeWeldEtherealBoulderBreakupDebrisProgram({ scale: actor.scale, rng }) : null
  persistentCases.push({ input, birth, castProgressFactor, heldTicks, blockTick, frames,
    released, flight, breakup, order: order.state().nextRegistrationOrdinal })
}
const hail = await loadKernel('core-kernels/native-weld-hail-contact.ts')
const hailChildCases = []
for (const sample of persistentCases.filter(sample => sample.input.buildId === 1008 && !sample.input.weak)) {
  for (const rockCount of sample.input.name.includes('bare') ? [0, 2] : [2]) {
    const actor = { ...sample.released.actors[0], rocks: sample.released.actors[0].rocks.slice(0, rockCount) }
    if (!sample.input.enhancedEffects) actor.pushback = 0.125
    let rng = random.createNativeRng(731)
    const terrain = hail.createNativeWeldHailTerrainImpact({ actor, enhancedEffects: sample.input.enhancedEffects,
      firstId: 700, rng, tick: 200 })
    rng = terrain.rng
    const contact = hail.createNativeWeldHailContactPresentation({ actor, end: { x: 111, y: 222 },
      firstId: terrain.nextId, rng, start: { x: 11, y: 22 }, tick: 200 })
    rng = contact.rng
    const knockback = hail.createNativeWeldHailKnockback({ actor, id: terrain.nextId + 2,
      targetId: 'enemy:1', tick: 200 })
    let actors = [...terrain.actors, ...contact.actors, ...(knockback ? [knockback] : [])]
    const frames = [{ tick: 0, actors, rng }]
    for (let tick = 1; tick <= 220; tick += 1) {
      actors = actors.flatMap(actor => {
        const stepped = hail.stepNativeWeldHailChild(actor, rng)
        rng = stepped.rng
        return stepped.actor ? [stepped.actor] : []
      })
      if ([1, 2, 3, 17, 39, 79, 139, 220].includes(tick)) frames.push({ tick, actors, rng })
    }
    hailChildCases.push({ actor, enhancedEffects: sample.input.enhancedEffects, frames })
  }
}
for (const sample of cases) {
  const impact = sample.frames.find(frame => frame.actor?.phase === 'impact')?.tick
  addWeldVisualSamples(sample, sample.frames, frame => [frame.actor], [0, 37, impact])
  const markers = {}
  addWeldVisualSamples(markers, sample.markerFrames, frame => [frame.marker], [0, 3])
  sample.markerVisuals = markers.visuals
}
for (const sample of steamCases) addWeldVisualSamples(sample, sample.frames, frame => [frame.actor], [0, 17])
for (const sample of persistentCases) {
  addWeldVisualSamples(sample, sample.frames, frame => [frame.actor], [0, 59, sample.heldTicks])
  const flight = {}
  addWeldVisualSamples(flight, sample.flight, frame => frame.actors, [1, 22])
  sample.flightVisuals = flight.visuals
}
for (const sample of hailChildCases) addWeldVisualSamples(sample, sample.frames, frame => frame.actors, [0, 3])
writeFixture('weld-actor-goldens.json', 'tools/website-oracle/weld-actor-goldens.mjs',
  'Website/frontend/src/game/core-kernels/native-weld-primary-runtime.ts', {
    cases, steamCases, oddSteam: { input: oddInput, result: oddSteam }, steamDetonations, persistentCases, hailChildCases,
  })
