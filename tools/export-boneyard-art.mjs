// Reuse the pinned Website's static painter as an asset compiler. The shipped game consumes
// only PNGs and JSON; scenery pieces remain separate so Rust owns live order and visibility.
import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const script = fileURLToPath(import.meta.url)
const root = resolve(dirname(script), '..')
const website = process.env.SOLOMON_WEBSITE_DIR || resolve(root, '..', 'Website')
const baseUrl = process.env.SOLOMON_WEBSITE_URL || 'http://127.0.0.1:4178'
const output = process.env.BONEYARD_OUTPUT
  ? resolve(process.env.BONEYARD_OUTPUT)
  : resolve(root, 'web/assets/stock/boneyard')
const bank = JSON.parse(readFileSync(resolve(root, 'web/assets/boneyards.json'), 'utf8'))
const sourceRevision = execFileSync('git', ['-C', website, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim()
const generatorHash = createHash('sha256').update(readFileSync(script)).update(sourceRevision).digest('hex')
const limit = process.env.BONEYARD_LIMIT ? Number(process.env.BONEYARD_LIMIT) : Infinity
const { chromium } = await import(pathToFileURL(
  resolve(website, 'frontend/node_modules/playwright-core/index.mjs'),
).href)
mkdirSync(resolve(output, 'images'), { recursive: true })

const surfaceDirectory = resolve(website, 'frontend/src/assets/game/boneyard/textures')
for (const name of ['arena-ground.webp', 'road.png', 'road2.png', 'road3.png', 'road4.png', 'road5.png']) {
  copyFileSync(resolve(surfaceDirectory, name), resolve(output, name))
}
const atlas = JSON.parse(readFileSync(
  resolve(website, 'frontend/src/editor/manifest/deadhawg.json'), 'utf8',
))
const sprites = {}
for (const [name, record] of [['gate-leaf', 7], ['gate-hinge', 8], ['light-glyph', 18]]) {
  const entry = atlas.entries[record]
  const filename = `${name}.png`
  copyFileSync(
    resolve(website, 'frontend/src/assets/game/boneyard', entry.file),
    resolve(output, filename),
  )
  sprites[name] = {
    image: filename,
    anchor: [entry.rect.w / 2 - entry.origin.x, entry.rect.h / 2 - entry.origin.y],
  }
}

const browser = await chromium.launch({
  executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  headless: true,
})
const page = await browser.newPage({ viewport: { width: 1600, height: 900 } })
const errors = []
page.on('pageerror', error => errors.push(error.message))
await page.route('**/__rust_art_bake', route => route.fulfill({
  contentType: 'text/html',
  body: '<!doctype html><title>Rust scenery asset compiler</title><body></body>',
}))
await page.exposeFunction('saveSceneryImage', png => {
  const bytes = Buffer.from(png.split(',')[1], 'base64')
  const hash = createHash('sha256').update(bytes).digest('hex')
  const filename = `images/${hash}.png`
  const destination = resolve(output, filename)
  if (!existsSync(destination)) writeFileSync(destination, bytes)
  return filename
})

const templates = [...bank.templates, bank.tutorial]
const index = []
try {
  await page.goto(`${baseUrl}/__rust_art_bake`, { waitUntil: 'domcontentloaded' })
  await page.evaluate(async () => {
    // Use the same optimized Pixi module URL as the Website's own transformed import.
    const source = await fetch('/src/game/renderer/boneyard-static-world.ts').then(response => response.text())
    const pixiUrl = source.match(/from\s*["']([^"']*pixi__js\.js[^"']*)["']/)?.[1]
    if (!pixiUrl) throw new Error('The Website did not expose its optimized Pixi module URL')
    const [pixi, layout, staticWorld, painter, surface, trees, arena, tutorial] = await Promise.all([
      import(pixiUrl),
      import('/src/game/renderer/boneyard-static-layout.ts'),
      import('/src/game/renderer/boneyard-static-world.ts'),
      import('/src/editor/render.ts'),
      import('/src/game/renderer/native-boneyard-surface.ts'),
      import('/src/game/renderer/boneyard-tree-occlusion.ts'),
      import('/src/game/core-kernels/boneyard-arena-transition.ts'),
      import('/src/game/core-kernels/native-tutorial.ts'),
    ])
    await layout.loadStaticPainterImages()
    window.sceneryCompiler = { pixi, layout, staticWorld, painter, surface, trees, arena, tutorial }
  })

  for (const template of templates.slice(0, limit)) {
    const hash = template.geometrySha256
    if (!/^[a-f0-9]{64}$/.test(hash)) throw new Error('A Boneyard has an invalid geometry identity')
    const manifestPath = resolve(output, `${hash}.json`)
    const previous = existsSync(manifestPath)
      ? JSON.parse(readFileSync(manifestPath, 'utf8'))
      : null
    if (previous?.generatorHash === generatorHash
      && previous.layers.every(layer => existsSync(resolve(output, layer.image)))) {
      index.push({ geometrySha256: hash, manifest: `${hash}.json`, layerCount: previous.layers.length })
      console.log(`Reused ${hash.slice(0, 12)}: ${previous.layers.length} scenery pieces`)
      continue
    }

    const manifest = await page.evaluate(async ({ template, isTutorial }) => {
      const { pixi, layout, staticWorld, painter, surface, trees, arena, tutorial } = window.sceneryCompiler
      const loaded = {
        ...template,
        choice: { id: 'asset-compiler', name: template.scene.name, source: 'default' },
        runId: 'asset-compiler',
        seed: '00000000000000000000000000000000',
      }
      const document = layout.editorDocument(loaded)
      const root = new pixi.Container()
      const cleanupBounds = isTutorial
        ? tutorial.NATIVE_TUTORIAL_CAMERA_TARGET
        : template.scene.solomonDig === null ? null
          : arena.createBoneyardArenaTransition(template.scene.bounds, template.scene.spawn).combatBounds
      // Surface geometry is exported below. Its GPU view only needs identity textures while
      // the existing painter builds the independent Canvas2D scenery pieces.
      const build = await staticWorld.buildStaticWorld(
        document,
        template.scene,
        root,
        { ground: pixi.Texture.WHITE, roads: Array(5).fill(pixi.Texture.WHITE) },
        cleanupBounds,
      )
      const mainLayers = painter.nativeBoneyardMainLayers(document)
      const proxyLayers = new Map(painter.nativeBoneyardProxyLayers(document)
        .map(layer => [layer.object.eid, layer]))
      const proxyOwners = new Map()
      const treeOwners = new Map()
      const buildingOwners = new Map()
      const wallOwners = new Map()
      for (const [id, pair] of build.treeResidents) {
        proxyOwners.set(pair.proxy, { id, ...proxyLayers.get(id) })
        treeOwners.set(pair.main, id)
        treeOwners.set(pair.proxy, id)
      }
      for (const [id, pair] of build.buildingResidents) {
        proxyOwners.set(pair.roof, { id, ...proxyLayers.get(id) })
        buildingOwners.set(pair.main, pair)
        buildingOwners.set(pair.roof, pair)
      }
      for (const wall of build.wallResidents.values()) {
        wallOwners.set(wall.resident, wall)
      }

      const canvas = window.document.createElement('canvas')
      const layers = []
      for (const resident of build.residents) {
        canvas.width = resident.w
        canvas.height = resident.h
        const context = canvas.getContext('2d')
        if (!context) throw new Error('Scenery export requires Canvas2D')
        context.putImageData(new ImageData(resident.pixels, resident.w, resident.h), 0, 0)
        const image = await window.saveSceneryImage(canvas.toDataURL('image/png'))
        const main = resident.mainLayerIndex === null ? null : mainLayers[resident.mainLayerIndex]
        const proxy = proxyOwners.get(resident)
        const source = main?.kind === 'object' ? main.object : main?.fence
        const building = buildingOwners.get(resident)
        const wall = wallOwners.get(resident)
        let surfaceLighting = null
        if (building) {
          surfaceLighting = { kind: 'building', samplePoints: building.samplePoints }
        } else if (wall) {
          surfaceLighting = {
            kind: 'wall', start: wall.start, end: wall.end,
            vertexWeights: Array.from(wall.vertexWeights),
          }
        }
        layers.push({
          image,
          bounds: { x: resident.x, y: resident.y, width: resident.w, height: resident.h },
          band: main ? 'main' : proxy ? 'proxy' : 'base',
          worldY: main?.worldY ?? proxy?.worldY ?? 0,
          sortBias: main?.sortBias ?? proxy?.sortBias ?? 0,
          sourceOrder: main?.sourceOrder ?? proxy?.sourceOrder ?? layers.length,
          insertsProxy: main ? Boolean(layout.nativeStaticProxyInsertions(main)) : false,
          sourceKey: resident.cleanupSourceKey,
          objectId: main?.kind === 'object' ? main.object.eid : proxy?.id ?? null,
          typeId: source?.typeId ?? null,
          treeId: treeOwners.get(resident) ?? null,
          initialAlpha: resident.sprite.alpha,
          shadow: resident.shadowCaster,
          lightPosition: main?.pos ?? proxy?.object?.pos ?? null,
          surfaceLighting,
        })
      }
      const mesh = plan => ({
        positions: Array.from(plan.positions),
        uvs: Array.from(plan.uvs),
        colors: Array.from(plan.colors),
        indices: Array.from(plan.indices),
      })
      const result = {
        sourceSha256: template.sourceSha256,
        geometrySha256: template.geometrySha256,
        bounds: template.scene.bounds,
        layers,
        gates: mainLayers.filter(layout.isMovingGateBody).map(layer => ({
          fenceEid: layer.fence.eid,
          side: layer.pieceIndex,
          sourceOrder: layer.sourceOrder,
          sortBias: layer.sortBias,
        })),
        treeInputs: build.treeInputs.map(input => ({
          ...input,
          bounds: trees.NATIVE_TREE_OCCLUSION_BOUNDS[input.secondaryVariant],
          polygon: trees.NATIVE_TREE_OCCLUSION_POLYGONS[input.secondaryVariant],
        })),
        ground: mesh(surface.webArenaGroundMeshPlan(template.scene.bounds)),
        roads: template.scene.roads.map(road => {
          const plan = surface.nativeRoadMeshPlan(road)
          return { eid: road.eid, style: plan.style, ...mesh(plan) }
        }),
      }
      // Reuse the actual Source lifecycle, including the post-road Canvas repaint. Merely
      // hiding off-camera main residents leaves retired terrain/compact art in the base.
      build.applyOffCameraCleanup()
      const baseImages = []
      for (const [index, resident] of build.residents.entries()) {
        if (cleanupBounds === null || resident.cleanupSourceKey !== null
          || resident.mainLayerIndex !== null || proxyOwners.has(resident)) continue
        canvas.width = resident.w
        canvas.height = resident.h
        const context = canvas.getContext('2d')
        if (!context) throw new Error('Scenery cleanup export requires Canvas2D')
        context.putImageData(new ImageData(resident.pixels, resident.w, resident.h), 0, 0)
        baseImages.push({ layer: index, image: await window.saveSceneryImage(canvas.toDataURL('image/png')) })
      }
      // NativeBoneyardSurfaceView retains ground at child zero and its authored road root
      // at child one. Read their actual renderability after the Source cleanup method.
      const roadViews = build.surface.container.children[1].children
      if (roadViews.length !== template.scene.roads.length) throw new Error('Source road order changed')
      result.cleanup = {
        bounds: cleanupBounds,
        retiredLayers: build.residents.flatMap((resident, index) => resident.sprite.renderable ? [] : [index]),
        retiredRoads: roadViews.flatMap((road, index) => road.renderable ? [] : [index]),
        baseImages,
        retiredSourceCount: build.retiredStaticSourceCount,
      }
      build.surface.destroy()
      for (const resident of build.residents) staticWorld.destroyResidentTexture(resident)
      root.destroy()
      return result
    }, { template, isTutorial: template === bank.tutorial })
    manifest.generatorHash = generatorHash
    manifest.sourceRevision = sourceRevision
    manifest.sprites = sprites
    writeFileSync(manifestPath, `${JSON.stringify(manifest)}\n`)
    index.push({ geometrySha256: hash, manifest: `${hash}.json`, layerCount: manifest.layers.length })
    console.log(`Compiled ${hash.slice(0, 12)}: ${manifest.layers.length} scenery pieces, ${manifest.roads.length} roads`)
  }
  writeFileSync(resolve(output, 'index.json'), `${JSON.stringify({ generatorHash, maps: index }, null, 2)}\n`)
  if (errors.length) throw new Error(errors.join('\n'))
  console.log(`Scenery export complete: ${index.length} maps`)
} finally {
  await browser.close()
}
