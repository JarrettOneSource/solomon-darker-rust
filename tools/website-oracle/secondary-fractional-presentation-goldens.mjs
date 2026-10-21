import { execFileSync } from 'node:child_process'
// Focused display-age samples from the unchanged source planner. Simulation
// snapshots keep integral ages; only this view input carries the fraction.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const sourceRevision=execFileSync('git',['rev-parse','HEAD'],{cwd:websiteDir,encoding:'utf8'}).trim()

const source = await loadKernel('renderer/native-secondary-presentation.ts')
const lighting = await loadKernel('core-kernels/native-boneyard-light-model.ts')
const recorded = JSON.parse(readFileSync(
  resolve(fixturesDir, 'secondary-presentation-goldens.json'), 'utf8',
))
const cases = []
const lights = []

function record(kind, ageTicks, overrides = {}, enhancedEffects) {
  const original = recorded.cases.find(sample => sample.actor.kind === kind)
  if (!original) throw new Error(`Missing retained source actor: ${kind}`)
  const actor = { ...original.actor, ...overrides, ageTicks: Math.floor(ageTicks) }
  const frame = 17.25
  cases.push({
    name: `${kind}-${ageTicks}-${overrides.variant ?? 'default'}`
      + (enhancedEffects === undefined ? '' : `-quality-${enhancedEffects}`),
    actor,
    ageTicks,
    frame,
    pointGain: 0.75,
    ...(enhancedEffects === undefined ? {} : { enhancedEffects }),
    plan: source.nativeSecondaryPresentationPlan(
      { ...actor, ageTicks }, frame, 0.75, enhancedEffects,
    ),
  })
}

record('storm-cloud', 12.5, { variant: 0, enhanced: true })
record('storm-cloud', 12.5, { variant: 1, enhanced: true })
for (const age of [99.5, 100.5, 199.5, 200.5]) record('golem', age)
record('golem-death', 10.5)
record('acid-rain', 12.5, { phase: 0.75, rotationRadians: 0.5 })
record('magic-trap', 12.5)
record('comet', 399.5)
record('comet-impact', 499.5)
record('dampened-projectile', 12.5, { variant: 1 })
record('dampened-projectile', 12.5, { variant: 3 })

// These families deliberately reconstruct only complete source updates.
record('ring-fire-explosion', 9.5)
record('freeze-wave-visual', 7.5)
record('prismatic-wave', 7.5)
record('magic-circle', 7.5)

// Live rendering quality is independent of the actor's retained birth flag.
// These are the only three planner families that consume the live override.
for (const enhancedEffects of [false, true]) {
  const enhanced = !enhancedEffects
  record('plane-orb-shot', 12.5, { enhanced }, enhancedEffects)
  record('storm-cloud', 12.5, { enhanced, variant: 1 }, enhancedEffects)
  record('ring-fire-fragment', 12.5, { enhanced, phase: -7 }, enhancedEffects)
}

for (const kind of ['fire-burn', 'ether-burn', 'electric-burn', 'magic-circle', 'ether-fade']) {
  const original = recorded.cases.find(sample => sample.actor.kind === kind)
  const ageTicks = kind === 'magic-circle' ? 1499.5 : kind === 'ether-fade' ? 12.5 : 0.5
  const actor = { ...original.actor, ageTicks: Math.floor(ageTicks), variant: 1 }
  const sampled = { ...actor, ageTicks }
  lights.push({
    actor, ageTicks, frame: 17,
    provider: lighting.nativeSecondaryProviderLightSource(sampled, 17, true, 0.75),
    misc: lighting.nativeSecondaryMiscLightSource(sampled),
  })
}

writeFixture(
  'secondary-fractional-presentation-goldens.json',
  'tools/website-oracle/secondary-fractional-presentation-goldens.mjs',
  'Website/frontend/src/game/renderer/native-secondary-presentation.ts and source family planners',
  { sourceRevision, cases, lights },
)
