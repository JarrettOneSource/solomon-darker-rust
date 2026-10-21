import { loadKernel, writeFixture } from './oracle.mjs'

const trees = await loadKernel('renderer/boneyard-tree-occlusion.ts')
const gates = await loadKernel('../editor/native-fence-geometry.ts')
const painters = await loadKernel('region-painter-order.ts')
const treeCases = []
for (let variant = 0; variant < 8; variant += 1) {
  const tree = { eid: `object-${variant}`, position: { x: 100, y: 250 },
    mainVariant: 0, secondaryVariant: variant, secondaryVisible: true }
  const polygon = trees.NATIVE_TREE_OCCLUSION_POLYGONS[variant]
  const bounds = trees.NATIVE_TREE_OCCLUSION_BOUNDS[variant]
  const points = [{ x: 100, y: 150 }, { x: 900, y: 900 },
    { x: 100 + polygon[0].x, y: 250 + polygon[0].y }]
  let state = trees.createNativeTreeOcclusionState(trees.nativeTreeInitialCountdown(tree.eid))
  const ticks = []
  for (let tick = 0; tick < 160; tick += 1) {
    const position = points[Math.floor(tick / 55) % points.length]
    const inside = trees.nativeTreeContainsLocalPlayer(tree, position)
    state = trees.advanceNativeTreeOcclusionTick(state, inside)
    ticks.push({ position, inside, ...state })
  }
  treeCases.push({ tree, polygon, bounds, ticks })
}
const gateCases = []
for (const tip of [{ x: 240, y: 300 }, { x: 200.125, y: 255.875 }, { x: 220.75, y: 330.125 }]) {
  const hinge = { x: 150.5, y: 300.25 }
  const leaf = gates.nativeGateLeaf(hinge, tip)
  gateCases.push({
    hinge, tip, vertices: Array.from(gates.nativeGateArtVertices(leaf)),
    hingeSprite: gates.nativeGateHingeArtPosition(leaf),
    lines: gates.nativeGateRules(leaf),
    painterY: gates.nativeGatePainterRoot(hinge, tip).y,
  })
}
const entries = [
  { id: '0', registration: { managerLane: 'scenery', registrationOrdinal: 2 },
    worldY: 100, sortBias: 0, visible: true,
    insertions: [{ id: '10', worldY: 200, sortBias: 0, visible: true }] },
  { id: '1', registration: { managerLane: 'actor', registrationOrdinal: 1 },
    worldY: 200, sortBias: 0, visible: true },
  { id: '2', registration: { managerLane: 'transient', registrationOrdinal: 1 },
    worldY: 200, sortBias: 0, visible: true },
  { id: '3', registration: { managerLane: 'scenery', registrationOrdinal: 1 },
    worldY: 201, sortBias: 0, visible: true,
    insertions: [{ id: '11', worldY: 201, sortBias: 0, visible: true }] },
]
const orders = [0, 99.5, 100, 101, 200].map(referenceY => ({
  referenceY, entries,
  order: painters.buildNativeRegionPainterOrder({ entries, referenceY }).orderedLayers.map(layer => Number(layer.id)),
}))
writeFixture('boneyard-presentation-goldens.json',
  'tools/website-oracle/boneyard-presentation-goldens.mjs',
  'Website@fa08ae249ff5563570bdb2a653e2ef9358ce7b83/frontend/src/game/renderer/boneyard-tree-occlusion.ts; editor/native-fence-geometry.ts; game/region-painter-order.ts',
  { trees: treeCases, gates: gateCases, orders })
