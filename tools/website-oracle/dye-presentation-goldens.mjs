import { loadKernel, writeFixture } from './oracle.mjs'
const source = await loadKernel('renderer/hub-inventory-render-contract.ts')
const swatches = Array.from({ length: 18 }, (_, index) => ({ index, expected: source.hubDyeSwatchRect(index) }))
const opacity = []
for (const closingAt of [null, 1030, 2000]) {
  for (const now of [999, 1000, 1009, 1010, 1030, 1039, 1040, 1050, 1500, 1999, 2000, 2010, 2090, 2100, 2200]) {
    opacity.push({ openedAt: 1000, closingAt, now, expected: source.hubDyeModalOpacity(1000, closingAt, now) })
  }
}
const pulse = []
for (const selectedAt of [null, 1000]) {
  for (const now of [999, 1000, 1009, 1010, 1100, 1199, 1200, 1300]) {
    pulse.push({ selectedAt, now, expected: source.hubDyeSelectedPulse(selectedAt, now) })
  }
}
writeFixture('dye-presentation-goldens.json', 'dye-presentation-goldens.mjs',
  'Current Website fa08ae249ff5563570bdb2a653e2ef9358ce7b83 renderer/hub-inventory-render-contract.ts public Dye presentation',
  { swatches, opacity, pulse })
