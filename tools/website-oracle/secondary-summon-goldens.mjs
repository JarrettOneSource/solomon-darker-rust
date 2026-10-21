import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const sourceRevision=execFileSync('git',['rev-parse','HEAD'],{cwd:websiteDir,encoding:'utf8'}).trim()
const leviathan = await loadKernel('core-kernels/native-secondary-leviathan.ts')
const golem = await loadKernel('core-kernels/native-secondary-golem.ts')
const death = await loadKernel('core-kernels/native-death-animations.ts')
const rng = await loadKernel('core-kernels/native-rng.ts')
const secondary = await loadKernel('core-kernels/native-secondary-abilities.ts')
const manager = await loadKernel('core-kernels/native-world-manager-order.ts')
const births = []
for (const seed of [0, 1, 55, 65535]) {
  for (const maximum of [0, 1, 1.5, 2, 3, 4, 5, 9]) {
    for (const forced of [false, true]) {
      births.push({ seed, maximum, forced,
        result: leviathan.createNativeLeviathanBirth(rng.createNativeRng(seed), maximum, forced) })
    }
  }
}
const scales = Array.from({ length: 1667 }, (_, age) => ({ age,
  scale: leviathan.nativeLeviathanCurrentScale(age), phase: leviathan.nativeLeviathanPhase(age),
  active: leviathan.nativeLeviathanActive(age) }))
const geometry = []
for (const heading of [0, 11.49, 11.5, 12, 23.49, 23.5, 24, 90, 180, 270, 359.49, 359.5, 360]) {
  for (const bank of [0, 1]) {
    const parent = { x: 700, y: 500 }
    const base = { x: 15, y: -30 }
    const recoil = { x: 2.2, y: -1.3 }
    const local = leviathan.nativeLeviathanAppendageLocalRoot(base, recoil, heading, 0.4)
    geometry.push({ heading, bank, parent, base, recoil, deployment: 0.4, compositeScale: 0.75,
      spriteScale: 2.1, vector: leviathan.nativeLeviathanHeadingVector(heading),
      frame: leviathan.nativeLeviathanDirectionFrame(heading),
      record: leviathan.nativeLeviathanAppendageRecord(bank, heading), local,
      presentation: leviathan.nativeLeviathanAppendagePresentationRoot(parent, 0.75, local),
      muzzle: leviathan.nativeLeviathanMuzzlePosition(parent, local, bank, heading, 2.1) })
  }
}
const lanes = []
for (const heading of [0, 90, 180, 270, 359.5]) {
  for (const delta of [-25.001, -25, -24.999, 0, 24.999, 25, 25.001]) {
    for (const distance of [0, 299.999, 300, 300.001]) {
      const radians = (heading + delta) * Math.PI / 180
      const origin = { x: 700, y: 500 }
      const target = { x: origin.x + Math.sin(radians) * distance, y: origin.y - Math.cos(radians) * distance }
      lanes.push({ heading, origin, target,
        headingToTarget: leviathan.nativeLeviathanHeadingDegrees(origin, target),
        inside: leviathan.nativeLeviathanInsideTargetLane(origin, heading, target) })
    }
  }
}
function footResolver(mode) {
  return (current, requested) => mode === 'wall' && requested.x > 120 ? current : requested
}
function actor(rotation = Math.PI / 2, position = { x: 100, y: 100 }, mode = 'open') {
  return {
    ageTicks: 0, damageMinimum: 10, id: 1, ownerId: 'player', position, rotationRadians: rotation, targetId: null,
    golem: {
      ...golem.nativeInitialGolemArticulation(position, rotation, footResolver(mode)),
      actionDurationTicks: 0, actionTick: 0, circleSlowTicks: 0, currentHealth: 100,
      damageMaximum: 20, iron: false, maximumHealth: 100, orbitDirection: 0,
      orbitHeadingRadians: null, phase: 'assembly', poseVariant: 0, provokeRollBound: 0,
      reflectFactor: 1.5, targetPollTicksRemaining: 50,
    },
  }
}
const initialArticulations = []
for (const rotation of [-0.01, 0, 0.1, Math.PI / 2, Math.PI, 2 * Math.PI, 9]) {
  for (const mode of ['open', 'wall']) initialArticulations.push({ mode, actor: actor(rotation, { x: 119, y: 100 }, mode) })
}
const journeys = []
function journey(name, initial, mode, epochs, seed = 77) {
  let current = initial
  let random = rng.createNativeRng(seed)
  const records = []
  let tick = 0
  for (const epoch of epochs) {
    for (let localTick = 0; localTick < epoch.ticks; localTick += 1) {
      tick += 1
      const result = golem.stepNativeSecondaryGolem(current, {
        ownerPosition: epoch.owner,
        resolveFootTarget: footResolver(mode),
        resolveMovement: requested => mode === 'wall' && requested.x > 120 ? current.position : requested,
        rng: random, targets: epoch.targets,
      })
      current = result.actor
      random = result.rng
      if (tick === 1 || tick % 25 === 0 || result.assemblyMilestone !== null || result.contact !== null
        || result.footstep || result.provokeStarted || localTick === epoch.ticks - 1) {
        records.push({ tick, ...result })
      }
    }
  }
  journeys.push({ name, initial, mode, epochs, seed, records })
}
journey('assembly-and-owner-orbit', actor(), 'open', [{ ticks: 1500, owner: { x: 120, y: 120 }, targets: [] }])
const active = () => {
  const source = actor()
  return { ...source, ageTicks: 401, golem: { ...source.golem, phase: 'active', provokeRollBound: 1200, targetPollTicksRemaining: 0 } }
}
journey('attack-front-and-back', active(), 'open', [{ ticks: 1000, owner: null, targets: [
  { id: 2, position: { x: 140, y: 100 }, radius: 15 },
  { id: 3, position: { x: 80, y: 100 }, radius: 15 },
  { id: 4, position: { x: 160, y: 110 }, radius: 15 },
] }], 1)
journey('chase-disappear-and-reacquire', active(), 'open', [
  { ticks: 250, owner: null, targets: [{ id: 9, position: { x: 500, y: 100 }, radius: 15 }] },
  { ticks: 250, owner: { x: 0, y: 0 }, targets: [] },
  { ticks: 400, owner: null, targets: [{ id: 3, position: { x: 50, y: 120 }, radius: 15 }] },
], 2)
const slow = active()
journey('slowed-wall-and-feet', { ...slow, golem: { ...slow.golem, circleSlowTicks: 600 } }, 'wall', [
  { ticks: 800, owner: null, targets: [{ id: 9, position: { x: 500, y: 100 }, radius: 15 }] },
])
journey('target-range-and-equal-distance-order', active(), 'open', [
  { ticks: 75, owner: null, targets: [{ id: 8, position: { x: 1100, y: 100 }, radius: 15 }] },
  { ticks: 150, owner: null, targets: [
    { id: 7, position: { x: 1099, y: 100 }, radius: 15 },
    { id: 3, position: { x: -899, y: 100 }, radius: 15 },
  ] },
])
const impact = active()
journey('impact-and-two-foot-landing-order', { ...impact, targetId: 2,
  golem: { ...impact.golem, actionTick: 36, actionDurationTicks: 90, phase: 'attack',
    gaitTick: 17, leftFootProgress: 0.99, rightFootProgress: 0.99 } }, 'open', [
  { ticks: 200, owner: null, targets: [{ id: 2, position: { x: 140, y: 100 }, radius: 15 }] },
])
const damages = []
for (const age of [399, 400]) {
  for (const primaryDamage of [0, 10, 100, 101]) {
    for (const secondaryDamage of [0, 50]) {
      for (const reflectablePhysicalSourceInRange of [false, true]) {
        const source = { ...active(), ageTicks: age }
        const damage = { primaryDamage, secondaryDamage, reflectablePhysicalSourceInRange }
        damages.push({ actor: source, damage, result: golem.damageNativeSecondaryGolem(source, damage) })
      }
    }
  }
}
const deathRng = [0, 1, 555].map(seed => ({ seed,
  result: death.createNativeGolemDeathAnimation(rng.createNativeRng(seed), {x:0,y:0}, true).rng }))
