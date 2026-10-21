import { loadKernel, writeFixture } from './oracle.mjs'

const light = await loadKernel('core-kernels/native-boneyard-light-model.ts')
const factories = []
for (const tick of [0, 1, 249]) {
  for (let headingIndex = 0; headingIndex < 24; headingIndex += 1) {
    const player = {
      id: 'local', headingIndex, position: { x: 700, y: 450 },
      lighting: { driveActive: false, overlayEffectPhase: headingIndex % 2 ? 0.5 : 0 },
    }
    factories.push({ kind: 'player', tick, player, expected: light.nativePlayerLightSource(player, tick, true) })
  }
  factories.push({ kind: 'lantern', tick, expected: light.nativeLanternLightSource({ x: 100, y: 300 }, tick) })
  factories.push({ kind: 'missile', tick, expected: light.nativeMissileLightSource({ id: 17, position: { x: 100, y: 300 } }, tick) })
}
const source = (x, y, radius, intensity, castsDirectionalShadow) => ({
  position: { x, y }, radius, intensity, castsDirectionalShadow,
})
const providers = [
  source(700.125, 450.375, 2.6, 1, true),
  source(710, 455, 0.2, 0.5, false),
  source(710, 455, 0.2, 0.5, true),
  source(-299.99, 450, 0.75, 0.8, true),
  source(1550, 820, 1.35, 0.9, false),
  source(2500, 450, 0.1, 0.3, false),
]
const tail = [source(720, 460, 0.25, 0.4, false), source(740, 470, 3, 1.2, false)]
const grids = []
for (const quality of [0.06, 0.25, 1]) {
  const view = { camera: { x: 800, y: 450, zoom: 1.35 }, viewport: { width: 1600, height: 900 } }
  const index = new light.NativeBoneyardLightIndex({ width: 2100, height: 3700 })
  index.rebuild(providers, tail, view, quality)
  const queries = []
  for (const x of [-450, -300, -299.99, 0, 149.999, 150, 500, 700, 775, 845, 1100, 1600, 2400]) {
    for (const y of [-300, 0, 150, 300, 450, 525, 595, 900, 3900]) {
      const position = { x, y }
      queries.push({
        position,
        indices: [...index.sourceIndicesAt(position)],
        scalar: index.scalarAt(position),
        surface: light.nativeBoneyardSurfaceLightScalar(position, index),
      })
    }
  }
  grids.push({ quality, view, providers, tail, accepted: [...index.acceptedSources], queries })
}
writeFixture(
  'boneyard-light-goldens.json',
  'tools/website-oracle/boneyard-light-goldens.mjs',
  'Website@fa08ae249ff5563570bdb2a653e2ef9358ce7b83/frontend/src/game/core-kernels/native-boneyard-light-model.ts and native-boneyard-lighting.ts',
  { factories, grids },
)
