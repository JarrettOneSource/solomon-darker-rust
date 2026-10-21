// Execute the unmodified renderer closure bodies; Pixi/DOM setup is outside this clock.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { gameDir, loadKernel, writeFixture } from './oracle.mjs'

const level = await loadKernel('renderer/level-up-presentation.ts')
const lighting = await loadKernel('core-kernels/native-boneyard-lighting.ts')
const renderer = readFileSync(resolve(gameDir, 'renderer/boneyard-world-renderer.ts'), 'utf8')
const menu = readFileSync(resolve(gameDir, 'MainMenuScene.tsx'), 'utf8')
const sceneLights = readFileSync(resolve(gameDir, 'renderer/boneyard-scene-lights.ts'), 'utf8')
const setter = renderer.match(/    setLevelUpPresentation\(presentationId\) \{([\s\S]*?)\n    \},/)[1]
const frameStart = renderer.indexOf('      const frameAt = now()')
const frameEnd = renderer.indexOf('      const worldPresentationFrame', frameStart)
const clock = renderer.slice(frameStart, frameEnd)
const admissionStart = menu.indexOf('  const levelUpPresentationId = ')
const admissionEnd = menu.indexOf('\n  const levelUpPickerPresentationId', admissionStart)
const admission = menu.slice(admissionStart, admissionEnd)
const radius = sceneLights.match(/playerLight.radius = \(([\s\S]*?)\n          \)/)[1]
const createSource = new Function('nativeFrame', 'duration', 'baseRadius', `
  let destroyed = false
  let armedLevelUpPresentationId = null
  let lastLevelUpPresentationId = null
  let levelUpPresentationStartedAt = null
  const canvas = { dataset: {} }
  const NATIVE_LEVEL_UP_PRESENTATION_DURATION_MS = duration
  const NATIVE_PLAYER_LIGHT_RADIUS = baseRadius
  return (input) => {
    const runtimeProgression = { pendingOffer: input.pendingOffer ? {} : null }
    const levelUpBarrierId = input.barrierId
    ${admission}
    const presentationId = levelUpPresentationId
    ${setter}
    const now = () => input.frameAtMs
    ${clock}
    if (armedLevelUpPresentationId === null) return null
    const levelUpFrame = nativeFrame(armedLevelUpPresentationId, levelUpPresentationElapsedMs, input.playerScreenY)
    const player = { lighting: { overlayEffectPhase: input.overlayPhase } }
    return {
      id: armedLevelUpPresentationId,
      tick: levelUpFrame.tick,
      emitting: levelUpFrame.emitting,
      lightRadius: levelUpFrame.lightRadius,
      particleCount: levelUpFrame.particles.length,
      playerLightRadius: levelUpFrame.emitting ? (${radius}) : null,
    }
  }
`)
const inputs = [
  ['run', 0, false, 10], ['run', 10, true, 10], ['run', 110, false, null],
  ['run', 510, true, 10], ['run', 1810, false, null], ['run', 2399, true, 10],
  ['run', 2400, true, 10], ['run', 2500, true, 10], ['run', 2600, true, 11],
  ['run', 2610, false, 11], ['next', 4000, true, 11], ['next', 4010, true, 12],
  ['next', 4020, false, null],
]
let previousWorld = null
let source = null
const cases = inputs.map(([worldKey, frameAtMs, pendingOffer, barrierId]) => {
  if (worldKey !== previousWorld) {
    source = createSource(level.nativeLevelUpPresentationFrame,
      level.NATIVE_LEVEL_UP_PRESENTATION_DURATION_MS, lighting.NATIVE_PLAYER_LIGHT_RADIUS)
    previousWorld = worldKey
  }
  const input = { worldKey, frameAtMs, pendingOffer, barrierId, playerScreenY: 175, overlayPhase: 0.25 }
  return { input, output: source(input) }
})
writeFixture('level-up-lifecycle-goldens.json', 'level-up-lifecycle-goldens.mjs', [
  'MainMenuScene.tsx:levelUpPresentationId',
  'renderer/boneyard-world-renderer.ts:setLevelUpPresentation/render clock',
  'renderer/boneyard-scene-lights.ts:local emitter radius',
], { cases })
