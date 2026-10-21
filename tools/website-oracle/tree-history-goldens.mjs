// Real delivered pose APIs and Tree frame/scan owners; no stateful sample() calls.
import { loadKernel, writeFixture } from './oracle.mjs'

const timelineKernel = await loadKernel('client/boneyard-presentation-timeline.ts')
const actors = await loadKernel('renderer/boneyard-tree-actors.ts')
const trees = await loadKernel('renderer/boneyard-tree-occlusion.ts')
const { BoneyardTreeFrameHistory } = await loadKernel('renderer/boneyard-tree-frame-history.ts')
const enemy = (id, enemyToken, nativeTypeId, state, coffinState, x) => ({
  id, enemyToken, nativeTypeId, position: { x, y: 155 }, animation: { state, coffinState },
})
const older = {
  tick: 10, primarySpells: { hail: {} }, run: { phase: 'active' },
  players: { local: { position: { x: 100, y: 150 } }, departed: { position: { x: 20, y: 30 } } },
  world: { enemies: [enemy(1, 'SKELETON', 1001, 'idle', null, 120),
    enemy(2, 'COFFIN', 1013, 'idle', 'hidden', 130),
    enemy(3, 'WRAITH', 1007, 'death', null, 140),
    enemy(4, 'IMP', 1001, 'idle', null, 160)],
  maggots: [{ id: 20, position: { x: 180, y: 160 }, state: 'walking', nativeTreeQueryMember: true },
    { id: 21, position: { x: 190, y: 160 }, state: 'walking', nativeTreeQueryMember: false }],
  encounter: { position: { x: 210, y: 150 }, phase: 'digging' } },
}
const newer = structuredClone(older)
newer.tick = 20
newer.players = { local: { position: { x: 200, y: 250 } }, arrived: { position: { x: 220, y: 230 } } }
newer.world.enemies = [enemy(1, 'SKELETON', 1001, 'death', null, 220),
  enemy(2, 'COFFIN', 1013, 'idle', 'opening', 230), enemy(5, 'PORTAL', 5021, 'idle', null, 240)]
newer.world.maggots = [{ id: 20, position: { x: 280, y: 260 }, state: 'walking', nativeTreeQueryMember: false },
  { id: 22, position: { x: 290, y: 260 }, state: 'walking', nativeTreeQueryMember: true }]
newer.world.encounter = { position: { x: 310, y: 250 }, phase: 'gone' }
const timeline = timelineKernel.createBoneyardPresentationTimeline({
  initialSnapshot: older, initialReceivedAtMs: 0, serverTickRate: 100, snapshotRate: 20,
})
timeline.push(newer, 100)
const poses = [9, 10, 12.5, 19.999, 20, 21].map(tick => {
  const expected = timeline.sampleSceneryActorPosesAtTick(tick)
  return { tick, expected, local: timeline.samplePlayerPositionAtTick(tick, 'local'),
    positions: expected && actors.nativeTreeActorPositions(expected, 'local') }
})

const tree = { eid: 'history-canopy', position: { x: 100, y: 250 },
  mainVariant: 0, secondaryVariant: 0, secondaryVisible: true }
const cameraBounds = { x: -200, y: -200, w: 600, h: 600 }
const presentation = new trees.BoneyardTreeOcclusionPresentation([tree], 10)
const frameHistory = new BoneyardTreeFrameHistory()
const operations = [
  { tick: 10, retained: 10, epoch: 'first' },
  { tick: 24, retained: 10, epoch: 'first' },
  { tick: 55, retained: 20, epoch: 'first' },
  { tick: 90, retained: 70, epoch: 'first', offCamera: true },
  { tick: 130, retained: 95, epoch: 'other-focus' },
  { tick: 1000, retained: 995, epoch: 'other-focus' },
  { tick: 1020, retained: 995, epoch: 'other-focus', missingCurrent: true },
  { tick: 1040, retained: 1030, epoch: 'zoom-change' },
]
const history = operations.map(operation => {
  const sampleActors = tick => tick < operation.retained || (operation.missingCurrent && tick === operation.tick)
    ? null : { players: [{ id: 'local', position: { x: tick < 65 || tick >= 1000 ? 100 : 900, y: 150 } }],
      enemies: [], maggots: [], encounter: null }
  const sampleFocus = tick => sampleActors(tick)?.players[0].position ?? null
  const camera = operation.offCamera ? { x: 4000, y: 4000, w: 600, h: 600 } : cameraBounds
  const currentFrame = { actorPositions: [{ x: 100, y: 150 }], cameraBounds: camera }
  const frames = frameHistory.frame({ epoch: operation.epoch, tick: operation.tick, currentFrame,
    focusPlayerId: 'local', sampleActors, sampleFocus,
    actorPositions: poses => actors.nativeTreeActorPositions(poses, 'local'), cameraBounds: () => camera })
  const samples = []
  if (frames.history) {
    for (let tick = frames.history.fromTick; tick <= operation.tick; tick += 1) {
      samples.push({ tick, frame: frames.history.sample(tick) })
    }
  }
  const expected = presentation.update(operation.tick, frames.current, frames.history)[0]
  return { ...operation, current: frames.current, fromTick: frames.history?.fromTick ?? null,
    samples, expected: { alpha: expected.alpha, historyGapTicks: presentation.historyGapTicks,
      ...presentation.trees[0].state } }
})
const secondary = [-500.25, 0, 100.125, 4000.75].map(x => ({ root: { x, y: 251.125 },
  camera: cameraBounds, expected: trees.nativeTreeSecondaryPosition({ x, y: 251.125 }, cameraBounds) }))
writeFixture('tree-history-goldens.json', 'tools/website-oracle/tree-history-goldens.mjs',
  'Website@fa08ae249ff5563570bdb2a653e2ef9358ce7b83/client/boneyard-presentation-timeline.ts; renderer/boneyard-tree-{actors,frame-history,occlusion}.ts',
  { older, newer, poses, tree, bounds: trees.NATIVE_TREE_OCCLUSION_BOUNDS[0],
    polygon: trees.NATIVE_TREE_OCCLUSION_POLYGONS[0], history, secondary })
