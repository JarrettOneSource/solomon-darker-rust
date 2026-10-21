import { loadKernel, writeFixture } from './oracle.mjs'

const activity = await loadKernel('host/game-activity-events.ts')
const game = await loadKernel('core-server/game-simulation.ts')
const initial = game.createGameSimulation({
  zeta: { displayName: 'Aurelia', discipline: 'arcane', element: 'fire' },
  alpha: { displayName: 'Octavia', discipline: 'body', element: 'water' },
}, { gameRngSeed: 17, combatRngSeed: 91 })
const projected = activity.projectGameActivity(initial)
const copy = value => structuredClone(value)
const active = { ...projected, runId: 'activity-run', phase: 'active', tick: 101, wave: { ordinal: 9 } }
const cases = []
const record = (name, previous, current) => cases.push({name, previous, current,
  events: activity.deriveGameActivityEvents(previous, current)})
record('stationary-hub', projected, projected)
record('new-run', projected, active)
let died = copy(active)
died.tick += 1
died.players[1].deathEpoch += 1
died.players[1].lifeState = 'dying'
died.players[1].currentHealth = -10
record('one-actual-death', active, died)
const both = copy(died)
both.tick += 1
both.players[0].deathEpoch += 2
both.players[0].lifeState = 'dying'
both.players[0].currentHealth = -10
both.players.reverse()
record('reordered-players-use-identities', died, both)
record('retained-death-epoch', both, {...both, tick: 105})
const over = { ...both, phase: 'game-over', tick: 106 }
record('actual-game-over', both, over)
record('game-over-ended', over, { ...projected, tick: 107 })
record('active-run-ended', active, { ...projected, tick: 108 })
record('new-dead-participant-has-no-old-epoch', active, {
  ...died, players: [{ ...died.players[1], playerId: 'new-player' }], tick: 109,
})
record('combined-order', { ...projected, phase: 'active' }, { ...over, runId: 'another-run', tick: 110 })
record('wave-absent', { ...active, wave: null }, { ...over, wave: null, tick: 111 })
writeFixture('game-activity-goldens.json', 'game-activity-goldens.mjs',
  'host/game-activity-events.ts (unchanged public project/derive)', { projected, cases })
