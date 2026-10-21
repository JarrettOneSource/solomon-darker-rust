// Fixed drawing bands and the manager-lane tie break, from the pinned Website.
import { loadKernel, writeFixture } from './oracle.mjs'

const depth = await loadKernel('hub-depth.ts')
const contract = await loadKernel('renderer/hub-render-contract.ts')
const painter = await loadKernel('region-painter-order.ts')
const actors = []
for (const y of [-3, -1, 0, 1.9, 2, 100, 834, 2000]) {
  for (const reference of [0, 100, 950.64]) {
    actors.push([y, reference, depth.hubActorDepth(y, reference)])
  }
}
const layers = [
  ['transient-first', 'transient', 0, 100],
  ['player', 'actor', 48, 100],
  ['scenery', 'scenery', 0, 100],
  ['teacher', 'actor', 5, 100],
  ['transient-next', 'transient', 1, 100],
  ['northern', 'transient', 2, 98],
]
const order = painter.buildNativeRegionPainterOrder({
  referenceY: 100,
  entries: layers.map(([id, managerLane, registrationOrdinal, worldY]) => ({
    id, registration: { managerLane, registrationOrdinal }, worldY, sortBias: 0, visible: true,
  })),
})
const insertionCases = [
  [
    ['hidden-body', 'actor', 49, 90, false, false],
    ['body-band-0', 'actor', 49, 125, true, true],
    ['body-band-1', 'actor', 49, 150, true, true],
    ['queued-actor-0', 'actor', 50, 125, true, false],
    ['queued-actor-1', 'actor', 51, 150, true, false],
    ['queued-transient', 'transient', 0, 150, true, false],
  ],
  [
    ['owner', 'actor', 10, 100, true, false],
    ['same-row-child', 'actor', 10, 100, true, true],
    ['next-row-child', 'actor', 10, 125, true, true],
    ['queued-root', 'actor', 20, 100, true, false],
  ],
].map(layers => {
  const entry = ([id, managerLane, registrationOrdinal, worldY, visible]) => ({
    id, registration: { managerLane, registrationOrdinal }, worldY, sortBias: 0, visible,
  })
  const entries = layers.filter(layer => !layer[5]).map(root => ({
    ...entry(root), insertions: layers.filter(child => child[5] && child[1] === root[1] && child[2] === root[2]).map(entry),
  }))
  const ordered = painter.buildNativeRegionPainterOrder({ entries, referenceY: 100 })
  return { layers, order: ordered.orderedLayers.map(({ id }) => layers.findIndex(layer => layer[0] === id)) }
})
writeFixture('hub-depth-goldens.json', 'tools/website-oracle/hub-depth-goldens.mjs',
  'Website/frontend/src/game: hub-depth.ts, renderer/hub-render-contract.ts, region-painter-order.ts', {
    bands: Object.entries(contract.HUB_WORLD_DEPTH).sort((a, b) => a[1] - b[1]),
    actors, layers, order: order.orderedLayers.map(({ id }) => layers.findIndex(([name]) => name === id)), insertion_cases: insertionCases,
  })
