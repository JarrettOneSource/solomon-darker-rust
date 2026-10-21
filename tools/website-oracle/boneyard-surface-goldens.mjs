// Current runtime scenery uses these same kernels for stock and authored worlds.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const revision = execFileSync('git', ['-C', websiteDir, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim()
assert.equal(revision, 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83')
const field = await loadKernel('native-arena-field.ts')
const road = await loadKernel('renderer/native-boneyard-surface.ts')
const mesh = plan => ({ positions: Array.from(plan.positions), uvs: Array.from(plan.uvs),
  colors: Array.from(plan.colors), indices: Array.from(plan.indices) })
const fields = []
for (const bounds of [
  { x: 0, y: 0, w: 901, h: 801 },
  { x: -123.45, y: .125, w: 700.5, h: 350.75 },
  { x: 33554449, y: -33554449, w: 1100, h: 700 },
]) {
  const lattice = field.createNativeArenaFieldLattice(bounds)
  const views = [bounds,
    { x: bounds.x + 350, y: bounds.y, w: 350, h: 350 },
    { x: bounds.x - 500, y: bounds.y - 500, w: 100, h: 100 },
    { x: bounds.x + 349.99999, y: bounds.y + 349.99999, w: .01, h: .01 },
  ]
  for (const [index, view] of views.entries()) {
    const mode = [0, 1, 2, 255][index]
    const range = field.nativeArenaFieldVisibleRange(lattice, view)
    fields.push({ bounds, view, mode, range, lattice: { x: Array.from(lattice.x), y: Array.from(lattice.y) },
      descriptor: field.nativeArenaFieldDescriptor(mode), mesh: mesh(field.nativeArenaFieldMeshPlan(lattice, range, mode)) })
  }
}
const roads = []
for (let style = 0; style < 5; style++) for (let linkMask = 0; linkMask < 4; linkMask++) {
  const input = { eid: `road-${style}-${linkMask}`, typeId: 2050, style, linkMask,
    points: [{ x: -81.125, y: 27.3 }, { x: 610.375, y: 539.1875 }],
    startWidthScale: 1.4, endWidthScale: .6 }
  const plan = road.nativeRoadMeshPlan(input)
  roads.push({ input, endpointAlphas: road.nativeRoadEndpointAlphas(linkMask), style: plan.style, mesh: mesh(plan) })
}
for (const input of [
  { eid: 'authored-quad', typeId: 2050, linkMask: 3, style: 2,
    points: [], quad: [{ x: 1.1, y: 2.2 }, { x: 35.3, y: -5.5 }, { x: 4.25, y: 70.125 }, { x: 84.35, y: 91.1875 }] },
  { eid: 'zero-length', typeId: 2050, linkMask: 0, points: [{ x: 10, y: 20 }, { x: 10, y: 20 }] },
  { eid: 'zero-width', typeId: 2050, linkMask: 2, startWidthScale: 0, endWidthScale: 0,
    points: [{ x: 10, y: 20 }, { x: 300, y: 400 }] },
]) {
  const plan = road.nativeRoadMeshPlan(input)
  roads.push({ input, endpointAlphas: road.nativeRoadEndpointAlphas(input.linkMask), style: plan.style, mesh: mesh(plan) })
}
const invalidFields = [
  { x: 0, y: 0, w: 0, h: 1 }, { x: 0, y: 0, w: 1, h: -1 },
  { x: 1e40, y: 0, w: 100, h: 100 }, { x: 1e30, y: 0, w: 1e30, h: 100 },
]
for (const input of invalidFields) assert.throws(() => field.createNativeArenaFieldLattice(input))
const invalidRoads = [
  { eid: 'bad-style', style: 5, points: [{ x: 0, y: 0 }, { x: 1, y: 1 }] },
  { eid: 'missing-endpoint', points: [{ x: 0, y: 0 }] },
]
for (const input of invalidRoads) assert.throws(() => road.nativeRoadMeshPlan(input))
writeFixture('boneyard-surface-goldens.json', 'tools/website-oracle/boneyard-surface-goldens.mjs',
  `Website@${revision}/{native-arena-field,renderer/native-boneyard-surface}.ts`,
  { fields, roads, invalidFields, invalidRoads })
