import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { gameDir, websiteDir, writeFixture } from './oracle.mjs'

// Execute the source method and Pixi's actual stable, in-place child sorter.
const source = readFileSync(resolve(gameDir, 'renderer/primary-spell-world-view.ts'), 'utf8')
const body = source.match(/promoteOwnerOverlays\(ownerDepth:[^\n]+\{\n([\s\S]*?)\n  }\n\n  get count/)[1]
const promote = new Function('ownerDepth', body)
const { sortMixin } = await import(pathToFileURL(resolve(websiteDir,
  'frontend/node_modules/pixi.js/lib/scene/container/container-mixins/sortMixin.mjs')).href)
const parent = { children: [], sortDirty: true }
const views = new Map()
let creationOrder = 0
const frames = []
for (const input of [
  { name: 'creation order differs from IDs', live: [3, 2], rows: [[3, 1], [2, 2], [100, 3], [101, 4]] },
  { name: 'crossing changes retained order', live: [3, 2], rows: [[100, 1], [2, 2], [3, 3], [101, 4]] },
  { name: 'equal promotion retains crossing', live: [3, 2], rows: [[3, 1], [2, 2], [100, 3], [101, 4]] },
  { name: 'reordered source array keeps retained order', live: [2, 3], rows: [[3, 1], [2, 2], [100, 3], [101, 4]] },
  { name: 'new view appends on a tie', live: [1, 3, 2], rows: [[1, 1], [3, 2], [2, 3], [100, 4], [101, 5]] },
  { name: 'empty view retains its old depth', live: [1, 3, 2], rows: [[1, 1], [3, 2], [100, 3], [101, 4]] },
  { name: 'empty view returns without recreating', live: [1, 3, 2], rows: [[1, 1], [3, 2], [2, 3], [100, 4], [101, 5]] },
  { name: 'retired view is removed', live: [1, 2], rows: [[1, 1], [2, 2], [100, 3], [101, 4]] },
  { name: 'reused ID is a new view', live: [1, 2, 3], rows: [[3, 1], [1, 2], [2, 3], [100, 4], [101, 5]] },
]) {
  for (const [id, view] of views) if (!input.live.includes(id)) {
    parent.children.splice(parent.children.indexOf(view.container), 1)
    views.delete(id)
  }
  for (const id of input.live) if (!views.has(id)) {
    const container = { id, _zIndex: 0, get zIndex() { return this._zIndex },
      set zIndex(value) { this._zIndex = value } }
    const view = { container, creationOrder: creationOrder++, active: true,
      painterRoots() { return this.active ? [{ container, overlayOwnerId: 'owner' }] : [] } }
    parent.children.push(container)
    views.set(id, view)
  }
  const depths = new Map(input.rows)
  for (const [id, view] of views) {
    view.active = depths.has(id)
    if (view.active) view.container.zIndex = depths.get(id)
  }
  promote.call({ views }, () => depths.get(100))
  parent.sortDirty = true
  sortMixin.sortChildren.call(parent)
  const children = parent.children.filter(child => depths.has(child.id))
  const final = input.rows.map(([id, depth]) => ({ id, depth: views.get(id)?.container.zIndex ?? depth }))
  final.sort((a, b) => a.depth - b.depth || children.findIndex(c => c.id === a.id) - children.findIndex(c => c.id === b.id))
  frames.push({ ...input, liveViews: input.live.map(id => [id, views.get(id).creationOrder]),
    expected: final.map(row => row.id) })
}
writeFixture('painter-overlay-goldens.json', 'tools/website-oracle/painter-overlay-goldens.mjs',
  'PrimarySpellWorldView.promoteOwnerOverlays and installed Pixi sortMixin', { frames })
