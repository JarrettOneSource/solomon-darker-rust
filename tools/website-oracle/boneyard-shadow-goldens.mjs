import { loadKernel, writeFixture } from './oracle.mjs'

const shadows = await loadKernel('renderer/boneyard-complex-shadows.ts')
const fenceGeometry = await loadKernel('../editor/native-fence-geometry.ts')
const lighting = await loadKernel('core-kernels/native-boneyard-light-model.ts')
const view = { camera: { x: 800, y: 450, zoom: 1.2 }, viewport: { width: 1600, height: 900 } }
const providers = [
  { position: { x: 450.25, y: 450.5 }, intensity: 1, radius: 2.6, castsDirectionalShadow: true },
  { position: { x: 650.75, y: 480.125 }, intensity: 0.7, radius: 1.5, castsDirectionalShadow: true },
  { position: { x: 500, y: 400 }, intensity: 0.7, radius: 1, castsDirectionalShadow: false },
]
const index = new lighting.NativeBoneyardLightIndex({ width: 2100, height: 3700 })
index.rebuild(providers, [], view, 0.25)
const casters = []
for (let variant = 0; variant < 6; variant += 1) {
  casters.push({
    id: `tree-${variant}`, position: { x: 500 + variant * 20, y: 450 },
    outline: shadows.nativeBoneyardTreeComplexShadowOutline(variant),
  })
}
for (const kind of ['fence-grate', 'rails', 'wall']) {
  for (const construction of kind === 'fence-grate' ? ['gate', 'intact'] : [undefined]) {
    for (const span of [3, 170]) {
      casters.push({
        id: `${kind}-${construction}-${span}`,
        position: { x: 510, y: 470 },
        outline: [],
        program: { kind, construction, start: { x: 510, y: 470 }, end: { x: 510 + span, y: 470 + span / 3 } },
      })
    }
  }
}
for (const points of [[{x:510,y:470},{x:680,y:526.6666666667}], [{x:510,y:470},{x:510,y:470}]]) {
  for (const [part,half] of fenceGeometry.nativeBrokenFenceHalves(points,3,5).entries()) {
    casters.push({id:`broken-${points[1].x}-${part}`,position:half.root,outline:[],
      program:{kind:'fence-grate',construction:'broken',start:half.shadowStart,end:half.shadowEnd,
        step:half.shadowStep,count:half.shadowCount}})
  }
}
const cases = []
for (const frame of [0, 1, 515]) {
  for (const caster of casters) {
    const records = shadows.nativeBoneyardComplexShadowRecords(caster, index, frame)
    const edges = records.flatMap(record => {
      if (caster.program?.kind === 'fence-grate') {
        const plan = shadows.nativeBoneyardFenceGrateShadows(caster.program, record)
        return [...plan.bars, shadows.nativeBoneyardLineShadowEdge(plan.rail)]
      }
      if (caster.program?.kind === 'rails') {
        return shadows.nativeBoneyardRailsShadows(caster.program, record).map(shadows.nativeBoneyardLineShadowEdge)
      }
      if (caster.program?.kind === 'wall') return [shadows.nativeBoneyardWallShadow(caster.program, record)]
      return shadows.nativeBoneyardProjectedShadowEdges(caster, record)
    })
    cases.push({ caster, frame, records, edges })
  }
}
const targets = []
for (const viewport of [{ width: 1600, height: 900 }, { width: 390, height: 844 }]) {
  for (const resolution of [1, 2]) {
    for (const quality of [0.06, 0.25, 1]) {
      targets.push({ viewport, resolution, quality, expected: lighting.nativeRegionLightTargetPlan(viewport, resolution, quality) })
    }
  }
}
writeFixture('boneyard-shadow-goldens.json', 'tools/website-oracle/boneyard-shadow-goldens.mjs',
  'Website@fa08ae249ff5563570bdb2a653e2ef9358ce7b83/frontend/src/game/renderer/boneyard-complex-shadows.ts; core-kernels/native-boneyard-light-model.ts',
  { view, providers, cases, targets })
