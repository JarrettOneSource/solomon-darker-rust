import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const revision = execFileSync('git', ['-C', websiteDir, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim()
assert.equal(revision, 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83')
const state = await loadKernel('core-kernels/native-weld-flame-state.ts')
const camera = await loadKernel('core-kernels/native-weld-flame-camera.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const cameras = []
for (const seed of [17, 731]) {
  for (const underpowered of [false, true]) {
    const result = camera.drawNativeFlameCameraDisplacement(random.createNativeRng(seed), underpowered)
    cameras.push({ seed, underpowered, ...result })
  }
}
const fades = []
for (const [birthAlpha, alphaStep] of [[.5, .1], [.5, .4], [1, .2], [.5, .2], [1, .4]]) {
  for (let age = 0; age <= 5; age += 1) {
    const step = Math.fround(alphaStep)
    fades.push({ birthAlpha, alphaStep: step, age,
      alpha: state.nativeFlameFadeAlpha(birthAlpha, step, age),
      wrapperIntensity: state.nativeFlameFadeWrapperIntensity(birthAlpha, age) })
  }
}
const decays = []
for (const value of [{ x: 4, y: 0 }, { x: -4, y: 2 }, { x: .5, y: 0 }, { x: .6666666865348816, y: 0 }]) {
  for (const updates of [0, 1, 2, 7, 8]) {
    decays.push({ value, updates,
      squared: camera.nativeCameraMagnitudeSquared(value),
      result: camera.decayNativeRegionCameraDisplacement(value, updates) })
  }
}
writeFixture('flame-state-goldens.json', 'tools/website-oracle/flame-state-goldens.mjs',
  `Website@${revision}/core-kernels/native-weld-flame-{state,camera}.ts`, { cameras, fades, decays })
