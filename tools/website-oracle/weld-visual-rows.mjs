// Add representative draw samples to the existing actor journeys. The physics
// rows stay intact; there is no second set of synthesized presentation actors.
import { loadKernel } from './oracle.mjs'

const weld = await loadKernel('renderer/primary-spell-weld-native.ts')

const point = value => value === null ? null : [value.x, value.y]
const pairs = values => Array.from({ length: values.length / 2 }, (_, i) => [values[i * 2], values[i * 2 + 1]])
const sprite = draw => ({
  alpha: draw.alpha,
  atlas: draw.atlas,
  blend: draw.blend,
  diffuse: draw.textureColor === 'diffuse',
  matrix: draw.matrix,
  offset: point(draw.offset),
  record: draw.record,
  rotation: draw.rotationRadians,
  scale: [draw.scaleX, draw.scaleY],
  tint: draw.tint,
})
const plan = value => ({
  lines: value.lines.map(({ alpha, color, end, start, width }) => ({ alpha, color, end: point(end), start: point(start), width })),
  meshes: value.meshes.map(({ alpha, blend, indices, record, tint, uvs, vertices }) => ({
    alpha, blend, indices: Array.from(indices), record, tint, uvs: pairs(uvs), vertices: pairs(vertices),
  })),
  position: point(value.position),
  regionLightPoint: point(value.regionLightPoint),
  sortBias: value.sortBias,
  sprites: value.sprites.map(sprite),
  underlays: (value.underlays ?? []).map(sprite),
  worldY: value.worldY,
})

export function addWeldVisualSamples(sample, frames, actorsAt, ticks) {
  sample.visuals = frames.filter(frame => ticks.includes(frame.tick)).map(frame => {
    const actors = actorsAt(frame).filter(actor => actor !== null)
    const presentationFrame = 91 + frame.tick
    const puppetHit = actors.some(actor => actor.kind === 'weld-meteor')
    return { tick: frame.tick, presentationFrame, puppetHit,
      plans: actors.map(actor => plan(weld.nativeWeldVisualPlan(actor, presentationFrame, puppetHit))) }
  })
}
