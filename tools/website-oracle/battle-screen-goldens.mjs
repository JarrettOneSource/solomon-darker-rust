import { loadKernel, writeFixture } from './oracle.mjs'

const feedback = await loadKernel('renderer/native-enemy-world-feedback.ts')
const lighting = await loadKernel('core-kernels/native-boneyard-light-model.ts')
const transforms = []
for (const zoom of [1.08, 1.35, 1.8]) for (const magnitude of [0, 0.05, 0.2, 2]) {
  const camera = { x: 831.75, y: 420.125, zoom }
  const viewport = { width: 1600, height: 900 }
  const player = { x: 905.25, y: 612.5 }
  transforms.push({ camera, viewport, player, magnitude,
    result: feedback.nativeEnemyWorldFeedbackTransform(camera, viewport, player, magnitude) })
}
const covers = []
for (const displacement of [{ x: 0, y: 0 }, { x: 0.999, y: 0 }, { x: 1, y: 0 },
  { x: 0, y: -1 }, { x: -1.125, y: 5.625 }, { x: 3.75, y: -12.625 }]) {
  for (const complexLighting of [false, true]) {
    const viewport = { width: 1600, height: 900 }
    covers.push({ displacement, viewport, complexLighting,
      result: lighting.nativeArenaDisplacementCoverPlan(displacement, viewport, complexLighting) })
  }
}
writeFixture('battle-screen-goldens.json', 'tools/website-oracle/battle-screen-goldens.mjs',
  'Website renderer/native-enemy-world-feedback.ts and core-kernels/native-boneyard-light-model.ts',
  { transforms, covers })
