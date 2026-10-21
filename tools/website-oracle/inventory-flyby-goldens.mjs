import { loadKernel, writeFixture } from './oracle.mjs'

const source = await loadKernel('renderer/hub-inventory-render-contract.ts')
const frames = [-1, 0, 9.999, ...Array.from({length: 32}, (_, tick) => tick * 10), 1000].map(elapsed => ({
  elapsed, expected: source.hubInventoryFlybyFrame(1000, 1000 + elapsed),
}))
const points = []
for (const [from, to] of [[{x: 1550, y: 350}, {x: 36, y: 534}], [{x: -20.5, y: 2.25}, {x: 120.75, y: -40}], [{x: 5, y: 6}, {x: 5, y: 6}]]) {
  for (const progress of [-1, 0, .05, .5, .95, 1, 2]) {
    points.push({from, to, progress, expected: source.hubInventoryFlybyPoint(from, to, progress)})
  }
}
writeFixture('inventory-flyby-goldens.json', 'inventory-flyby-goldens.mjs',
  'Current Website fa08ae249ff5563570bdb2a653e2ef9358ce7b83 hub-inventory-render-contract.ts InventoryFlyby public frame/point helpers',
  {frames, points})
