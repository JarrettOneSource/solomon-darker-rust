import { loadKernel, writeFixture } from './oracle.mjs'

const mobile = await loadKernel('mobile-ui-layout.ts')
const fresh = () => structuredClone(mobile.mobileUiLayoutDocument(mobile.DEFAULT_MOBILE_UI_LAYOUT))
const cases = []
const add = (name, document) => cases.push({name, document,
  expected: mobile.mobileUiLayoutFromDocument(document),
  local: mobile.readMobileUiLayoutState({getItem: () => JSON.stringify(document)}),
})
add('current', fresh())
add('null', null)
add('array', [])
for (const version of [1, 2.5, 3, '2', null, true]) { const value = fresh(); value.version = version; add(`version ${version}`, value) }
const legacy = fresh(); legacy.version = 1; delete legacy.elements.meters; add('previous local seventeen-element schema', legacy)
const extra = fresh(); extra.extra = 1; add('unknown root field', extra)
const missing = fresh(); delete missing.elements.meters; add('missing meters', missing)
const unknown = fresh(); unknown.elements.other = unknown.elements.pause; add('unknown element', unknown)
for (const field of ['rotation', 'scale', 'x', 'y']) {
  const missing = fresh(); delete missing.elements.pause[field]; add(`missing transform ${field}`, missing)
  for (const value of [null, true, '1']) { const doc = fresh(); doc.elements.pause[field] = value; add(`${field} nonnumber ${value}`, doc) }
}
const nested = fresh(); nested.elements.pause.other = 1; add('unknown transform field', nested)
for (const [field, values] of Object.entries({x: [-0.1, 0, 100, 100.1], y: [-0.1, 0, 100, 100.1], scale: [0.399, 0.4, 3, 3.001], rotation: [-180.1, -180, 180, 180.1]})) {
  for (const value of values) { const doc = fresh(); doc.elements.pause[field] = value; add(`${field} boundary ${value}`, doc) }
}
const geometries = []
for (const [width, height] of [[896,414], [1600,900], [390,844], [844,390], [2560,1080]]) {
  for (const scale of [0.5, 0.75, 1, 1.25, 1.5, 2]) geometries.push({width, height, scale, expected: mobile.defaultMobileUiGeometry(width,height,scale)})
}
const normalized = []
for (const rotation of [-1080,-540,-360,-180,-0,0,180,360,540,1080,NaN,Infinity]) {
  const transform = {rotation, scale: 1, x: 50, y: 50}
  normalized.push({transform, expected: mobile.mobileUiLayoutWith(mobile.DEFAULT_MOBILE_UI_LAYOUT, 'pause', transform).pause})
}
for (const [field, values] of Object.entries({x: [-1,-0,100,101,NaN], y: [-1,-0,100,101,Infinity], scale: [-1,0.399,0.4,3,3.001,NaN]})) {
  for (const value of values) { const transform = {rotation:0,scale:1,x:50,y:50,[field]:value}; normalized.push({transform,expected:mobile.mobileUiLayoutWith(mobile.DEFAULT_MOBILE_UI_LAYOUT,'pause',transform).pause}) }
}
writeFixture('mobile-ui-document-goldens.json', 'tools/website-oracle/mobile-ui-document-goldens.mjs',
  'Website/frontend/src/game/mobile-ui-layout.ts', {cases, geometries, normalized})
