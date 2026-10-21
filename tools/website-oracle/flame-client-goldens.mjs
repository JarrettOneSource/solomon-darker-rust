// Current Source client replay and actual mixed-family feedback intake.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { execFileSync } from 'node:child_process'
import { fixturesDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const revision = execFileSync('git', ['-C', websiteDir, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim()
assert.equal(revision, 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83')
const client = await loadKernel('client/flame-presentation.ts')
const protocol = await loadKernel('protocol/flame-presentation.ts')
const feedback = await loadKernel('renderer/native-screen-feedback.ts')
const born = JSON.parse(readFileSync(resolve(fixturesDir, 'flame-birth-goldens.json'), 'utf8')).cases[4]
const births = [born.channel, born.source].map(protocol.flamePresentationBirth)
const reset = (epoch, throughTick) => ({ kind: 'reset', epoch, worldKey: 'boneyard:flame', throughTick, births: [] })
const continuous = (afterTick, throughTick, rows = births) => ({ kind: 'continuous', epoch: 1, worldKey: 'boneyard:flame', throughTick, afterTick, births: rows })
const sample = (tick, stateBirths = [], liveTick = tick) => ({ kind: 'sample', tick, liveTick, stateBirths })
const replayInputs = [
  { name: 'admission-does-not-replay-continuous-past', initial: continuous(132, 133), steps: [sample(133), sample(133, [0, 1]), sample(134, [0, 1]), sample(133.5, [0, 1])] },
  { name: 'retained-between-snapshots-retirement-and-cameras', initial: reset(1, 132), steps: [
    { kind: 'accept', frame: continuous(132, 135), oldestTick: 132 },
    sample(132.9), sample(133.75), sample(134), sample(139, [], 139), sample(134, [], 140),
    { kind: 'accept', frame: continuous(133, 135, [{ ...births[1], endTickExclusive: 134 }]), oldestTick: 132 },
    sample(134, [1]), { kind: 'accept', frame: reset(1, 130), oldestTick: 132 }, sample(134, [1]),
    { kind: 'accept', frame: reset(2, 10), oldestTick: 132 }, sample(9, [0, 1]), sample(133, [0, 1]),
    { kind: 'accept', frame: reset(1, 200), oldestTick: 0 }, sample(133, [0, 1]),
  ] },
  { name: 'coverage-gap-is-a-real-recovery-error', initial: reset(1, 132), steps: [
    { kind: 'accept', frame: continuous(133, 135), oldestTick: 132 }, sample(133),
  ] },
]
const row = state => state.transients.map(actor => ({ id: actor.id, kind: actor.kind, age: actor.ageTicks,
  alpha: actor.kind === 'weld-flame-lash-fade' ? actor.alpha : null }))
const replays = replayInputs.map(input => {
  const retained = client.createRetainedFlamePresentation(input.initial)
  return { ...input, steps: input.steps.map(step => {
    if (step.kind === 'accept') {
      try { return { ...step, replaced: retained.accept(step.frame, step.oldestTick) } }
      catch (error) { return { ...step, error: error.message } }
    }
    const state = { nextId: 100, projectiles: [], transients: [
      { id: 3, kind: 'fire', ageTicks: 7 },
      ...step.stateBirths.map(index => protocol.materializeFlamePresentation(births[index], births[index].birthTick)),
    ] }
    const cameras = retained.cameraDisplacements(state, step.tick, step.liveTick)
    return { ...step, actors: row(retained.sample(state, step.tick)), cameras }
  }) }
})
const context = { cameraCenter: { x: 0, y: 0 }, localPlayerAlternate: false, visibleWorldWidth: 1000 }
const worldKey = 'boneyard:flame'
const flame = (id, birthTick, x, y = 0) => ({ kind: 'flame', proposals: [{ id, birthTick, worldKey, displacement: { x, y } }] })
const current = (eventId, tick, x, y = 0) => ({ kind: 'current', eventId, tick, worldKey, displacement: { x, y } })
const synchronize = (worldTick, epoch, liveTick = worldTick) => ({ kind: 'synchronize', worldTick, epoch, liveTick })
const observe = tick => ({ kind: 'sample', tick })
const flash = (order, tick, color, onlyIfClear = false, position = { x: 0, y: 0 }) => ({ order, tick, worldKey, position, onlyIfClear,
  flash: { alpha: 1, red: color[0], green: color[1], blue: color[2], decayPerTick: Math.fround(.01), pointAttenuated: true } })
const cyan = flash(1, 100, [Math.fround(.9), 1, 1])
const black = flash(2, 101, [0, 0, 0])
const green = flash(3, 102, [0, 1, 0], true)
const flashes = (writes, epoch = 0, gainContext = context) => ({ kind: 'flashes', state: { epoch, nextOrder: 20, writes }, context: gainContext })
const feedbackInputs = [
  { name: 'late-flame-strict-ties-and-birth-order', initialTick: 0, steps: [
    current(1, 0, 1.7084840536117554, 1.4097665548324585), observe(2),
    flame(2, 1, -1.7084840536117554, -1.409766674041748), observe(2),
    flame(2, 1, -1.7084840536117554, -1.409766674041748), observe(3), observe(9),
    flame(2, 10, -4), observe(10), flame(1, 10, 4), observe(10), observe(17),
  ] },
  { name: 'shared-region-cutoff-and-replacement', initialTick: 0, steps: [
    synchronize(0, 1), current(1, 0, Math.fround(2 / 3)), flame(2, 1, 0), observe(1), observe(2),
    synchronize(100, 1), flame(3, 100, 4), observe(100), synchronize(1, 2), flame(1, 1, 0, 4), observe(1),
    synchronize(1, 3), flame(1, 1, 0, 4), observe(1), synchronize(0, 4), observe(0),
  ] },
  { name: 'future-flashes-overwrite-conditional-and-delivery', initialTick: 98, steps: [
    flashes([cyan]), observe(99.9), observe(100), flashes([black]), observe(101), flashes([green]), observe(105),
    observe(205), flashes([cyan, black, green]), observe(205), flashes([], 1), observe(205), flashes([cyan]), observe(205),
  ] },
  { name: 'ineligible-flash-gain-remains-live-until-admission', initialTick: 98, steps: [
    flashes([cyan], 0, { ...context, cameraCenter: { x: 1100, y: 0 } }), observe(99),
    flashes([cyan]), observe(100), flashes([cyan], 0, { ...context, cameraCenter: { x: 1100, y: 0 } }), observe(101),
  ] },
]
const feedbackCases = feedbackInputs.map(input => {
  const lane = new feedback.NativeSecondaryScreenFeedbackPresentation(input.initialTick, worldKey)
  return { ...input, steps: input.steps.map(step => {
    if (step.kind === 'flame') lane.consumeFlameCameraDisplacements(step.proposals)
    if (step.kind === 'current') lane.consumePrimaryCameraDisplacement(step)
    if (step.kind === 'synchronize') lane.synchronizeWorldTimeline(step.worldTick, step.epoch, step.liveTick)
    if (step.kind === 'flashes') lane.consumeScreenFlashes(step.state, step.context)
    if (step.kind !== 'sample') return step
    return { ...step, overlay: lane.sample(step.tick), magnitude: lane.sampleCameraMagnitude(step.tick), displacement: lane.sampleCameraDisplacement(step.tick) }
  }) }
})
writeFixture('flame-client-goldens.json', 'tools/website-oracle/flame-client-goldens.mjs',
  `Website@${revision}/{client/flame-presentation,renderer/native-screen-feedback}.ts`, { births, replays, feedbackCases })
