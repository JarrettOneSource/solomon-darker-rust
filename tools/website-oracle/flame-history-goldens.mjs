import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const revision = execFileSync('git', ['-C', websiteDir, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim()
assert.equal(revision, 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83')
const histories = await loadKernel('protocol/flame-presentation-history.ts')
const presentation = await loadKernel('protocol/flame-presentation.ts')
const host = await loadKernel('host/flame-presentation-history.ts')
const factory = await loadKernel('core-kernels/native-weld-flame-birth.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const managers = await loadKernel('core-kernels/native-world-manager-order.ts')
function births(id, tick, ownerId = 'flame-owner') {
  const order = managers.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 30, transient: 90 } })
  const start = { x: 100, y: 200 }, end = { x: 500, y: 100 }, control = { x: 300, y: 150 }
  const result = factory.createNativeFlameLashFactory({
    channel: { id, tick, buildId: 1003, direction: { x: .6, y: -.8 }, origin: start, endpoint: end,
      midpoint: control, targetId: null, underpowered: false, enhancedEffects: true,
      vector: [1, 2, 3, .5, 0, 0, 0, 0], ownerId, worldKey: 'hub:courtyard' },
    geometry: { start, control, end, offset: { x: 0, y: 0 } }, sourceFadeId: id + 1,
    role: 'main', width: 1, cameraDisplacement: { x: .5, y: -1 },
    creatorRegistration: { managerLane: 'actor', registrationOrdinal: 48 }, miscAppendOrdinal: 0,
    rng: random.createNativeRng(17), registerWorldPainter: order.register,
  })
  return [result.channel, result.source].map(presentation.flamePresentationBirth)
}
const first = births(71, 1), second = births(81, 2, 'flame🧙'), third = births(91, 3)
const baseline = (worldTick, epoch = 1) => ({ epoch, worldTick })
const frame = (throughTick, afterTick, incoming, epoch = 1) => ({ kind: 'continuous', epoch, worldKey: 'hub', throughTick, afterTick, births: incoming })
const reset = (throughTick, epoch = 1) => ({ kind: 'reset', epoch, worldKey: 'hub', throughTick, births: [] })
function historyCase(name, options, operations) {
  const history = histories.createFlamePresentationHistory(options)
  const rows = operations.map(input => {
    try {
      let result = null
      switch (input.op) {
        case 'record': history.recordTick(input.tick, input.births); break
        case 'retire': history.retireBirth(input.birth, input.tick); break
        case 'owner': history.retireOwner(input.owner, input.tick); break
        case 'discard': history.discardThroughTick(input.tick); break
        case 'reset': history.reset(input.worldKey, input.tick); break
        case 'frame': result = history.frame(input.baseline, input.tick); break
      }
      return { input, result }
    } catch (error) { return { input, error: error.message } }
  })
  return { name, options, rows }
}
const options = { worldKey: 'hub', tick: 0, maximumBirths: 16, maximumBytes: 100000, initialEpoch: 1 }
const historyCases = [historyCase('birth-retirement-reset-and-coverage', options, [
  { op: 'frame', baseline: null, tick: 0 }, { op: 'record', tick: 1, births: first },
  { op: 'frame', baseline: baseline(0), tick: 1 }, { op: 'record', tick: 2, births: second },
  { op: 'retire', birth: first[1], tick: 2 }, { op: 'frame', baseline: baseline(1), tick: 2 },
  { op: 'owner', owner: 'flame🧙', tick: 2 }, { op: 'frame', baseline: baseline(1), tick: 2 },
  { op: 'discard', tick: 1 }, { op: 'frame', baseline: baseline(0), tick: 2 },
  { op: 'record', tick: 4, births: [] }, { op: 'frame', baseline: baseline(1), tick: 2 },
  { op: 'reset', worldKey: 'boneyard:next', tick: 17 }, { op: 'frame', baseline: baseline(2), tick: 17 },
]), historyCase('whole-tick-count-eviction', { ...options, maximumBirths: 3 }, [
  { op: 'record', tick: 1, births: first }, { op: 'record', tick: 2, births: second },
  { op: 'frame', baseline: baseline(0), tick: 2 }, { op: 'frame', baseline: baseline(1), tick: 2 },
]), historyCase('exact-utf16-byte-eviction', { ...options, maximumBytes: JSON.stringify(second[0]).length * 2 }, [
  { op: 'record', tick: 1, births: first }, { op: 'record', tick: 2, births: second },
  { op: 'frame', baseline: baseline(1), tick: 2 },
]), historyCase('fallback-live-actor-shortening', { ...options, tick: 2 }, [
  { op: 'retire', birth: first[1], tick: 2 }, { op: 'frame', baseline: baseline(2), tick: 2 },
  { op: 'record', tick: 3, births: [] }, { op: 'frame', baseline: baseline(2), tick: 3 },
])]
function replayCase(name, maximumBirths, camera, operations) {
  const replay = histories.createFlamePresentationReplay(maximumBirths, camera ? birth => birth.recipe.kind === 'ribbon' ? birth.birthTick + 7 : birth.endTickExclusive : undefined)
  const rows = operations.map(input => {
    try {
      let result = null
      switch (input.op) {
        case 'accept': replay.accept(input.frame); break
        case 'sample': result = replay.sample(input.tick); break
        case 'history': result = replay.historyAt(input.tick); break
        case 'discard': replay.discardBeforeTick(input.tick); break
        case 'retained': result = replay.retained(input.birth); break
      }
      return { input, result, throughTick: replay.throughTick() }
    } catch (error) { return { input, error: error.message, throughTick: replay.throughTick() } }
  })
  return { name, maximumBirths, camera, rows }
}
const retired = { ...first[1], endTickExclusive: 2 }
const changed = structuredClone(first[1]); changed.recipe.baseScale += .01
const replayCases = [replayCase('short-lifetime-camera-and-tombstone', 16, true, [
  { op: 'accept', frame: reset(0) }, { op: 'accept', frame: frame(2, 0, [...first, ...second]) },
  { op: 'sample', tick: .5 }, { op: 'sample', tick: 1.75 }, { op: 'sample', tick: 2 },
  { op: 'accept', frame: frame(2, 1, [retired]) }, { op: 'accept', frame: frame(2, 0, first) },
  { op: 'retained', birth: first[1] }, { op: 'discard', tick: 3 }, { op: 'history', tick: 2 },
  { op: 'accept', frame: frame(7, 2, []) }, { op: 'history', tick: 7 },
  { op: 'discard', tick: 8 }, { op: 'history', tick: 8 },
]), replayCase('atomic-invalid-and-recovery', 2, false, [
  { op: 'accept', frame: reset(0) }, { op: 'accept', frame: frame(1, 0, first) },
  { op: 'accept', frame: frame(2, 1, second) }, { op: 'sample', tick: 1 },
  { op: 'accept', frame: frame(2, 1, [changed]) }, { op: 'retained', birth: first[1] },
  { op: 'accept', frame: frame(3, 2, []) }, { op: 'accept', frame: reset(3, 2) },
  { op: 'accept', frame: frame(4, 3, [], 1) }, { op: 'sample', tick: 4 },
]), replayCase('post-reset-prebirth-retirement', 16, false, [
  { op: 'accept', frame: reset(2) }, { op: 'accept', frame: frame(3, 2, [retired, ...third]) },
  { op: 'sample', tick: 1 }, { op: 'sample', tick: 3 }, { op: 'retained', birth: first[1] },
])]
const materialized = [...first, ...second].map(birth => ({ birth,
  frames: [birth.birthTick - .5, birth.birthTick, birth.birthTick + .75, birth.endTickExclusive].map(tick => ({ tick, actor: presentation.materializeFlamePresentation(birth, tick) })) }))
const hostHistory = host.createHostFlamePresentationHistory()
function state(tick, incoming, worldKey = 'hub') {
  return { tick, run: { phase: 'hub' }, world: worldKey === 'hub' ? { kind: 'hub' } : { kind: 'boneyard', runId: worldKey.slice(9) },
    primarySpells: { transients: incoming.map(birth => presentation.materializeFlamePresentation(birth, tick)).filter(Boolean) } }
}
let previous = state(0, [])
hostHistory.resetFrame(previous)
const advances = []
for (const [tick, incoming, worldKey] of [[1, first, 'hub'], [2, [...first, ...second], 'hub'], [2, [...first, ...second], 'hub'],
  [2, [], 'hub'], [3, third, 'hub'], [3, [], 'boneyard:changed']]) {
  const current = state(tick, incoming, worldKey)
  const before = previous.primarySpells.transients.map(presentation.flamePresentationBirth).filter(Boolean)
  const after = current.primarySpells.transients.map(presentation.flamePresentationBirth).filter(Boolean)
  hostHistory.advance(previous, current)
  advances.push({ tick, worldKey, before, after, reset: hostHistory.resetFrame(current),
    frame: hostHistory.frame(current, baseline(Math.max(0, tick - 1))) })
  previous = current
}
writeFixture('flame-history-goldens.json', 'tools/website-oracle/flame-history-goldens.mjs',
  `Website@${revision}/protocol/flame-presentation{,-history}.ts`, { historyCases, replayCases, materialized, advances,
    sizes: [...first, ...second].map(birth => ({ birth, bytes: JSON.stringify(birth).length * 2 })) })
