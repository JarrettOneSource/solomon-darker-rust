#!/usr/bin/env node
// Real population feedback at the wave director boundary, including all generated scripts.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const wave = await loadKernel('core-kernels/boneyard-wave-director.ts')
const boss = await loadKernel('core-kernels/native-survival-boss-encounter.ts')
const skeleton = await loadKernel('core-kernels/native-survival-skeleton-bosses.ts')
const { NATIVE_SURVIVAL_BOSS_SOURCES } = await loadKernel('core-kernels/native-survival-boss-catalog.ts')
const { nativePortalProgram } = await loadKernel('core-kernels/native-survival-portal.ts')
const { nativeSpiderWaveDefinitions } = await loadKernel('core-kernels/native-spider-wave-data.ts')

function project(source) {
  const fields = ['activeBurstIndex', 'activeGroupIndex', 'activeGroupMemberIndex', 'burstStarted',
    'burstSpawnRemaining', 'burstSpreadTicksRemaining', 'bossEncounters', 'interwaveDelayTicks',
    'lullThreshold', 'lowPopulationTicks', 'nextSpawnIntentId', 'nextScheduleIndex',
    'openingReleaseThreshold', 'pendingSpawnBudget', 'phase', 'portalPhaseIndex',
    'portalScriptPhase', 'portalSpawnRemaining', 'portalTicksRemaining', 'portalTimelinePaused',
    'populationThreshold', 'rngState', 'scheduleIndex', 'skeletonBosses', 'slumpgutPhase',
    'slumpgutPollCursor', 'slumpgutRecipeUid', 'slumpgutTicksRemaining', 'spawnCountdown',
    'spawnDelayTicks', 'spiderState', 'waveEventId', 'waveOrdinal']
  return Object.fromEntries(fields.map(field => [field, source[field]]))
}

function makeDirector(setup) {
  let state = wave.startBoneyardWaveDirector(wave.createBoneyardWaveDirector(setup.seed, undefined,
    setup.sourceSha256 ? { sourceSha256: setup.sourceSha256 } : {}))
  if (setup.kind === 'ordinary') return state
  // Park a completed graph while exercising one source program through its real public
  // director boundary. The population threshold of zero holds the ordinary graph in place.
  state = { ...state, phase: 'wave-threshold', populationThreshold: 0, lullThreshold: 0,
    activeBurstIndex: null, activeGroupIndex: null, burstStarted: false, burstSpawnRemaining: 0,
    burstSpreadTicksRemaining: 0, pendingSpawnBudget: 0, spawnCountdown: 0, spawnDelayTicks: 0,
    bossEncounters: [], skeletonBosses: null, spiderWaves: [], portalScriptPhase: 'retired',
    slumpgutPhase: 'retired', waveOrdinal: setup.waveOrdinal }
  if (setup.kind === 'boss') state = { ...state, bossEncounters: [boss.createNativeBossEncounter(setup.sourceSha256, setup.bossKind)] }
  if (setup.kind === 'skeleton') state = { ...state, skeletonBosses: skeleton.createNativeSkeletonBossProgram(setup.sourceSha256) }
  if (setup.kind === 'portal') state = { ...state, portalScriptPhase: 'idle', portalPhaseIndex: setup.phaseIndex }
  if (setup.kind === 'spider') state = { ...state, spiderWaves: [nativeSpiderWaveDefinitions(setup.sourceSha256)[setup.definitionIndex]] }
  if (setup.kind === 'slumpgut') state = { ...state, slumpgutPhase: 'eligible' }
  return state
}

