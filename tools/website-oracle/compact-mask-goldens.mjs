// Evaluate the pinned view's actual admission code without a Pixi or DOM dependency.
import { readFileSync } from 'node:fs'
import { stripTypeScriptTypes } from 'node:module'
import { resolve } from 'node:path'
import { gameDir, loadKernel, writeFixture } from './oracle.mjs'

const source = readFileSync(resolve(gameDir, 'renderer/native-compact-mask-view.ts'), 'utf8')
const manifest = JSON.parse(readFileSync(resolve(gameDir, '../editor/manifest/deadhawg.json'), 'utf8'))
const rng = await loadKernel('core-kernels/native-rng.ts')
const environment = await loadKernel('renderer/boneyard-environment-light-plan.ts')
const record = (_atlas, entry) => ({ width: manifest.entries[entry].rect.w, height: manifest.entries[entry].rect.h })
const gridCode = source.slice(source.indexOf('function compactGridCells('))
const grid = new Function(`${stripTypeScriptTypes(gridCode)};return compactGridCells`)()
const intersectsCode = source.slice(source.indexOf('function intersects('), source.indexOf('/** MagicGrid'))
const intersects = new Function('nativeEnemySpriteRecord', `${stripTypeScriptTypes(intersectsCode)};return intersects`)(record)
const authoredCode = source.slice(source.indexOf('this.authored ='), source.indexOf('\n  }\n\n  update('))
const authored = new Function('scene', `${authoredCode};return this.authored`)
const dynamicCode = source.slice(source.indexOf('const dynamic ='), source.indexOf('    const liveGround'))
const masksForFrame = new Function('remains', 'bounds', 'intersects', `${stripTypeScriptTypes(dynamicCode)};return masks`)
const queryCode = source.slice(source.indexOf('const query ='), source.indexOf('      let target ='))
const candidates = new Function('player', 'masks', 'compactGridCells', 'nativeEnemySpriteRecord',
  `${queryCode};return candidates`)
const alphaCode = source.slice(source.indexOf('const sample ='), source.indexOf('      slot += 1', source.indexOf('const sample =')))
const sampleAlpha = new Function('presentationFrame', 'slot', 'createNativeRng', 'drawNativeFloat',
  'target', 'renderer', 'candidates', 'player', alphaCode)

const scene = { bounds: { x: 0, y: 0, w: 1033.7, h: 759.3 }, sprites: [
  { eid: 'ignored', atlasEntry: 24, pos: { x: 100, y: 100 }, s0: 0, s1: 1, flags: 0 },
  ...[25, 26, 27, 28, 29].map((atlasEntry, index) => ({
    eid: `authored:${index}`, atlasEntry,
    pos: [{ x: -50.125, y: 10.75 }, { x: 130.3, y: 155.9 }, { x: 350, y: 200 },
      { x: 800.7, y: 640.5 }, { x: 1100, y: 800 }][index],
    s0: 31.25 * index, s1: .6 + index * .4, flags: index % 2,
  })),
] }
const remains = [
  { id: 66, state: { decal: { entry: 140, position: { x: 499.99, y: 400 }, rotationDeg: 67, scale: 1.3, alpha: .4 } } },
  { id: 67, state: { decal: null } },
  { id: 68, state: { decal: { entry: 142, position: { x: 30, y: 700 }, rotationDeg: -91, scale: 2, alpha: .75 } } },
]
const state = {
  authored: authored.call({}, scene),
  maximumColumn: Math.trunc(Math.trunc(scene.bounds.w) / 50 + .5),
  maximumRow: Math.trunc(Math.trunc(scene.bounds.h) / 50 + .5),
}
const boundaryPlayers = [-257.1, -256.9, -256, -.9, 0, 49.99999, 50, 499.99999, 500, 1200].map(x => ({ x, y: 210.5 }))
const requests = [
  { players: [{ x: 2000, y: 2000 }, { x: 210, y: 330 }, { x: 720, y: 600 }], frame: 100.9 },
  { players: [{ x: 720, y: 600 }, { x: 210, y: 330 }], frame: 101.8 },
  { players: boundaryPlayers, frame: 4294967299 },
  { players: [{ x: 210, y: 330 }], frame: -1 },
  { players: [{ x: 210, y: 330 }], frame: 0, bounds: { x: 500, y: 350, w: 400, h: 400 } },
  { players: [{ x: 210, y: 330 }], frame: 5, bounds: { x: 2000, y: 2000, w: 400, h: 400 } },
  { players: [], frame: 6 },
]
const maskRow = mask => [mask.id, mask.entry, mask.position.x, mask.position.y,
  mask.rotation, mask.scaleX, mask.scaleY, mask.alpha, mask.dynamic]
const cases = requests.map(request => {
  const bounds = request.bounds ?? { x: -80, y: -20, w: 900, h: 800 }
  const masks = masksForFrame.call(state, remains, bounds, intersects)
  const players = request.players.map((position, slot) => {
    const admitted = candidates.call(state, { position }, masks, grid, record)
    let alpha = 0
    if (admitted.length > 0) {
      const target = { update: (_renderer, _masks, _position, value) => { alpha = value } }
      sampleAlpha(request.frame, slot, rng.createNativeRng, rng.drawNativeFloat, target, null, admitted, { position })
    }
    return [slot, position.x, position.y, alpha, admitted.map(mask => mask.id)]
  })
  return { players: request.players, frame: request.frame, bounds, plan: { masks: masks.map(maskRow), players } }
})
const sizes = Object.fromEntries([139, 140, 141, 142, 143].map(entry => [entry, record('DeadHawg', entry)]))
const apertures = [[0, 0], [0, 1], [60.25, 2], [1200, 0], [123456.75, 3]].map(([now, slot]) => (
  [now, slot, environment.nativeDirectEnvironmentLightAlpha(now, slot)]
))
writeFixture('compact-mask-goldens.json', 'compact-mask-goldens.mjs', [
  'renderer/native-compact-mask-view.ts', 'renderer/boneyard-environment-light-plan.ts',
  'editor/manifest/deadhawg.json',
], { scene, remains, sizes, cases, apertures })
