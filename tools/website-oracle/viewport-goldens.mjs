import { loadKernel, writeFixture } from './oracle.mjs'

const viewport = await loadKernel('renderer/game-viewport.ts')
const cases = []
for (const size of [[1600, 900], [1280, 720], [1920, 1080], [3440, 1440],
  [390, 844], [844, 390], [0, 900], [-1, 10]]) {
  for (const world of [{ width: 2000, height: 1024 }, { width: 1024, height: 1024 },
    { width: 4000, height: 4000 }]) {
    for (const zoom of [1.08, 1.35, 1.8]) {
      cases.push({ size, world, zoom,
        result: viewport.boundedGameViewportLayout(...size, world, zoom) })
    }
  }
}
writeFixture('viewport-goldens.json', 'tools/website-oracle/viewport-goldens.mjs',
  'Website renderer/game-viewport.ts', { cases })
