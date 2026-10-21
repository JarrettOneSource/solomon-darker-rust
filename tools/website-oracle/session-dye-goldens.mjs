import { loadKernel, writeFixture } from './oracle.mjs'
const game = await loadKernel('core-server/game-simulation.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const economy = await loadKernel('core-kernels/hub-economy.ts')
const input = { movement: { x: 0, y: 0 }, aim: null, cast: { primary: false, quickbar: null }, viewport: { x: 1600, y: 900 } }
let state = game.createGameSimulation({ local: { displayName: 'Local', discipline: 'arcane', element: 'ether' },
  other: { displayName: 'Other', discipline: 'arcane', element: 'ether' } }, { gameRngSeed: 17 })
for (const [id, first] of [['local', 100], ['other', 200]]) {
  const current = game.getPlayerEconomy(state, id)
  state = { ...state, playerEntities: entities.replacePlayerEconomy(state.playerEntities, id, {
    ...current, backpack: [economy.createFomentiusInventoryItem(economy.FOMENTIUS_STOCK_DEFINITIONS[3], first, 2),
      { ...current.equipment.hat, id: first + 1 }, { ...current.equipment.robe, id: first + 2 }],
    equipment: { ...current.equipment, hat: null, robe: null },
  }) }
}
const open = (playerId, sessionId, dyeItemId = 100) => ({ playerId, action: { type: 'open-dye', sessionId, dyeItemId } })
const paint = (sessionId, rows, layer = 'cloth', targetItemId = 101) => ({ playerId: 'local', action: {
  type: 'dye', sessionId, swatchRows: rows, layer, targetItemId,
} })
const steps = [
  open('local', ''), open('local', 'first'), paint('first', []), paint('first', [1, 5, 9]),
  open('local', 'bad-open', 999), paint('stale', [5]), paint('first', [0, 17], 'trim'),
  { playerId: 'local', action: { type: 'close-dye', sessionId: 'stale' } },
  { playerId: 'local', action: { type: 'close-dye', sessionId: 'first' } },
  open('local', 'second'), open('other', 'independent', 200),
  { playerId: 'local', action: { type: 'rename-sack', itemId: 999, name: 'Rejected but closes painting' } },
  open('local', 'movement'), { input: { local: input } },
  { input: { local: { ...input, movement: { x: 1, y: 0 } } } },
  open('local', 'primary'), { input: { local: { ...input, cast: { primary: true, quickbar: null } } } },
  open('local', 'quickbar'), { input: { local: { ...input, cast: { primary: false, quickbar: 0 } } } },
  open('local', 'return'), { returnHub: true },
  open('local', 'remove'), open('other', 'survives-removal', 200), { remove: 'local' },
]
function snapshot() {
  return { sessions: state.inventoryDyeSessions, players: Object.fromEntries(state.playerEntities.identities.map(({ playerId }) => {
    const current = game.getPlayerEconomy(state, playerId), feedback = current.actionFeedback
    return [playerId, { backpack: current.backpack.map(item => ({ id: item.id, quantity: item.quantity, iconTints: item.iconTints ?? null })),
      feedback: feedback ? { accepted: feedback.accepted, action: feedback.action, sequence: feedback.sequence,
        reason: feedback.reason, dyeSessionId: feedback.dyeSessionId ?? null } : null }]
  })) }
}
const initial = snapshot(), rows = []
for (const step of steps) {
  let accepted = null, reason = null
  if (step.action) {
    const result = game.applyGameSimulationHubAction(state, step.playerId, step.action)
    state = result.state; accepted = result.accepted; reason = result.reason
  } else if (step.input) state = game.stepGameSimulationTick(state, step.input)
  else if (step.returnHub) state = game.returnGameSimulationToHub(state)
  else if (step.remove) state = game.removePlayerCharacter(state, step.remove)
  rows.push({ step, accepted, reason, expected: snapshot() })
}
writeFixture('session-dye-goldens.json', 'session-dye-goldens.mjs',
  'current core-server/game-simulation.ts public inventory/tick/return/removal; actual live two-player painting ownership', { initial, rows })
