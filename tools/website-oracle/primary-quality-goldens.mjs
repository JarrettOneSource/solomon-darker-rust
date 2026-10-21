// Current quality changes only; unchanged corona art remains in primary-presentation goldens.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const revision = execFileSync('git', ['-C', websiteDir, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim()
assert.equal(revision, 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83')
const air = await loadKernel('core-kernels/native-air-presentation.ts')
const fire = await loadKernel('core-kernels/primary-spell-fire-native.ts')
const fireView = await loadKernel('renderer/primary-spell-fire-native.ts')
const mesh = layer => ({ alpha: layer.alpha, indices: Array.from(layer.indices), parameters: Array.from(layer.parameterSamples),
  phase: layer.phaseDegrees, offset: layer.phaseOffset, record: layer.textureRecord, tint: layer.tint,
  uvs: Array.from(layer.uvs), vertices: Array.from(layer.vertices), width: layer.width })
const base = { id: 7, birthTick: 17, endpoint: { x: 700, y: 800 }, midpoint: { x: 400, y: 350 } }
const airCases = []
for (const enhancedEffects of [true, false]) for (const chained of [false, true]) for (const underpowered of [false, true]) {
  const input = { ...base, chained, enhancedEffects, underpowered }
  const contacts = []
  for (let ageTicks = 0; ageTicks <= 5; ageTicks += 1) {
    const plan = air.buildNativeAirLightningPlan({ ...input, ageTicks })
    contacts.push({ age: ageTicks, alpha: plan.contactCorona.alpha, light: plan.contactLight })
  }
  airCases.push({ input, lifetime: air.nativeAirContactLifetimeTicks(input), contacts,
    ...(chained ? {} : { body: air.buildNativeAirLightningPlan({ ...input, ageTicks: 0 }).body.layers.map(mesh) }) })
}
const paths = []
for (const enhancedEffects of [true, false]) for (const weakCast of [false, true]) {
  const input = { ...base, origin: { x: 100, y: 200 }, weakCast, enhancedEffects }
  paths.push({ input, lights: air.buildNativeAirPathLightSources(input) })
}
const fireCases = []
for (const id of [1, 7]) for (const enhancedEffects of [true, false]) for (const ageTicks of [0, 10, 25]) {
  const state = { ageTicks, direction: { x: .6, y: -.8 }, enhancedEffects, id,
    origin: { x: 300.25, y: 400.125 }, variant: 3 }
  fireCases.push({ state, fade: fire.nativeFireParticleFadeStep(id, enhancedEffects),
    lifetime: fire.nativeFireParticleLifetimeTicks(id, enhancedEffects), plan: fireView.nativeFireParticlePlan(state) })
}
writeFixture('primary-quality-goldens.json', 'tools/website-oracle/primary-quality-goldens.mjs',
  `Website@${revision}/core-kernels/{native-air-presentation,primary-spell-fire-native}.ts`, { airCases, paths, fireCases })
