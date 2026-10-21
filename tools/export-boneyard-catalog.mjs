// Scene-independent original glyphs and immutable recovered shape tables. The
// shipped Rust compiler owns all placements; this tool never compiles a scene.
import { execFileSync } from 'node:child_process'
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const website = process.env.SOLOMON_WEBSITE_DIR || resolve(root, '..', 'website-rust-current')
const output = process.env.BONEYARD_OUTPUT || resolve(root, 'web/assets/stock/boneyard')
const source = resolve(website, 'frontend/src')
const load = name => import(pathToFileURL(resolve(source, 'game', name)).href)
const [shapes, trees, treeShadows] = await Promise.all([
  load('renderer/boneyard-native-shadow-shapes.ts'),
  load('core-kernels/native-tree-geometry.ts'),
  load('renderer/boneyard-complex-shadows.ts'),
])
const manifest = JSON.parse(readFileSync(resolve(source, 'editor/manifest/deadhawg.json'), 'utf8'))
mkdirSync(resolve(output, 'scenery/deadhawg'), { recursive: true })
const records = manifest.entries.map(entry => {
  if (entry.empty || !entry.file) return null
  const file = `scenery/${entry.file}`
  copyFileSync(resolve(source, 'assets/game/boneyard', entry.file), resolve(output, file))
  return { file, w: entry.rect.w, h: entry.rect.h,
    anchorX: entry.rect.w / 2 - entry.origin.x,
    anchorY: entry.rect.h / 2 - entry.origin.y }
})
const outlines = (count, project) => Array.from({ length: count }, (_, variant) => project(variant))
const catalog = {
  sourceRevision: execFileSync('git', ['-C', website, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
  records,
  shadows: {
    Tree: outlines(15, treeShadows.nativeBoneyardTreeComplexShadowOutline),
    Gravestone: outlines(17, shapes.nativeGravestoneShadowOutline),
    Monument: outlines(21, shapes.nativeMonumentShadowOutline),
    Building: outlines(4, shapes.nativeBuildingShadowOutline),
    Goodie: outlines(1, shapes.nativeGoodieShadowOutline),
    Fencepost: outlines(7, variant => shapes.nativeFencepostShadowOutline(variant, 0)),
    Railpost: outlines(28, variant => shapes.nativeFencepostShadowOutline(variant, 1)),
  },
  treeBounds: trees.NATIVE_TREE_OCCLUSION_BOUNDS,
  treePolygons: trees.NATIVE_TREE_OCCLUSION_POLYGONS,
}
copyFileSync(resolve(source, 'assets/game/boneyard-combat-atlas-2.png'), resolve(output, 'ground.png'))
for (const file of ['road.png', 'road2.png', 'road3.png', 'road4.png', 'road5.png', 'river.png', 'rise.png', 'fencegrate.png']) {
  copyFileSync(resolve(source, 'assets/game/boneyard/textures', file), resolve(output, file))
}
writeFileSync(resolve(output, 'catalog.json'), `${JSON.stringify(catalog)}\n`)
console.log(`Retained ${records.filter(Boolean).length} original scenery glyphs and native shape tables in ${output}`)
