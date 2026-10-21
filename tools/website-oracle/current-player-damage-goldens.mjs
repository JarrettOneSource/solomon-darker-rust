#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
import { setup } from './player-entity-fixture.mjs'

const entities = await loadKernel('core-server/player-entity-store.ts')
const combat = await loadKernel('core-kernels/player-combat.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const projection = store => {
  const p = store.progressions[0]
  return { health: p.currentHealth, mana: p.currentMana, life: p.lifeState,
    death: [p.deathAgeTicks, p.deathEpoch, p.deathTick], corpseConsumed: p.corpseConsumed,
    collision: combat.playerCollisionEnabled(p), protection: [p.rescueProtection.fraction,
      p.rescueProtection.nextParticleId, p.rescueProtection.particles.length],
    hagatha: p.hagathaRuntime,
    potions: store.economies[0].backpack.filter(item => item.nativeTypeId === 7001)
      .map(item => [item.nativeSubtype, item.quantity]),
  }
}
const specs = []
for (const perks of [[], [7], [15], [15, 7], [24, 25], [7, 24, 25]]) {
  for (const blindness of [0, 5]) for (const fraction of [0, .75, 1]) {
    specs.push({ perks, blindness, fraction, amount: 100, scaled: false })
  }
}
specs.push({ perks: [24, 25], blindness: 0, fraction: 0, amount: 0, scaled: false },
  { perks: [24, 25], blindness: 0, fraction: 0, amount: 1, scaled: false },
  { perks: [7, 24, 25], blindness: 0, fraction: 1, amount: 100, scaled: true },
  { perks: [15, 7], blindness: 0, fraction: 0, amount: 100, scaled: false, removePotions: true })
const cases = specs.map(spec => {
  let store = setup({ perks: spec.perks, removePotions: spec.removePotions, combat: { currentHealth: 50, currentMana: 10 } })
  store = { ...store, progressions: store.progressions.map(p => ({ ...p,
    rescueProtection: { ...p.rescueProtection, fraction: spec.fraction } })),
    lightings: store.lightings.map(row => ({ ...row, blindnessTicksRemaining: spec.blindness })) }
  const result = entities.damagePlayerEntityWithResult(store, 'local', spec.amount, 201, spec.scaled, true, .75)
  store = result.store
  const flags = { autoHealthPotionUsed: result.autoHealthPotionUsed,
    cheatDeathTriggered: result.cheatDeathTriggered, hagathaCharmsLost: result.hagathaCharmsLost }
  const frames = [{ tick: 201, expected: projection(store) }]
  let rng = random.createNativeRng(917)
  for (let tick = 202; tick <= 204; tick += 1) {
    const stepped = entities.stepPlayerEntityCombatTick(store, rng, new Set(), {}, tick)
    store = stepped.store
    rng = stepped.rng
    frames.push({ tick, expected: projection(store) })
  }
  return { spec, flags, frames }
})

const core = []
for (const action of ['step', 'respawn', 'reset']) {
  for (const corpseConsumed of [false, true]) {
    const source = { ...combat.createPlayerCombat(), corpseConsumed, currentHealth: -50,
      lifeState: action === 'step' ? 'dying' : 'spectating', deathAgeTicks: 264, deathEpoch: 2,
      deathTick: 158, rescueProtection: { fraction: .75, nextParticleId: 1, particles: [] } }
    const result = action === 'step' ? combat.stepPlayerCombatTick(source, { tick: 300 }).combat
      : action === 'respawn' ? combat.respawnPlayerCombat(source) : combat.resetPlayerCombatForNewRun(source)
    core.push({ action, corpseConsumed, expected: { health: result.currentHealth, life: result.lifeState,
      death: [result.deathAgeTicks, result.deathEpoch, result.deathTick], corpseConsumed: result.corpseConsumed,
      fraction: result.rescueProtection.fraction, collision: combat.playerCollisionEnabled(result),
      burst: action === 'step' && combat.stepPlayerCombatTick(source, { tick: 300 }).emittedDeathBurst } })
  }
}
writeFixture('current-player-damage-goldens.json', 'current-player-damage-goldens.mjs',
  'core-server/player-entity-store.ts accepted damage/poison receiver and core-kernels/player-combat.ts',
  { sourceRevision, cases, core })
