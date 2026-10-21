// The unchanged Source diagnostic recorder; each row exposes only the state fields it consumes.
import { loadKernel, writeFixture } from './oracle.mjs'
const { RunArchiveRecorder, runArchivePlayers } = await loadKernel('host/run-archive.ts')
const loaded = id => ({ runId: id, choice: { id: 'default-random', name: 'Random Boneyard', source: 'default' } })
const state = (tick, lives = ['alive', 'alive'], phase = 'active', kind = 'boneyard') => ({ tick, run: { phase }, world: { kind },
  playerEntities: { identities: ['alpha', 'beta'].map(playerId => ({ playerId })),
    configs: ['Alpha', 'Beta'].map(displayName => ({ displayName })), progressions: lives.map(lifeState => ({ lifeState })) } })
const content = { manifestSha256: 'a'.repeat(64), mods: [] }, archives = []
const recorder = new RunArchiveRecorder({ content, revision: 'b'.repeat(40), sessionId: 'archive-owner',
  write: archive => archives.push(archive), now: () => new Date('2026-10-21T00:00:00.000Z') })
const operations = [
  { op: 'checkpoint', run: 'first', state: { tick: 0 } },
  { op: 'observe', run: 'first', before: { tick: 0 }, after: { tick: 1, lives: ['lethal-pending', 'alive'] }, tickMs: 4, behindMs: 3, input: 1 },
  { op: 'checkpoint', run: 'first', state: { tick: 1, lives: ['lethal-pending', 'alive'] } },
  { op: 'observe', run: 'first', before: { tick: 1, lives: ['lethal-pending', 'alive'] }, after: { tick: 2, lives: ['dying', 'alive'] }, tickMs: 15, behindMs: 2, input: 2 },
  { op: 'observe', run: 'first', before: { tick: 2, lives: ['dying', 'alive'] }, after: { tick: 3, lives: ['dying', 'alive'] }, tickMs: 15, behindMs: 9, input: 3 },
  { op: 'observe', run: 'first', before: { tick: 3, lives: ['dying', 'alive'] }, after: { tick: 4, lives: ['dying', 'dying'], phase: 'game-over' }, tickMs: 12, behindMs: 1, input: 4 },
  { op: 'observe', run: 'world-ended', before: { tick: 10 }, after: { tick: 11, phase: 'hub', kind: 'hub' }, tickMs: 10, behindMs: 0, input: 5 },
  { op: 'checkpoint', run: 'retained', state: { tick: 20 } },
  { op: 'checkpoint', run: 'closed', state: { tick: 30 } },
  { op: 'retain', active: ['closed'] }, { op: 'close', reason: 'host-closed' },
  { op: 'checkpoint', run: 'failed', state: { tick: 40 } }, { op: 'close', reason: 'server-error' },
]
function from(spec) { return state(spec.tick, spec.lives, spec.phase, spec.kind) }
for (const operation of operations) {
  if (operation.op === 'checkpoint') recorder.checkpoint(from(operation.state), loaded(operation.run))
  else if (operation.op === 'observe') recorder.observe({ before: from(operation.before), after: from(operation.after),
    loadedBoneyard: loaded(operation.run), inputs: { alpha: { movement: { x: operation.input, y: 0 } } }, enemySpawnIntents: [], tickMs: operation.tickMs, behindMs: operation.behindMs })
  else if (operation.op === 'retain') recorder.retain(new Set(operation.active))
  else recorder.close(operation.reason)
}
function checkpoint(value) { return value ? { tick: value.state.tick, movement: value.inputs.alpha?.movement ?? null, tickMs: value.tickMs, spawns: value.enemySpawnIntents.length } : null }
function projection(archive) { return { runId: archive.runId, sessionId: archive.sessionId, revision: archive.revision,
  startedAtUtc: archive.startedAtUtc, endedAtUtc: archive.endedAtUtc, endReason: archive.endReason,
  lastAlive: checkpoint(archive.lastAlive), byPlayer: [...archive.lastAliveByPlayer].map(([id, row]) => [id, checkpoint(row)]),
  worst: checkpoint(archive.worstTick), finalTick: archive.finalState.tick, players: runArchivePlayers(archive), performance: archive.performance } }
const expected = archives.map(projection)
for (let tick = 0; tick < 6001; tick++) recorder.observe({ before: state(tick), after: state(tick + 1), loadedBoneyard: loaded('window-retention'),
  inputs: {}, tickMs: tick % 3 === 0 ? 10.5 : 2, behindMs: tick % 9 })
recorder.close()
writeFixture('run-archive-goldens.json', 'run-archive-goldens.mjs', 'current host/run-archive.ts public recorder: actual living states, strict worst tie, lifecycle/retention and 60x100-tick performance windows',
  { operations, expected, windows: projection(archives.at(-1)) })
