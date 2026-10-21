// Current Console next-wave admission borrows the actual Session and wave director.
import { readFileSync } from 'node:fs'
import { loadKernel, writeFixture } from './oracle.mjs'

const simulation = await loadKernel('core-server/game-simulation.ts')
const { applyWebLuaCommands, createWebLuaFrameState } = await loadKernel('host/lua/web-lua-game-api.ts')
const { createBoneyardWaveDirector } = await loadKernel('core-kernels/boneyard-wave-director.ts')
const { createNativeTutorialState } = await loadKernel('core-kernels/native-tutorial.ts')
const { NATIVE_SURVIVAL_BOSS_SOURCES } = await loadKernel('core-kernels/native-survival-boss-catalog.ts')
const { loaded } = JSON.parse(readFileSync(new URL('../../crates/sim/tests/fixtures/website/mod-host-game-goldens.json', import.meta.url)))
const sourceSha256 = NATIVE_SURVIVAL_BOSS_SOURCES[0].sourceSha256
// The Source regression uses this two-row signed NEXT schedule.
const schedule = [
  { groups: [{ entries: [{ enemy: 'SKELETON', flags: [] }] }], maxEnemies: 40, next: [1], spawn: 3, spawnDelay: [0, 0], waveDelay: [0, 0] },
  { groups: [{ entries: [{ enemy: 'ZOMBIE', flags: [] }] }], maxEnemies: 40, next: [-1], spawn: 2, spawnDelay: [0, 0], waveDelay: [0, 0] },
]
const waveFields = ['phase', 'scheduleIndex', 'waveOrdinal', 'waveEventId', 'pendingSpawnBudget',
  'spawnCountdown', 'spawnDelayTicks', 'interwaveDelayTicks', 'nextScheduleIndex', 'lullThreshold',
  'lowPopulationTicks', 'populationThreshold', 'activeBurstIndex', 'activeGroupIndex',
  'activeGroupMemberIndex', 'burstStarted', 'burstSpawnRemaining', 'burstSpreadTicksRemaining', 'rngState']
function snapshot(state) {
  const wave = state.world.kind === 'boneyard' ? state.world.waves : null
  return { waves: wave === null ? null : Object.fromEntries(waveFields.map(field => [field, wave[field]])),
    frame: createWebLuaFrameState(state, 'player-1', state.world.kind === 'boneyard' ? loaded : null).waves,
    gameRng: state.gameRng, tick: state.tick,
    enemyCount: state.world.kind === 'boneyard' ? state.world.enemies.actors.length : 0 }
}
const cases = []
for (const mode of ['opening-threshold', 'wave-threshold', 'wave-lull-delay', 'wave-lull',
  'interwave', 'preselected', 'duplicate', 'stale-run', 'stale-event', 'dormant', 'opening',
  'spawning', 'pending', 'portal-hold', 'spider-hold', 'heart-hold', 'faculty-hold',
  'discorporeal-hold', 'skeleton-reinforcements', 'skeleton-boss-wait', 'barrier', 'tutorial', 'game-over', 'hub']) {
  const hub = simulation.createGameSimulation({ 'player-1': { discipline: 'arcane', displayName: 'Tester', element: 'ether' } })
  let state = simulation.enterBoneyardWorld(hub, loaded)
  let waves = { ...createBoneyardWaveDirector('report58', schedule, { sourceSha256 }), phase: 'opening-threshold' }
  if (['wave-threshold', 'wave-lull-delay', 'wave-lull', 'interwave', 'preselected'].includes(mode)) {
    waves = { ...waves, phase: mode === 'preselected' ? 'interwave' : mode,
      scheduleIndex: 1, waveOrdinal: 9, waveEventId: 9, interwaveDelayTicks: 77,
      ...(mode === 'preselected' ? { nextScheduleIndex: 0 } : {}) }
  }
  if (['dormant', 'opening', 'spawning'].includes(mode)) waves = { ...waves, phase: mode }
  if (mode === 'pending') waves = { ...waves, pendingSpawnBudget: 1 }
  if (mode === 'portal-hold') waves = { ...waves, portalTimelinePaused: true }
  if (mode === 'spider-hold') waves = { ...waves, spiderState: { ...waves.spiderState, timelinePaused: true } }
  const bossIndex = ['heart-hold', 'faculty-hold', 'discorporeal-hold'].indexOf(mode)
  if (bossIndex >= 0) waves = { ...waves, bossEncounters: waves.bossEncounters.map((encounter, index) => index === bossIndex ? { ...encounter, phase: 'boss-wait' } : encounter) }
  if (mode.startsWith('skeleton-')) waves = { ...waves, skeletonBosses: { ...waves.skeletonBosses,
    foulshaftPhase: mode === 'skeleton-reinforcements' ? 'reinforcements' : 'boss-wait' } }
  state = { ...state, world: { ...state.world, waves } }
  if (mode === 'barrier') state = simulation.grantGameSimulationPlayerExperience(state, 'player-1', 1000)
  if (mode === 'tutorial') state = { ...state, world: { ...state.world,
    tutorial: createNativeTutorialState(loaded.scene.spawn, 0, loaded.seed) } }
  if (mode === 'game-over') state = { ...state, run: { ...state.run, phase: 'game-over' } }
  if (mode === 'hub') state = hub
  const request = { type: 'spawn-next-wave', runId: mode === 'stale-run' ? 'previous-run' : loaded.runId,
    waveEventId: waves.waveEventId + (mode === 'stale-event' ? 1 : 0) }
  const commands = mode === 'duplicate' ? [request, request] : [request]
  const initial = snapshot(state)
  const applied = applyWebLuaCommands(state, commands)
  cases.push({ mode, commands, initial, expected: snapshot(applied.state),
    output: { enemySpawnIntents: applied.enemySpawnIntents, nextRunSeed: applied.nextRunSeed } })
}
writeFixture('current-lua-wave-goldens.json', 'tools/website-oracle/current-lua-wave-goldens.mjs',
  'website-rust-current host/lua/web-lua-game-api.ts and core-kernels/boneyard-wave-director.ts',
  { sourceRevision: 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83', loaded, sourceSha256, schedule, cases })
