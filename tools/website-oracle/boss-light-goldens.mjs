import { loadKernel, writeFixture } from './oracle.mjs'

const { nativeBossSpellLight } = await loadKernel('core-kernels/native-boss-spell-light.ts')
const kinds = ['ultra-banish', 'unholy-soul', 'mouth-beam-segment', 'unholy-burst',
  'eye-laser', 'unholy-spit', 'green-fire', 'heartmonger-flicker', 'heartmonger-soul', 'blightning', 'death-magic',
  'falling-bone', 'skull-missile', 'dark-fireball', 'rain-of-bones', 'tragic-circle', 'dire-fire']
const cases = []
for (const kind of kinds) {
  for (const ageTicks of [0, 1, 25, 26, 71]) {
    for (const multipleShadows of [false, true]) {
      for (const hasLight of ['death-magic', 'green-fire'].includes(kind) ? [false, true] : [false]) {
        const spell = { ageTicks, alpha: ageTicks / 71, fire: { life: ageTicks / 71 },
          glow: hasLight, id: 731, lightIntensity: ageTicks / 25, lightRadius: 3.5,
          kind, light: hasLight ? { intensity: 0.375, radius: 2.5 } : null,
          position: { x: 10.25, y: -90.125 } }
        const presentationFrame = ageTicks + 37
        cases.push({ spell, presentationFrame, multipleShadows,
          expected: nativeBossSpellLight(spell, presentationFrame, multipleShadows) })
      }
    }
  }
}
writeFixture('boss-light-goldens.json', 'tools/website-oracle/boss-light-goldens.mjs',
  'Website/frontend/src/game/core-kernels/native-boss-spell-light.ts', { cases })
