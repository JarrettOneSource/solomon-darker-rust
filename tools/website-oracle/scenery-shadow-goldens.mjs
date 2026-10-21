import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const scenery = await loadKernel('renderer/native-scenery-shadow.ts')
const scrub = await loadKernel('renderer/native-scrub-shadow.ts')
const placement = await loadKernel('../editor/native-render-plan.ts')
const art = await loadKernel('renderer/native-enemy-assets.ts')
const manifest = JSON.parse(readFileSync(resolve(websiteDir, 'frontend/src/editor/manifest/deadhawg.json'), 'utf8'))
const existing = JSON.parse(readFileSync(resolve(fixturesDir, 'boneyard-scenery-goldens.json'), 'utf8'))
const glyphs = manifest.entries.map((entry, index) => entry.empty ? null : art.nativeEnemySpriteGeometry('DeadHawg', index))
const extra = { ...existing.cases[0].input, objects: [
  { eid: 'tree-max', typeId: 2001, variant: 14, pos: { x: 450.125, y: 901.3 } },
  { eid: 'tree-secondary-max', typeId: 2001, variant: 2, secondaryVariant: 20, pos: { x: 23.2, y: 301.7 } },
  { eid: 'grave-max', typeId: 2029, variant: 16, pos: { x: 89.3, y: -25.2 } },
  { eid: 'monument-max', typeId: 2009, variant: 20, pos: { x: -71.25, y: 11.5 } },
  { eid: 'building-max', typeId: 2040, variant: 3, pos: { x: 99.1, y: -75.7 } },
] }
const scenes = [...existing.cases.map(row => row.input), extra].map(input => {
  const plan = placement.buildNativeRenderPlan(input)
  const layers = plan.shadows.filter(layer => layer.kind === 'object'
    || (layer.fence.segmentCode ?? layer.fence.style ?? 0) !== 3)
  return { input, modes: [false, true].map(complex => ({ complex,
    layers: layers.map(layer => scenery.nativeSceneryShadowPlan(layer, complex)),
  })) }
})
const goodies = [0, 1, 2].flatMap(phase => [false, true].map(complex => {
  const goodie = { position: { x: -221.125, y: 91.35 }, subtype: 0, phase }
  return { goodie, complex, quads: scenery.nativeGoodieShadowPlan(goodie, complex) }
}))
const walls = [
  [{ x: -72.5, y: 83.1 }, { x: 283.125, y: -199.25 }],
  [{ x: 31.75, y: 0 }, { x: 31.75, y: 0 }],
].map(([start, end]) => ({ start, end, quad: scenery.nativeWallSkirtPlan(start, end) }))
const records = [
  { baseAlpha: .4, behindScalar: .5, direction: { x: .6, y: -.8 }, distanceFraction: .25,
    projectionDistance: 145.75, sourcePosition: { x: 130, y: 40 }, sourceRadius: 200 },
  { baseAlpha: 1, behindScalar: .9, direction: { x: -.8, y: .6 }, distanceFraction: .005,
    projectionDistance: 300.125, sourcePosition: { x: 0, y: 40 }, sourceRadius: 400 },
]
const scrubs = [0, 15, 18].map((variant, index) => {
  const input = { variant, seed: index * 17, initialTick: 120, position: { x: 102.35, y: -44.65 } }
  const state = new scrub.NativeScrubShadowState(input.seed, input.initialTick)
  const frames = [
    { tick: 120, visible: true, complex: false, lit: true, surface: [false, false, true] },
    { tick: 120.25, visible: false, complex: true, lit: true, surface: [true, true, true] },
    { tick: 121.25, visible: true, complex: true, lit: true, surface: index === 1 ? [false, false, false] : [true, false, true] },
    { tick: 121.1, visible: true, complex: false, lit: false, surface: [true, true, true] },
    { tick: 130.5, visible: false, complex: true, lit: true, surface: [true, true, true] },
    { tick: 131, visible: true, complex: true, lit: false, surface: [false, false, false] },
    { tick: 140.75, visible: true, complex: true, lit: true, surface: [false, false, false] },
  ].map(frame => {
    state.advanceTo(frame.tick)
    const queries = []
    const quads = frame.visible ? scrub.nativeScrubShadowPlan(scrub.nativeScrubGlyph(variant),
      input.position, state, frame.lit && frame.complex ? records : [], frame.complex, point => {
        queries.push(point)
        return frame.surface[queries.length - 1]
      }) : []
    return { ...frame, queries, phase: state.phase, uvInset: state.uvInset,
      specialSurface: state.specialSurface, quads: quads.map(quad => ({ ...quad,
        texture: quad.role === 'scrub-surface-reflection' ? scrub.NATIVE_SCRUB_REFLECTION_ENTRY : 264 + variant,
      })) }
  })
  return { input, frames }
})
writeFixture('scenery-shadow-goldens.json', 'tools/website-oracle/scenery-shadow-goldens.mjs',
  'Website@fa08ae24/renderer/{native-scenery-shadow,native-scrub-shadow}.ts',
  { glyphs, scenes, goodies, walls, records, scrubs })
