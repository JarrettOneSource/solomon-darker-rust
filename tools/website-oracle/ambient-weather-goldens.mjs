// Ambient Arena weather, with real collision retries, viewport changes and first-draw lighting.
import { readFileSync } from 'node:fs'
import { stripTypeScriptTypes } from 'node:module'
import { resolve } from 'node:path'
import { gameDir, loadKernel, writeFixture } from './oracle.mjs'

const kernel = await loadKernel('core-kernels/native-boneyard-weather.ts')
const cases = []
for (const [mode, enhanced] of [[0, true], [1, false], [2, false], [2, true]]) {
  const weather = new kernel.NativeBoneyardWeather({ mode, enhancedEffects: enhanced, initialTick: 17 })
  const frames = []
  for (const tick of [17, 18, 19, 20, 40, 80, 120, 160, 159]) {
    const viewportHeight = tick < 80 ? 100 : 300
    const bounds = tick < 120 ? { x: -100, y: -50, w: 600, h: 400 } : { x: 70, y: 120, w: 450, h: 300 }
    let collisionChecks = 0
    weather.advanceTo(tick, bounds, viewportHeight, (position, radius) => {
      if (radius !== 4) throw new Error('rain lost native collision radius')
      collisionChecks += 1
      return position.x < 0 && position.y < 0
    })
    const plan = weather.plan(position => tick < 80 ? .25 : position.x < 200 ? NaN : .8)
    frames.push({ tick, bounds, viewportHeight, collisionChecks,
      plan: { tick: plan.tick, mode: plan.mode, spawnCount: plan.spawnCount,
        drops: plan.drops.map(drop => [drop.id, drop.position.x, drop.position.y, drop.height, drop.length,
          drop.start.x, drop.start.y, drop.end.x, drop.end.y, drop.startColor, drop.endColor,
          drop.startAlpha, drop.endAlpha, drop.width]),
        splashes: plan.splashes.map(splash => [splash.id, splash.position.x, splash.position.y,
          splash.ageTicks, splash.alpha, splash.scale]),
      },
    })
  }
  cases.push({ mode, enhanced, frames })
}
// Evaluate the actual authored ramp body without adding a dependency or rewriting it.
const view = readFileSync(resolve(gameDir, 'renderer/native-boneyard-weather-view.ts'), 'utf8')
const pixelsBody = view.slice(view.indexOf('function nativeBoneyardWeatherStreakRampPixels()'), view.indexOf('\nfunction weatherStreakRampTexture'))
const pixelsJs = stripTypeScriptTypes(pixelsBody)
const ramp = new Function('WEATHER_STREAK_RAMP_HEIGHT', `${pixelsJs};return nativeBoneyardWeatherStreakRampPixels()`)(256)
const seeds = [0, 1, 2, 100, 4294967295, 4294967299].map(tick => [tick, kernel.nativeBoneyardWeatherTickSeed(tick)])
writeFixture('ambient-weather-goldens.json', 'ambient-weather-goldens.mjs', [
  'core-kernels/native-boneyard-weather.ts', 'renderer/native-boneyard-weather-view.ts',
], { cases, ramp: Array.from(ramp), seeds })
