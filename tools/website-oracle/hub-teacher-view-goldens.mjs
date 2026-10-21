// Art selection and world painter lifetime at cast/release/fade boundaries. The existing
// teacher fixture already covers every tick of three cycles, so this keeps only view edges.
import { loadKernel, point, writeFixture } from './oracle.mjs'

const teacher = await loadKernel('hub-teacher.ts')
const ticks = [0, 1, 267, 268, 269, 277, 278, 279, 280, 281, 282, 283, 284,
  300, 317, 318, 319, 346, 347, 400, 401, 402, 846, 847, 1115]
const member = ({ alpha, scaleX, scaleY, visible }) => [alpha, scaleX, scaleY, visible ? 1 : 0]
const samples = []
for (const seed of [0, 7]) {
  for (const tick of ticks) {
    const burst = teacher.hubTeacherBurstAt(tick / 100, seed)
    samples.push({ tick, seed,
      actorFrame: teacher.hubTeacherFrameAt(tick / 100, seed),
      releaseOrigin: [576.5 + teacher.HUB_TEACHER_CAST_ORIGIN.x, 710.5 + teacher.HUB_TEACHER_CAST_ORIGIN.y],
      worldPainters: burst.column.visible || burst.frames.visible
        ? [0, 1].map((index) => burst.releaseIndex * 2 + index) : null,
      core: member(burst.core), flare: member(burst.flare), column: member(burst.column),
      frames: [...member(burst.frames), burst.frames.frame],
    })
  }
}
writeFixture('hub-teacher-view-goldens.json', 'tools/website-oracle/hub-teacher-view-goldens.mjs',
  'Website/frontend/src/game: hub-teacher.ts, renderer/hub-world-scene.ts (HubTeacherView)', {
    position: [576.5, 710.5], runeCenter: point(teacher.HUB_TEACHER_RUNE_CENTER),
    runeAlpha: teacher.HUB_TEACHER_RUNE_ALPHA, samples,
  })