const painterOrder = manager.createNativeWorldManagerOrder({
  nextRegistrationOrdinal: { actor: 49, transient: 10 },
})
const painterActors = secondary.NATIVE_SECONDARY_ACTOR_KINDS.flatMap((kind, index) => [0, 1].map(variant => ({
  id: index * 2 + variant + 1, kind, variant, painterRegistrations: [],
})))
const painted = secondary.enrollNativeSecondaryPainterOwners({
  ...secondary.createNativeSecondarySimulation(0), actors: painterActors,
}, painterOrder.register)
const painterCases = painted.actors.map(actor => ({
  ...actor,
  lane: secondary.nativeSecondaryPainterManagerLane(actor.kind),
  disposition: secondary.nativeSecondaryLightDisposition(actor),
}))
const nextPainterOrder = painterOrder.state()
secondary.enrollNativeSecondaryPainterOwners(painted, painterOrder.register)
writeFixture('secondary-summon-goldens.json', 'tools/website-oracle/secondary-summon-goldens.mjs',
  'Website/frontend/src/game/core-kernels/native-secondary-leviathan.ts and native-secondary-golem.ts and native-secondary-abilities.ts', {
    sourceRevision, births, scales, geometry, lanes, initialArticulations, journeys, damages, deathRng,
    painterCases, nextPainterOrder, repeatedPainterOrder: painterOrder.state(),
  })
