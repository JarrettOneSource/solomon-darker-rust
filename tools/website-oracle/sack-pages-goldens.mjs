import { loadKernel, writeFixture } from './oracle.mjs'

const source = await loadKernel('renderer/hub-inventory-render-contract.ts')
const heights = [.001, 1, 319, 320, 320.001, 321, 599, 600, 600.001, 799, 800, 800.001, 900, 1200]
  .map(height => ({height, expected:source.nativeInventorySackPageHeight(height)}))
const frames = []
for (const direction of ['open', 'back']) {
  for (const startedAtMs of [0, 1234.567]) {
    for (const elapsed of [-20, 0, .001, 9.999, ...Array.from({length:38}, (_, index) => index * 10), 369.999, 370, 900]) {
      const nowMs = startedAtMs + elapsed
      frames.push({direction, startedAtMs, nowMs, expected:source.hubSackPageOffsets(direction, startedAtMs, nowMs)})
    }
  }
}
writeFixture('sack-pages-goldens.json', 'sack-pages-goldens.mjs',
  'Current Website fa08ae249ff5563570bdb2a653e2ef9358ce7b83 unchanged renderer/hub-inventory-render-contract.ts nativeInventorySackPageHeight/hubSackPageOffsets; native900 stage stationary clip',
  {transition:source.HUB_SACK_PAGE_TRANSITION, clip:source.HUB_SACK_PAGE_CLIP, heights, frames})
