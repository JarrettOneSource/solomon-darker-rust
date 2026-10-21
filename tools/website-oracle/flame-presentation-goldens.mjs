// Exact current Flame plans and Region split membership. Source remains unmodified.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const revision = execFileSync('git', ['-C', websiteDir, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim()
assert.equal(revision, 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83')
const visual = await loadKernel('renderer/primary-spell-weld-native.ts')
const split = await loadKernel('renderer/primary-spell-weld-view.ts')
const birth = JSON.parse(readFileSync(resolve(fixturesDir, 'flame-birth-goldens.json'), 'utf8'))
const point = value => value === null ? null : [value.x, value.y]
const pairs = values => Array.from({ length: values.length / 2 }, (_, index) => values.slice(index * 2, index * 2 + 2))
const sprite = draw => ({ alpha: draw.alpha, atlas: draw.atlas, blend: draw.blend,
  diffuse: draw.textureColor === 'diffuse', matrix: draw.matrix, offset: point(draw.offset),
  record: draw.record, rotation: draw.rotationRadians, scale: [draw.scaleX, draw.scaleY], tint: draw.tint })
const planRow = plan => ({ lines: plan.lines.map(({ role: _role, ...draw }) =>
  ({ ...draw, start: point(draw.start), end: point(draw.end) })),
  meshes: plan.meshes.map(({ role: _role, ...draw }) =>
    ({ ...draw, uvs: pairs(draw.uvs), vertices: pairs(draw.vertices) })),
  position: point(plan.position), regionLightPoint: point(plan.regionLightPoint),
  sortBias: plan.sortBias, sprites: plan.sprites.map(sprite),
  underlays: (plan.underlays ?? []).map(sprite), worldY: plan.worldY,
  ...(plan.splitBounds === undefined ? {} : { splitBounds: plan.splitBounds }) })
const cases = []
function add(name, state, age, frame) {
  const input = { ...state, ageTicks: age }
  cases.push({ name, state, visuals: [{ tick: 0, age, presentationFrame: frame,
    puppetHit: false, plans: [planRow(visual.nativeWeldVisualPlan(input, frame))] }] })
}
for (const value of birth.cases) add(value.name, value.channel, 0, 17)
add('last-fractional-birth-draw', birth.cases[4].channel, .999, 17)
add('no-second-channel-draw', birth.cases[4].channel, 1, 17)
add('legacy-channel-suppressed', { ...birth.cases[4].channel, flameVisual: { kind: 'legacy-suppressed' } }, 0, 17)
for (const value of birth.fades) {
  for (const frame of [0, 19]) add(`${value.input.variant}-${value.input.enhancedEffects}-frame-${frame}`, value.frames[0].actor, 0, frame)
}
add('legacy-fade-suppressed', { ...birth.fades[0].frames[0].actor, flameVisual: { kind: 'legacy-suppressed' } }, 0, 0)
add('expired-fade-suppressed', { ...birth.fades[0].frames[0].actor, alpha: 0 }, 0, 0)
const bounds = { x: -40.5, y: -301.25, width: 800, height: 403.5 }
const bands = []
for (const enhancedEffects of [true, false]) {
  for (const regionView of [null, { x: 10, y: -250, width: 500, height: 200 },
    { x: 10001, y: 999, width: 0, height: 0 }]) {
    bands.push({ bounds, enhancedEffects, regionView,
      result: split.buildNativeFlameLashSplitBands('flame:1', bounds, enhancedEffects, regionView) })
  }
}
for (const value of [{ x: 0, y: 0, width: 0, height: 0 }, { x: 0, y: .1, width: 10, height: 25.0001 }]) {
  bands.push({ bounds: value, enhancedEffects: true, regionView: null,
    result: split.buildNativeFlameLashSplitBands('flame:1', value, true) })
}
const errors = [{ x: 0, y: 0, width: -1, height: 1 },
  { x: 0, y: 2 ** 30, width: 1, height: 256 }, { x: 0, y: 0, width: 1, height: 25 * 32769 }]
for (const value of errors) assert.throws(() => split.buildNativeFlameLashSplitBands('flame:1', value, true), RangeError)
writeFixture('flame-presentation-goldens.json', 'tools/website-oracle/flame-presentation-goldens.mjs',
  `Website@${revision}/renderer/primary-spell-weld-{native,view}.ts`, { cases, bands, errors })
