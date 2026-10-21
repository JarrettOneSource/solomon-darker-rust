import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const placement = await loadKernel('../editor/native-render-plan.ts')
const fences = await loadKernel('../editor/native-fence-geometry.ts')
const posts = await loadKernel('core-kernels/boneyard-fence-posts.ts')
const manifest = JSON.parse(readFileSync(resolve(websiteDir, 'frontend/src/editor/manifest/deadhawg.json'), 'utf8'))
const obj = (eid, typeId, variant, x, y, extras = {}) => ({ eid, typeId, variant, pos: { x, y }, ...extras })
const fence = (eid, code, start, end, extras = {}) => ({ eid, typeId: 2031,
  segmentCode: code, points: [start, end], ...extras })
const a = { x: -100.25, y: 173.875 }, b = { x: 277.125, y: 401.5 }, c = { x: 643.3, y: -80.125 }
const scenes = [
  { objects: [], sprites: [], fences: [] },
  { objects: [obj('tree', 2001, 0, 300, 310), obj('stone', 2029, 3, 400, 410),
      obj('building', 2040, 2, 500, 510), obj('statue', 2009, 20, 600, 610),
      obj('chest', 2061, 0, 700, 710), obj('unpainted', 9999, 0, 800, 810)],
    sprites: [{ eid: 'grass', atlasEntry: 26, pos: a, s0: 17.5, s1: .7, s2: .4, flags: 1 },
      { eid: 'explicit', atlasEntry: 0, deadHawgEntry: 271, pos: b, s0: 0, s1: 1, s2: 1, flags: 0 }],
    fences: [fence('grate', 0, a, b), fence('rail', 4, b, c, { startPostVariant: 17, endPostVariant: 3 }),
      fence('broken', 1, c, a, { startPostVariant: 0xffffffff, endPostVariant: 6 }),
      fence('gate', 2, { x: 120, y: 290 }, { x: 510, y: 340 }),
      fence('wall', 3, a, { x: -300, y: 180 })] },
  { objects: [obj('custom', 9999, 0, -25.25, -40.1, { atlasEntry: 90, sortBias: -8.5 }),
      obj('override', 2001, 1, 20, 40, { atlasEntries: [265], secondaryAtlasEntry: 0 }),
      obj('no-canopy', 2001, 5, 30, 50, { secondaryVisible: false }),
      obj('dead-tree', 2001, 6, 40, 60), obj('custom-roof', 2040, 0, 50, 70, { atlasEntries: [150, 154] })],
    sprites: [], fences: [fence('short-gate', 2, a, { x: a.x + 29, y: a.y })] },
  { objects: [], sprites: [], fences: [fence('zero-broken', 1, a, a),
      fence('backwards', 1, b, a, { startPostVariant: 2, endPostVariant: 5 })] },
]
function half(value) {
  return value ? { corners: [value.p0, value.p1, value.p2, value.p3], root: value.root,
    seed: value.seed, shadowStart: value.shadowStart, shadowEnd: value.shadowEnd,
    shadowStep: value.shadowStep, shadowCount: value.shadowCount } : null
}
const cases = scenes.map((scene, index) => {
  const doc = { meta: { name: `authored-${index}`, bounds: { x: -500, y: -500, w: 2000, h: 2000 } },
    ...scene, roads: [], terrain: [], opaque: [], hasTimeline: false }
  const plan = placement.buildNativeRenderPlan(doc)
  const postRows = posts.nativeBoneyardFencePosts(scene.fences)
  const layer = value => ({
    source: value.kind === 'object' ? { kind: 'object', index: scene.objects.indexOf(value.object), entry: value.atlasEntry }
      : value.part === 'post' ? { kind: 'post', index: value.pieceIndex,
        entry: (value.postStyle === 1 ? 320 : 36) + value.postVariant }
        : { kind: 'body', index: scene.fences.indexOf(value.fence), piece: value.pieceIndex, broken: half(value.brokenHalf) },
    position: value.pos, worldY: value.worldY, sortBias: value.sortBias, sourceOrder: value.sourceOrder,
  })
  return { input: doc,
    underlays: plan.underlays.map(row => [scene.objects.indexOf(row.object), row.atlasEntry]),
    compact: plan.compact.map(row => [scene.sprites.indexOf(row.sprite), row.atlasEntry]),
    main: plan.shadows.filter(row => row.kind === 'object' || (row.fence.segmentCode ?? row.fence.style ?? 0) !== 3).map(layer),
    walls: plan.shadows.filter(row => row.kind === 'fence' && (row.fence.segmentCode ?? row.fence.style ?? 0) === 3).map(layer),
    proxies: plan.proxies.map(layer),
    posts: postRows.map(row => ({ fenceIndex: scene.fences.indexOf(row.fence), position: row.pos,
      variant: row.postVariant, style: row.postStyle })),
    grates: scene.fences.map(row => fences.nativeFenceGrate(row.points)),
  }
})
writeFixture('boneyard-scenery-goldens.json', 'tools/website-oracle/boneyard-scenery-goldens.mjs',
  'Website@fa08ae24/editor/{native-render-plan,native-fence-geometry}.ts', {
    emptyEntries: manifest.entries.filter(entry => entry.empty).map(entry => entry.id), cases,
  })
