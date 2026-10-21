// Exercise the pinned receipt producer; JSON payloads and signatures are source outputs.
import { loadKernel, writeFixture } from './oracle.mjs'
const { createGameLeaderboardReceipt } = await loadKernel('host/game-leaderboard-receipt.ts')
const base = {
  accountUsername: null, awesomeness: 12345, awesomestKill: 'Skeleton',
  completedAtUtc: '2026-10-21T05:00:00.000Z', discipline: 'arcane', elapsedTicks: 10499,
  element: 'fire', headingIndex: 23, highestSkills: [{ skillId: 4, rank: 3 }, { skillId: 1, rank: 2 }],
  level: 7, monstersKilled: 51, perksUsed: [1, 7, 12], portraitScale: Math.fround(.93),
  runId: 'source-receipt', wave: 8, wizardName: 'Sirmin',
}
const secret = 'source-comparison-key-32-bytes-long'
const cases = []
for (const element of ['air', 'earth', 'ether', 'fire', 'water']) {
  for (const discipline of ['arcane', 'body', 'mind']) {
    const entry = { ...base, element, discipline, wizardName: '魔法 🧙 "\\\n',
      portraitScale: element === 'air' ? 1 : Math.fround(.85), awesomestKill: null }
    cases.push({ secret, userId: 2147483647, entry,
      receipt: createGameLeaderboardReceipt(secret, 2147483647, entry) })
  }
}
for (const [key, userId, patch] of [
  ['é'.repeat(16), 1, { highestSkills: [], perksUsed: [], portraitScale: 1 }],
  [secret, Number.MAX_SAFE_INTEGER, { awesomeness: 2000000000, elapsedTicks: 60480000 }],
  ['short', 1, {}], [secret, 0, {}], [secret, -1, {}],
]) {
  const entry = { ...base, ...patch }
  try { cases.push({ secret: key, userId, entry, receipt: createGameLeaderboardReceipt(key, userId, entry) }) }
  catch (error) { cases.push({ secret: key, userId, entry, error: error.message }) }
}
writeFixture('host-signing-goldens.json', 'tools/website-oracle/host-signing-goldens.mjs',
  'Website/frontend/src/game/host/game-leaderboard-receipt.ts', { cases })
