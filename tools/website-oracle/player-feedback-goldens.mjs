import { loadKernel, writeFixture } from './oracle.mjs'
const crow = await loadKernel('core-kernels/native-crow-blindness.ts')
const status = await loadKernel('core-server/boneyard-player-status.ts')
const sounds = await loadKernel('core-server/enemies/events.ts')
const rng = await loadKernel('core-kernels/native-rng.ts')
const blindness = []
for (const seed of [0, 1, 77, 917, 123456789]) {
  let current = rng.createNativeRng(seed)
  for (const currentHealth of [0, 10, 10.01, 50]) for (const remainingShield of [0, 0.01]) {
    for (const chancePercent of [0, 1, 50, 100]) {
      const before = current
      const result = crow.nativeCrowBlindness(currentHealth, remainingShield, chancePercent, current)
      current = result.rng
      blindness.push({ seed, currentHealth, remainingShield, chancePercent, before, result })
    }
  }
}
const opacity = [-10, 0, 0.01, 0.5, 1, 17.2345, 50, 99.999, 100, 234.5].map(ticksRemaining => ({
  ticksRemaining, result: crow.nativeCrowBlindnessOpacity(ticksRemaining),
}))
const bursts = []
for (const seed of [0, 1, 77, 917]) for (const kind of ['cold', 'poison']) {
  for (const position of [{ x: 0, y: 0 }, { x: 319.125, y: -211.75 }]) {
    const request = { actorId: 16, playerId: 'player:0', position, status: kind, tick: 201 }
    const result = status.emitPlayerStatusBurst({ deathEffects: [], nextDeathEffectId: 71, nextEventId: 83 }, request, rng.createNativeRng(seed))
    bursts.push({ seed, request, result })
  }
}
const ouches = []
for (const seed of [0, 1, 77, 917]) for (const currentHealth of [0, 10, 25, 25.01, 30, 35, 44.99, 45, 100]) {
  for (const tick of [0, 201, 16777217]) {
    const request = { actorId: 16, playerId: 'player:0', position: { x: 319.125, y: -211.75 }, currentHealth, tick }
    const result = sounds.emitBoneyardPlayerDamageSound({ nextEventId: 83 }, request, rng.createNativeRng(seed))
    ouches.push({ seed, request, result,
      readiness: [-1, 0, 1].map(delta => sounds.nativeWizardOuchCooldownReady(result.deadlineTick + delta, result.deadlineTick)) })
  }
}
writeFixture('player-feedback-goldens.json', 'tools/website-oracle/player-feedback-goldens.mjs',
  'Website native-crow-blindness.ts, boneyard-player-status.ts and enemies/events.ts (pinned ed0a2d59)',
  { blindness, opacity, bursts, ouches })
