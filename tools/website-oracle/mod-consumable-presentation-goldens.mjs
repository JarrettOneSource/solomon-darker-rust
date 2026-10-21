import { loadKernel, writeFixture } from './oracle.mjs'

const source = await loadKernel('renderer/mod-consumable-effect-presentation.ts')
const plans = []
for (const useId of [0, 7, 0x1_0000_0007]) {
  const effect = {
    color: useId === 0 ? [1, .5, 0, .25] : [.15, 1, .25, 1],
    contentId: 'fixture:consumable', expiresTick: 18001,
    playerId: 'guest', startedTick: 1, useId,
  }
  for (const tick of [1, 1.5, 31, 61, 91, 121]) {
    plans.push({ effect, tick, id: source.modConsumableEffectId(effect),
      plan: source.modConsumableEffectPlan(effect, tick) })
  }
}
const ringAlphas = []
for (let y = 0; y < source.MOD_CONSUMABLE_TEXTURE_SIZE; y += 1) {
  for (let x = 0; x < source.MOD_CONSUMABLE_TEXTURE_SIZE; x += 1) {
    ringAlphas.push(Math.round(source.modConsumableRingAlpha(x, y) * 210))
  }
}
writeFixture('mod-consumable-presentation-goldens.json',
  'tools/website-oracle/mod-consumable-presentation-goldens.mjs',
  'Pinned Website renderer/mod-consumable-effect-presentation.ts', { plans, ringAlphas })
