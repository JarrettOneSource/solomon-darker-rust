import { loadKernel, writeFixture } from './oracle.mjs'
const score = await loadKernel('core-kernels/hall-of-fame-score.ts')
const config = await loadKernel('core-kernels/boneyard-enemy-config.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const tokens = ['SKELETON', 'SKELETONARCHER', 'SKELETONMAGE', 'IMP', 'ZOMBIE', 'WRAITH',
  'DEMON', 'COFFIN', 'SPIDER', 'COCOON', 'DEMONSKULL', 'DIREFACULTY', 'HEARTMONGER']
const enemies = tokens.map(token => config.evaluateBoneyardEnemyConfig(token))
const names = enemies.map(enemy => score.nativeHallOfFameEnemyName(enemy))
const player = { currentHealth: 4.5, maximumHealth: 50, level: 1, scoreHealthMultiplierEnabled: true }
let rng = random.createNativeRng(17)
let state = score.createNativeHallOfFameRun(100)
const records = []
for (let tick = 0; tick < 450; tick += 1) {
  const enemyIndex = tick % enemies.length
  state = score.recordNativeHallOfFameEnemyKill(state, {
    enemy: enemies[enemyIndex], player, regionPulseAccumulator: tick % 3 ? 1 : 0.49,
  }, count => {
    const draw = random.drawNativeInteger(rng, count)
    rng = draw.state
    return draw.value
  })
  if ([0, 1, 12, 99, 100, 199, 200, 399, 400, 449].includes(tick)) records.push({ tick, state, rng })
}
state = score.resetNativeHallOfFameKillStreak(state)
state = score.archiveNativeHallOfFameRun(state, 900, 206.5, 0.9125)
const archived = state
const awards = []
for (const health of [-1, 4.9, 5.0000001, 50]) {
  for (const enabled of [false, true]) {
    const input = { ...player, currentHealth: health, scoreHealthMultiplierEnabled: enabled }
    awards.push({ player: input, expected: score.nativeHallOfFameAward(71, 0.5, input, 999) })
  }
}
writeFixture('score-goldens.json', 'tools/website-oracle/score-goldens.mjs',
  'Website/frontend/src/game/core-kernels/hall-of-fame-score.ts', { tokens, names, player, records, archived, awards })