function scenario(setup) {
  let state = makeDirector(setup)
  const initial = project(state)
  const records = []
  let lastTick = 0
  const bounds = { x: 0, y: 0, w: 2000, h: 3000 }
  const players = setup.noPlayers ? {} : { local: { position: { x: 700, y: 800 } }, second: { position: { x: 900, y: 1000 } } }
  for (let tick = 0; tick < setup.maximumTicks; tick++) {
    let liveEnemyCount = 0
    let liveBossCount = 0
    let liveZombieCount = 0
    if (setup.kind === 'ordinary') liveEnemyCount = tick % 1400 < 600 ? 20 : 0
    if (setup.kind === 'boss' || setup.kind === 'portal') liveBossCount = tick < 450 ? 1 : 0
    if (setup.kind === 'skeleton') {
      state = { ...state, waveOrdinal: tick === 0 ? setup.waveOrdinal : setup.foulshaftWaveOrdinal }
      liveBossCount = tick < 1200 ? 1 : 0
    }
    if (setup.kind === 'spider') liveEnemyCount = tick < 450 ? 2 : 0
    if (setup.kind === 'slumpgut') { liveEnemyCount = 76; liveZombieCount = 76 }
    const result = wave.stepBoneyardWaveDirector(state, { bounds, liveEnemyCount, liveBossCount,
      liveZombieCount, players, tick })
    let finished = false
    if (setup.kind === 'boss') finished = result.director.bossEncounters[0].phase === 'retired'
    if (setup.kind === 'skeleton') finished = result.director.skeletonBosses.foulshaftPhase === 'retired'
    if (setup.kind === 'portal') finished = result.director.portalPhaseIndex > setup.phaseIndex
    if (setup.kind === 'spider') finished = result.director.spiderState.phaseIndex === 1
    if (setup.kind === 'slumpgut') finished = result.director.slumpgutPhase === 'retired'
    if (tick % 100 === 0 || result.spawnIntents.length > 0 || result.director.phase !== state.phase
      || result.director.portalScriptPhase !== state.portalScriptPhase
      || result.director.slumpgutPhase !== state.slumpgutPhase || finished || tick + 1 === setup.maximumTicks) {
      records.push({ tick, state: project(result.director), spawnIntents: result.spawnIntents })
    }
    state = result.director
    lastTick = tick
    if (finished) break
  }
  if (setup.kind !== 'ordinary' && lastTick + 1 === setup.maximumTicks) {
    throw new Error(`survival script did not finish: ${JSON.stringify(setup)}`)
  }
  return { setup, initial, lastTick, records }
}

const cases = [
  scenario({ kind: 'ordinary', seed: 'ordinary-player', maximumTicks: 7000 }),
  scenario({ kind: 'ordinary', seed: 'ordinary-no-player', maximumTicks: 7000, noPlayers: true }),
  scenario({ kind: 'ordinary', seed: 'ordinary-native-programs', sourceSha256: NATIVE_SURVIVAL_BOSS_SOURCES[0].sourceSha256, maximumTicks: 14000 }),
]
for (const source of NATIVE_SURVIVAL_BOSS_SOURCES) {
  const sourceSha256 = source.sourceSha256
  const seed = 'generated-program'
  for (const bossKind of ['heartmonger', 'faculty', 'discorporeal']) {
    cases.push(scenario({ kind: 'boss', seed, sourceSha256, bossKind,
      waveOrdinal: source[bossKind].waveOrdinal, maximumTicks: 8000 }))
  }
  cases.push(scenario({ kind: 'skeleton', seed, sourceSha256, waveOrdinal: source.ironmaw.waveOrdinal,
    foulshaftWaveOrdinal: source.foulshaft.waveOrdinal, maximumTicks: 4000 }))
  cases.push(scenario({ kind: 'slumpgut', seed, sourceSha256, waveOrdinal: 1, maximumTicks: 4000 }))
  for (const [phaseIndex, phase] of nativePortalProgram(sourceSha256).phases.entries()) {
    cases.push(scenario({ kind: 'portal', seed, sourceSha256, phaseIndex, waveOrdinal: phase.startWave, maximumTicks: 2000 }))
  }
  for (const [definitionIndex, definition] of nativeSpiderWaveDefinitions(sourceSha256).entries()) {
    cases.push(scenario({ kind: 'spider', seed, sourceSha256, definitionIndex, waveOrdinal: definition.startWave, maximumTicks: 4000 }))
  }
}
writeFixture(process.env.SOLOMON_WAVE_DIRECTOR_FIXTURE ?? 'wave-director-goldens.json', 'tools/website-oracle/wave-director-goldens.mjs',
  'Website/frontend/src/game/core-kernels/boneyard-wave-director.ts and native-survival/native-spider programs',
  { sourceRevision, cases })
console.log(`recorded ${cases.length} complete wave-director scenarios`)
