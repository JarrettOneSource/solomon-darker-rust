import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, readFixture, websiteDir, writeFixture } from './oracle.mjs'
const sourceRevision=execFileSync('git',['rev-parse','HEAD'],{cwd:websiteDir,encoding:'utf8'}).trim()
const source = await loadKernel('renderer/native-secondary-presentation.ts')
const secondary = await loadKernel('core-kernels/native-secondary-abilities.ts')
const projectile = await loadKernel('renderer/native-enemy-projectile-presentation.ts')
const death = await loadKernel('renderer/native-death-magic-presentation.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const tickFixture = readFixture('secondary-tick-goldens.json.zlib')
const castFixture = JSON.parse(
  readFileSync(resolve(fixturesDir, 'secondary-cast-goldens.json'), 'utf8'),
)
const cases = []
const firstByKind = new Map()
const golemPoses = new Set()
const record = (name, actor, frame = actor.ageTicks, pointGain = 1) => {
  cases.push({
    name, actor, frame, pointGain,
    plan: source.nativeSecondaryPresentationPlan(actor, frame, pointGain),
  })
}
for (const journey of [...tickFixture.cases, ...tickFixture.feedback]) {
  for (const sample of journey.samples) for (const actor of sample.state.actors) {
    if (!firstByKind.has(actor.kind)) firstByKind.set(actor.kind, actor)
    if (actor.kind === 'golem') {
      const pose = `${actor.golem.phase}-${actor.golem.poseVariant}-${actor.golem.leftLimbMode}-${actor.golem.rightLimbMode}`
      if (!golemPoses.has(pose)) {
        golemPoses.add(pose)
        record(`golem-live-${pose}`, actor)
      }
    }
  }
}
for (const sample of castFixture.cases) for (const actor of sample.state.actors) {
  if (!firstByKind.has(actor.kind)) firstByKind.set(actor.kind, actor)
}
// Most plans replay real cast/tick snapshots. These complete source fields also
// exercise the few transient kinds not retained by those earlier sample points.
const base = {
  ageTicks: 0,
  alpha: 0.75,
  damage: 10,
  enhanced: false,
  endpoint: { x: 1, y: 2 },
  frame: 0,
  freezeTicks: 100,
  golem: null,
  hitTargetIds: [],
  id: 17,
  kind: 'ice-blast',
  lifetimeTicks: 100,
  lightRegistration: null,
  midpoint: { x: 2, y: 1 },
  miscLightAppendOrdinal: null,
  ownerId: 'player',
  painterRegistrations: [],
  phase: 0.25,
  position: { x: 123.25, y: -54.5 },
  presentationRng: random.createNativeRng(77),
  quantity: 0.5,
  radius: 5,
  rank: 1,
  rotationRadians: 0.75,
  scale: 1.25,
  skillId: null,
  slowFactor: 0.05,
  targetId: null,
  variant: 0,
  velocity: { x: 0.1, y: -0.2 },
  worldKey: 'boneyard:oracle',
}
for (const kind of secondary.NATIVE_SECONDARY_ACTOR_KINDS) {
  const actor = firstByKind.get(kind) ?? { ...base, kind }
  for (const ageTicks of [0, 9, 50, 175]) {
    record(`${kind}-${ageTicks}`, { ...actor, ageTicks }, ageTicks + 0.5, 0.75)
  }
}
for (const variant of [0, 1, 2, 3]) for (const ageTicks of [0, 15, 100]) {
  record(`dampened-${variant}-${ageTicks}`, {
    ...firstByKind.get('dampened-projectile'), variant, ageTicks,
  })
}
for (const variant of [0, 1]) for (const enhanced of [false, true]) {
  record(`storm-${variant}-${enhanced}`, {
    ...firstByKind.get('storm-cloud'), variant, enhanced, ageTicks: 75,
  })
  record(`freeze-${enhanced}`, { ...firstByKind.get('freeze-wave-visual'), enhanced, ageTicks: 75 })
  record(`orb-${enhanced}`, { ...firstByKind.get('plane-orb-shot'), enhanced, ageTicks: 75 })
}
for (const variant of [0, 1, 2, 3, 4, 5, 6, 7, 8]) {
  record(`trap-selector-${variant}`, {
    ...firstByKind.get('magic-trap'), variant, ageTicks: 50, scale: 1,
  })
}
for (const ageTicks of [99, 100, 199, 200, 201]) for (const iron of [false, true]) {
  const golem = firstByKind.get('golem')
  record(`golem-assembly-${ageTicks}-${iron}`, {
    ...golem, ageTicks, golem: { ...golem.golem, iron },
  })
}
const boundaries = [
  ['magic-circle', [1499, 1500, 1518]],
  ['prismatic-wave', [99, 100, 101, 166]],
  ['comet-impact', [499, 500, 999]],
  ['ring-fire-explosion', [9, 10, 34, 35, 36]],
]
for (const [kind, ages] of boundaries) {
  for (const ageTicks of ages) {
    record(`${kind}-boundary-${ageTicks}`, { ...firstByKind.get(kind), ageTicks })
  }
}
const ownerActors = [
  { ...base, id: 1, kind: 'leviathan' },
  { ...base, id: 2, kind: 'leviathan-appendage', hitTargetIds: [1] },
  { ...base, id: 3, kind: 'leviathan-appendage', hitTargetIds: [99] },
  { ...base, id: 4, kind: 'leviathan-appendage', hitTargetIds: [] },
  { ...base, id: 5, kind: 'leviathan-appendage', hitTargetIds: [1], worldKey: 'boneyard:other' },
  { ...base, id: 6, kind: 'leviathan-appendage', hitTargetIds: [5] },
]
const owners = ['boneyard:oracle', 'boneyard:other', 'hub:Courtyard'].map(worldKey => ({
  actors: ownerActors, worldKey, entries: source.nativeSecondaryCompositeOwnerEntries(ownerActors, worldKey),
}))
const shields = []
for (const absorb of [0, 10]) for (const pulse of [0, 1, 20, 40]) for (const tick of [0, 1, 100.5]) {
  const player = {
    ...secondary.createNativeSecondaryPlayerState(),
    magicShieldAbsorb: absorb,
    magicShieldPulseTicks: pulse,
  }
  shields.push({ player, tick, plan: source.nativePlayerMagicShieldPlan(player, tick) })
}
const composites = [0, 0.5, 1, 1.5].map(scale => ({
  scale, plan: source.nativeLeviathanCompositePlan(scale),
}))
const facing = Array.from({ length: 1441 }, (_, index) => index * 0.5 - 360)
  .map(heading => ({ heading, facing: source.nativeGolemFacing(heading) }))
const projectiles = []
const projectileKinds = [
  ['arrow', 'normal'], ['arrow', 'fire'], ['arrow', 'poison'],
  ['firebolt', 'fire'], ['guided-missile', 'cold'],
  ['guided-missile', 'poison'], ['demon-bomb', 'none'], ['poison-pool', 'poison'],
]
for (const [kind, payload] of projectileKinds) {
  for (const ageTicks of [0, 25, 100, 3010]) for (const verticalOffset of [0, -25]) {
    const actor = {
      ageTicks, headingDeg: 42.5, id: 17, kind, lifetimeTicks: 3200, payload,
      position: { x: 123.25, y: -54.5 },
      speed: ageTicks === 25 ? 1.5 : 3,
      verticalOffset, visualPhaseDeg: 12.5, visualScale: 1.25,
    }
    projectiles.push({
      actor, tick: ageTicks + 3.5,
      plan: projectile.nativeEnemyProjectilePlan(actor, ageTicks + 3.5),
    })
  }
}
const deathMagic = []
for (const seed of [0, 1, 77]) for (const phase of [0, 15, 99.5]) {
  deathMagic.push({
    seed, phase,
    result: death.nativeDeathMagicLayers(
      { x: 0.25, y: -0.5 }, 1.25, phase, random.createNativeRng(seed),
    ),
  })
}
writeFixture(
  'secondary-presentation-goldens.json',
  'tools/website-oracle/secondary-presentation-goldens.mjs',
  'Website/frontend/src/game/renderer/native-secondary-presentation.ts and source family planners',
  { sourceRevision, cases, owners, shields, composites, facing, projectiles, deathMagic },
)
