// Current Source's retained native record-25 builder, without Pixi or actor mocks.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const revision = execFileSync('git', ['-C', websiteDir, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim()
assert.equal(revision, 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83')
const source = await loadKernel('core-kernels/native-weld-flame-ribbon.ts')
const point = (x, y) => ({ x, y })
const inputs = [
  { start: point(10, 20), control: point(10, 20), end: point(10, 20), offset: point(0, 0) },
  { start: point(0, 0), control: point(30, 0), end: point(50, 0), offset: point(0, 5) },
  { start: point(0, 0), control: point(30, 0), end: point(60, 0), offset: point(0, 0) },
  { start: point(0, 0), control: point(300, 0), end: point(800, 0), offset: point(0, -35) },
  { start: point(123.25, 400.5), control: point(380.1, 5.75), end: point(710.6, 201.45), offset: point(-3.25, -35) },
  { start: point(-180.75, 201.125), control: point(80, -260), end: point(-120, 50), offset: point(5, 35) },
]
const cases = []
for (const [index, geometryInput] of inputs.entries()) {
  for (const enhancedEffects of [false, true]) {
    const input = { ...geometryInput, enhancedEffects, weak: index % 2 === 1,
      width: index % 2 === 1 ? .75 : 1,
      phaseDegrees: index === 4 ? 2147483648 : source.nativeFlameLashBirthPhase(index * 113),
      shortWidthFactor: .875 }
    const geometry = source.nativeFlameLashFactoryGeometry(input)
    const ribbon = source.buildNativeFlameLashRibbon(input)
    cases.push({ input, geometry, pathPoints: source.nativeFlameLashPathPoints(geometry),
      normalPairCount: source.nativeFlameLashRibbonPairCount(geometry, true),
      coarsePairCount: source.nativeFlameLashRibbonPairCount(geometry, false),
      ribbon: { kind: ribbon.kind, vertices: Array.from(ribbon.vertices), indices: Array.from(ribbon.indices),
        uvs: Array.from(ribbon.uvs), colors: Array.from(ribbon.colors), parameters: Array.from(ribbon.parameters) } })
  }
}
const phases = [0, 1, 715827882, 2147483647, 4294967295].map(tick => [tick, source.nativeFlameLashBirthPhase(tick)])
const privateSamples = [0, 1, -1, -2147483648, 2147483647].map(state => ({
  state, ...source.nativeFlameLashPrivateSample(state),
}))
const points = [point(1.1, -2.2), point(15.125, 90.99), point(150.6, -40.4)]
const splines = [-.001, 0, .4, 1, 1.2, 2, 2.5].map(parameter => ({ points, parameter,
  point: source.nativeFlameLashSplinePoint(points, parameter),
  normal: source.nativeFlameLashSplineNormal(points, parameter) }))
const errors = []
for (const [operation, index, budget] of [['path', 6, 1], ['path', 6, 0], ['pairs', 0, 2], ['pairs', 6, 2]]) {
  let message
  try {
    if (operation === 'path') source.nativeFlameLashPathPoints(cases[index].geometry, budget)
    else source.nativeFlameLashRibbonPairCount(cases[index].geometry, true, budget)
  } catch (error) { message = error.message }
  assert.ok(message, 'the recorded Source admission must reject this actual budget')
  errors.push({ operation, index, budget, message })
}
writeFixture('flame-geometry-goldens.json', 'tools/website-oracle/flame-geometry-goldens.mjs',
  `Website@${revision}/core-kernels/native-weld-flame-ribbon.ts`,
  { cases, phases, privateSamples, splines, errors })
