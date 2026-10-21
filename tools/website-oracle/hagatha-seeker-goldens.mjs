import { loadKernel, writeFixture } from './oracle.mjs'

const hagatha = await loadKernel('core-kernels/native-hagatha-effects.ts')
const line = await loadKernel('core-kernels/native-line.ts')
const cases = []
for (const distance of [0, 99.999, 100, 100.001, 101, 299.999, 300, 300.001, 1000]) {
  for (const heading of [0, 37.5, 90, 180]) {
    for (const tick of [0, 5, 180, 1000.5, 1000000]) {
      const position = { x: 12.25, y: -7.75 }
      const radians = heading * Math.PI / 180
      const targets = ['bonus', 'gold', 'sack'].map((kind, index) => ({
        id: 17 + index,
        kind,
        position: {
          x: position.x + Math.sin(radians) * distance,
          y: position.y - Math.cos(radians) * distance,
        },
      }))
      const segments = hagatha.nativeHagathaSeekerSegments(position, targets, tick)
      cases.push({ position, targets, tick, segments,
        meshes: segments.map(hagatha.nativeHagathaSeekerMeshPlan) })
    }
  }
}
const lines = []
for (const width of [0.125, 1, 3, 7.75]) {
  for (const end of [{ x: 20, y: 10 }, { x: -10, y: -10 }, { x: 12.5, y: -30.25 }]) {
    const start = { x: 10, y: 10 }
    lines.push({ start, end, width, vertices: line.nativeLineVertices(start, end, width) })
  }
}
const colors = []
for (const channels of [
  [0, 0, 0, 0], [1, 1, 1, 1], [0.25, 0.5, 0.75, 1],
  [-1, 1.5, 2, 0.999999], [1 / 255, 2 / 255, 127 / 255, 254 / 255],
  [1000000000, -1000000000, 1.1, 0.1],
]) {
  colors.push({ channels, color: line.nativeLineColor(...channels) })
}
const alphaMeshes = []
for (const alpha of [-1, 0, 0.25, 0.35, 1, 2]) {
  for (const startVisible of [false, true]) {
    for (const endVisible of [false, true]) {
      const segment = { alpha, start: { x: 10, y: 20 }, end: { x: 20, y: 20 },
        startVisible, endVisible, targetId: 1, targetKind: 'gold', width: 3 }
      alphaMeshes.push({ segment, mesh: hagatha.nativeHagathaSeekerMeshPlan(segment) })
    }
  }
}
writeFixture(
  'hagatha-seeker-goldens.json', 'tools/website-oracle/hagatha-seeker-goldens.mjs',
  'Website/frontend/src/game/core-kernels/native-hagatha-effects.ts and native-line.ts',
  { program: hagatha.NATIVE_HAGATHA_SEEKER_PROGRAM, ramp: hagatha.NATIVE_HAGATHA_SEEKER_RAMP_RGBA,
    rampU: hagatha.NATIVE_HAGATHA_SEEKER_RAMP_U, cases, lines, colors, alphaMeshes },
)
