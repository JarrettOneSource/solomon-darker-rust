// Bottom controls, nonlinear health, linear mana, poison and shield ordering.
import { loadKernel, writeFixture } from './oracle.mjs'

const layout = await loadKernel('native-hud-layout.ts')
const hud = await loadKernel('native-hud-presentation.ts')
const sizes = [[1600, 900], [1920, 1080], [2048, 900], [1600, 1600]]
const layouts = sizes.flatMap(([width, height]) => [null, 0, 0.5, 1].map(progress => ({
  width, height, progress,
  layout: progress === null
    ? layout.nativeHudControlLayout(width, height)
    : layout.nativeHudModalSlideLayout(width, height, progress),
})))
const health = [
  [50, 50, 0, 0], [25, 50, 0, 0], [0, 50, 0, 0], [-1, 50, 0, 0],
  [200, 150, 0, 0], [120, 150, 30, 100], [25, 50, 25, 100], [50, 50, 100, 100],
].map(args => {
  const presentation = hud.nativeHealthHudPresentation(...args)
  return { args, presentation, layers: [false, true].map(poisoned => ({
    poisoned,
    layers: hud.nativeHealthHudLayers(presentation.fillProgress, presentation.shieldProgress, poisoned),
  })) }
})
const mana = [[100, 100, 0], [25, 100, 0], [0, 100, 0], [-1, 100, 0], [500, 300, 50], [210, 300, 90]]
  .map(args => ({ args, presentation: hud.nativeManaHudPresentation(...args) }))
writeFixture('hud-goldens.json', 'tools/website-oracle/hud-goldens.mjs',
  ['native-hud-layout.ts', 'native-hud-presentation.ts'], { layouts, health, mana })
