// Health/mana authority, status ordering and the complete death-presentation lifetime.
import { loadKernel, writeFixture } from './oracle.mjs'

const combat = await loadKernel('core-kernels/player-combat.ts')
const lifeStates = combat.PLAYER_LIFE_STATES
function stateRow(state, tick) {
  return [state.currentHealth, state.currentMana, state.maximumHealth, state.maximumMana,
    state.circleSlowTicksRemaining, state.coldSlowTicksRemaining, state.dazzleTicksRemaining,
    state.deathAgeTicks, state.deathEpoch, state.deathTick, lifeStates.indexOf(state.lifeState),
    state.lastDamageTick ?? -1, state.poisonDamagePerTick, Number(state.poisonBeforeCold),
    state.poisonTicksRemaining, state.hitFeedback.strength, state.hitFeedback.tick,
    state.hitFeedback.timer, combat.playerMovementScale(state), combat.playerDisplayHealth(state),
    combat.playerHitOverlayAlpha(state, tick + 0.5), Number(combat.playerCollisionEnabled(state))]
}
const definitions = [
  { name: 'mana-and-status-order', actions: [
    { kind: 'damage', amount: 5, strength: 0.7, recordHit: true },
    { kind: 'step', ticks: 2 }, { kind: 'debit', cost: 6 }, { kind: 'debit', cost: 1000 },
    { kind: 'restore-mana', amount: 3 }, { kind: 'set-mana', amount: 1 },
    { kind: 'restore-health', amount: 1 },
    { kind: 'poison', damagePerSecond: 10, durationSeconds: 0.1 },
    { kind: 'cold', ticks: 8 }, { kind: 'dazzle', ticks: 50 }, { kind: 'circle', ticks: 4 },
    { kind: 'step', ticks: 10 }, { kind: 'step', ticks: 42 }, { kind: 'reset' },
  ] },
  { name: 'lethal-pending-death-and-respawn', actions: [
    { kind: 'damage', amount: 60, strength: 1, recordHit: true },
    { kind: 'step', ticks: 1 }, { kind: 'step', ticks: 254 }, { kind: 'step', ticks: 1 },
    { kind: 'step', ticks: 5 }, { kind: 'step', ticks: 5 }, { kind: 'step', ticks: 235 },
    { kind: 'step', ticks: 1 }, { kind: 'spectate' },
    { kind: 'damage', amount: 20, strength: 1, recordHit: true },
    { kind: 'poison', damagePerSecond: 10, durationSeconds: 3 }, { kind: 'respawn' },
  ] },
  { name: 'cold-before-poison-and-zero-health', actions: [
    { kind: 'cold', ticks: 2 }, { kind: 'poison', damagePerSecond: 25, durationSeconds: 0 },
    { kind: 'damage', amount: 50, strength: 1, recordHit: false },
    { kind: 'step', ticks: 1, healthRecovery: 0, manaRecovery: -0.25, manaCeiling: 60 },
    { kind: 'step', ticks: 3 }, { kind: 'reset' },
  ] },
]
const scenarios = []
for (const definition of definitions) {
  let state = combat.createPlayerCombat()
  let tick = 0
  const samples = []
  for (const action of definition.actions) {
    let flags = [0, 0, 0]
    let debitAccepted = null
    switch (action.kind) {
      case 'damage': state = combat.damagePlayer(state, action.amount, tick, action.recordHit, action.strength); break
      case 'debit': {
        const debit = combat.tryDebitPlayerMana(state, action.cost)
        state = debit.combat
        debitAccepted = debit.accepted
        break
      }
      case 'restore-health': state = combat.restorePlayerHealth(state, action.amount); break
      case 'restore-mana': state = combat.restorePlayerMana(state, action.amount); break
      case 'set-mana': state = combat.setPlayerMana(state, action.amount); break
      case 'poison': state = combat.poisonPlayer(state, action.damagePerSecond, action.durationSeconds); break
      case 'cold': state = combat.coldSlowPlayer(state, action.ticks); break
      case 'dazzle': state = combat.dazzlePlayer(state, action.ticks); break
      case 'circle': state = { ...state, circleSlowTicksRemaining: action.ticks }; break
      case 'reset': state = combat.resetPlayerCombatForNewRun(state); break
      case 'spectate': state = combat.setPlayerSpectating(state); break
      case 'respawn': state = combat.respawnPlayerCombat(state); break
      case 'step': {
        for (let update = 0; update < action.ticks; update += 1) {
          tick += 1
          const result = combat.stepPlayerCombatTick(state, { tick,
            healthRecoveryPerTick: action.healthRecovery,
            manaRecoveryPerTick: action.manaRecovery,
            manaCeiling: action.manaCeiling,
          })
          state = result.combat
          flags = [Number(result.beganDeathEpoch), Number(result.completedDeathPresentation), Number(result.emittedDeathBurst)]
        }
        break
      }
    }
    samples.push({ action, state: stateRow(state, tick), flags, debitAccepted })
  }
  scenarios.push({ name: definition.name, samples })
}
writeFixture('player-combat-goldens.json', 'tools/website-oracle/player-combat-goldens.mjs',
  'Website/frontend/src/game/core-kernels: player-combat.ts, native-puppet-hit.ts, boneyard-enemy-modifiers.ts', { scenarios })
