import { loadKernel, writeFixture } from './oracle.mjs'

const mobile = await loadKernel('mobile-ui-layout.ts')
const pages = [{ width: 896, height: 414 }, { width: 1600, height: 900 }]
const sizes = [{ width: 100, height: 100 }, { width: 200, height: 40 }]
const siblingRects = page => [
  { left: 32, right: 96, centerX: 64, top: 32, bottom: 96, centerY: 64 },
  { left: page.width / 2 - 50, right: page.width / 2 + 50, centerX: page.width / 2,
    top: page.height / 2 - 50, bottom: page.height / 2 + 50, centerY: page.height / 2 },
]
const frames = []
for (const page of pages) {
  for (const naturalSize of sizes) {
    for (const rotation of [0, 37, 90]) {
      for (const [x, y, scale] of [[1, 1, 0.4], [50, 50, 1], [99, 99, 3]]) {
        const transform = { x, y, scale, rotation }
        const siblings = siblingRects(page)
        const threshold = mobile.MOBILE_UI_SNAP_THRESHOLD
        frames.push({
          page, naturalSize, transform, siblings, threshold,
          constrained: mobile.constrainMobileUiTransform(transform, naturalSize, page),
          frame: mobile.mobileUiElementSnapRect(transform, naturalSize, page),
          move: mobile.snapMobileUiMove(transform, naturalSize, page, siblings, threshold),
          scale: mobile.snapMobileUiScale(transform, naturalSize, page, siblings, threshold),
        })
      }
    }
  }
}
// Page/element/grid ties and a zero-threshold exact-edge snap have visible guide results.
for (const threshold of [0, 1, 6]) {
  const page = pages[0]
  const naturalSize = sizes[0]
  const transform = { x: 50, y: 50, scale: 1, rotation: 0 }
  const siblings = [...siblingRects(page), ...siblingRects(page)]
  frames.push({
    page, naturalSize, transform, siblings, threshold,
    constrained: mobile.constrainMobileUiTransform(transform, naturalSize, page),
    frame: mobile.mobileUiElementSnapRect(transform, naturalSize, page),
    move: mobile.snapMobileUiMove(transform, naturalSize, page, siblings, threshold),
    scale: mobile.snapMobileUiScale(transform, naturalSize, page, siblings, threshold),
  })
}
const resizes = []
for (const page of pages) {
  for (const naturalSize of sizes) {
    for (const rotation of [0, 37, 90, -135]) {
      for (const handle of mobile.MOBILE_UI_RESIZE_HANDLES) {
        for (const delta of [{ x: -300, y: -100 }, { x: 0, y: 0 }, { x: 200, y: 120 }]) {
          const transform = { x: 50, y: 50, scale: 1, rotation }
          const initialPointer = { x: page.width / 2, y: page.height / 2 }
          const currentPointer = { x: initialPointer.x + delta.x, y: initialPointer.y + delta.y }
          const siblings = siblingRects(page)
          const threshold = mobile.MOBILE_UI_SNAP_THRESHOLD
          resizes.push({
            page, naturalSize, transform, handle, initialPointer, currentPointer, siblings, threshold,
            resized: mobile.mobileUiResizeTransform(transform, naturalSize, page, handle, initialPointer, currentPointer),
            snapped: mobile.snapMobileUiResize(transform, naturalSize, page, handle,
              initialPointer, currentPointer, siblings, threshold),
          })
        }
      }
    }
  }
}
const pinches = []
for (const [initial, from, to] of [
  [1, 100, 200], [0.4, 100, 1], [3, 100, 500], [1, 0, 100],
  [1, -1, 100], [1, 100, -1], [NaN, 10, 20], [2, Infinity, 100], [2, 100, NaN],
]) {
  pinches.push({ initial, from, to,
    scale: mobile.mobileUiElementPinchScale(initial, from, to),
    zoom: mobile.mobileUiPagePinchZoom(initial, from, to),
  })
}
const rotations = []
for (const initial of [-181, -2.5, 0, 2.5, 180]) {
  for (const current of [-0.2, 0, 0.2]) {
    for (const snap of [false, true]) {
      rotations.push({ initial, from: 0, to: current, snap,
        expected: mobile.mobileUiElementRotation(initial, 0, current, snap) })
    }
  }
}
const pageSizes = []
for (const [width, height] of [[390,844],[844,390],[0,100],[100,0],[NaN,400],[400,Infinity]]) {
  for (const coarse of [false, true]) {
    pageSizes.push({ width, height, coarse, expected: mobile.mobileUiEditorPageSize(width,height,coarse) })
  }
}
const belts = []
for (const entries of [
  Array(8).fill(null),
  [{ kind:'skill',skillId:8 },null,null,{ kind:'health-potion' },{ kind:'mana-potion' },null,null,null],
  [{ kind:'health-potion' },{ kind:'health-potion' },{ kind:'mana-potion' },{ kind:'mana-potion' },null,null,null,null],
  [null,null,null,null,null,null,{ kind:'mana-potion' },{ kind:'health-potion' }],
]) {
  belts.push({ entries, expected: entries.map((_,slot) => mobile.mobileUiBeltElementId(entries,slot)) })
}
writeFixture('mobile-editor-goldens.json', 'tools/website-oracle/mobile-editor-goldens.mjs',
  'Website/frontend/src/game/mobile-ui-layout.ts', { frames, resizes, pinches, rotations, pageSizes, belts })
