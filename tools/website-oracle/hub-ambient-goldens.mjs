#!/usr/bin/env node
// Records the courtyard's ambient kernels: the fountain spray, seal pulse and statue bob that
// `stepHubAmbient` advances each tick; the teacher's cast cycle and burst; and the native
// random reductions the teacher draws from.
import { loadKernel, point, writeFixture } from './oracle.mjs'

const ambient = await loadKernel('core-server/hub-ambient.ts')
const presentation = await loadKernel('hub-presentation.ts')
const teacher = await loadKernel('hub-teacher.ts')
const randomDomain = await loadKernel('core-kernels/native-random-domain.ts')
const manager = await loadKernel('core-kernels/native-world-manager-order.ts')

const generator = 'tools/website-oracle/hub-ambient-goldens.mjs'
const color = ({ red, green, blue, alpha }) => [red, green, blue, alpha]

// The ambient state tick by tick, with the presentation derived from it every fifty ticks.
const AMBIENT_TICKS = 3000
const SAMPLE_EVERY = 50
const initial = ambient.createHubAmbientState()
let state = initial
const ticks = []
const samples = []
const worldSamples = []
const order = manager.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 100, transient: 17 } })
let managedState = ambient.createHubAmbientState()
for (let tick = 1; tick <= 1116; tick += 1) {
  managedState = ambient.stepHubAmbient(managedState, order.register)
  if (![267, 268, 269, 318, 319, 1114, 1115, 1116].includes(tick)) continue
  worldSamples.push({ tick,
    registrations: managedState.teacherWorldRelease?.painterRegistrations.map(({ registrationOrdinal }) => registrationOrdinal) ?? null,
    nextActor: order.state().nextRegistrationOrdinal.actor,
    nextTransient: order.state().nextRegistrationOrdinal.transient,
  })
}
for (let tick = 1; tick <= AMBIENT_TICKS; tick += 1) {
  state = ambient.stepHubAmbient(state)
  ticks.push([
    state.rngState,
    state.sealCorePhase,
    state.sealGlyphPhase,
    state.statuePhaseDegrees,
    state.teacherTick,
    state.teacherWorldRelease?.releaseIndex ?? -1,
    state.fountainParticles.map((particle) => [particle.id, particle.remaining, particle.scale]),
    state.teacherWorldRelease?.painterRegistrations.map(({ managerLane, registrationOrdinal }) => [managerLane, registrationOrdinal]) ?? [],
  ])
  if (tick % SAMPLE_EVERY === 0) {
    const seals = presentation.hubSealColors(state)
    const statue = presentation.hubStatueOffsets(state)
    samples.push({
      tick,
      sealCore: color(seals.core),
      sealGlyphs: color(seals.glyphs),
      statueAura: point(statue.aura),
      statueBody: point(statue.body),
      fountainAlphas: state.fountainParticles.map(presentation.hubFountainParticleAlpha),
    })
  }
}

// The teacher's phase and frame at every tick of three cycles, and the burst through each
// release (every tick from the release to the idle start, then every fiftieth).
const TEACHER_SEEDS = [0, 7]
const CYCLES = 3
const cycle = teacher.HUB_TEACHER_CYCLE_TICKS
const seconds = (tick) => tick / teacher.HUB_TEACHER_TICKS_PER_SECOND
const member = ({ alpha, scaleX, scaleY, visible }) => [alpha, scaleX, scaleY, visible ? 1 : 0]
const phases = []
const bursts = []
for (const seed of TEACHER_SEEDS) {
  for (let tick = 0; tick < CYCLES * cycle; tick += 1) {
    phases.push([tick, seed, teacher.hubTeacherPhaseAt(seconds(tick)), teacher.hubTeacherFrameAt(seconds(tick), seed)])
    const cycleTick = tick % cycle
    const releasing = cycleTick >= teacher.HUB_TEACHER_CAST_TICKS && cycleTick < teacher.HUB_TEACHER_IDLE_START_TICK
    if (!releasing && cycleTick % 50 !== 0 && cycleTick !== teacher.HUB_TEACHER_CAST_TICKS - 1) continue
    const burst = teacher.hubTeacherBurstAt(seconds(tick), seed)
    bursts.push({
      tick,
      seed,
      ageTicks: burst.ageTicks,
      releaseIndex: burst.releaseIndex,
      visible: burst.visible,
      core: member(burst.core),
      flare: member(burst.flare),
      column: member(burst.column),
      frames: [burst.frames.alpha, burst.frames.frame, burst.frames.scaleX, burst.frames.scaleY, burst.frames.visible ? 1 : 0],
    })
  }
}

// The native reductions of a semantic word into an integer bound or a float maximum.
const words = [
  0, 1, 63, 64, 65, 127, 128, 192, 0x3fffffff, 0x40000000, 0x7fffffff, 0x80000000, 0xffffffff,
  0x9e3779b1, 0x85ebca6b, 0x7feb352d, 0x846ca68b, 0xdeadbeef, 12345678, 0x2f6e2b1d,
]
const bounds = [2, 3, 7, 10, 11, 24, 80, 100, 1000, 100001]
const maxima = [1, 0.1, 0.2, 0.5, 3, 0.019, 0.15]
const ints = []
const floats = []
for (const word of words) {
  for (const bound of bounds) ints.push([word, bound, randomDomain.nativeRandomIntFromSemanticWord(word, bound)])
  for (const maximum of maxima) floats.push([word, maximum, randomDomain.nativeRandomFloatFromSemanticWord(word, maximum)])
}

writeFixture(
  'hub-ambient-goldens.json',
  generator,
  'Website/frontend/src/game: core-server/hub-ambient.ts, hub-presentation.ts',
  {
    initial: {
      rngState: initial.rngState,
      sealCorePhase: initial.sealCorePhase,
      sealGlyphPhase: initial.sealGlyphPhase,
      statuePhaseDegrees: initial.statuePhaseDegrees,
      teacherTick: initial.teacherTick,
      fountainParticles: initial.fountainParticles.length,
    },
    ticks,
    samples,
    worldSamples,
  },
)
writeFixture(
  'hub-teacher-goldens.json',
  generator,
  'Website/frontend/src/game: hub-teacher.ts, core-kernels/native-random-domain.ts',
  { phases, bursts, randomDomain: { ints, floats } },
)
const spawned = state.nextFountainParticleId
const alive = state.fountainParticles.length
console.log(`ambient: ${ticks.length} ticks, ${spawned} drops spawned, ${alive} alive at the end, ${samples.length} presentation samples`)
console.log(`teacher: ${phases.length} phase/frame ticks, ${bursts.length} burst samples; random domain: ${ints.length} ints, ${floats.length} floats`)
