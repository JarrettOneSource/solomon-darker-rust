// Actual Lua-controller admission, leases, observation and stock skill selection.
import { loadKernel, writeFixture } from './oracle.mjs'
const simulation = await loadKernel('core-server/game-simulation.ts')
const { ModPlayerControl, selectModPlayerSkill, usesPlayerControlRule } = await loadKernel('host/mod-player-control.ts')
const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const loaded = { ...NATIVE_GENERATED_BONEYARDS[0], seed: 'controller-seed', runId: 'controller-run', choice: { id: 'default-random', source: 'default' } }
let state = simulation.createGameSimulation({ local: { displayName: 'Controller', element: 'fire', discipline: 'arcane' } })
state = simulation.enterBoneyardWorld(state, loaded)
const controller = new ModPlayerControl(), cases = []
const input = { aim: null, cast: { primary: false, quickbar: null }, movement: { x: 0, y: 0 }, viewportHeight: 768, viewportWidth: 1234 }
const context = { context: { participant_id: 'local' }, scope: { kind: 'participant-run', id: 'local:controller-run' }, tick: 0 }
const intent = fields => ({ fields, kind: 'input', modId: 'control.first', owner: 'bot', scope: context.scope, sequence: 1 })
let checkpoint = null
function record(action) {
  let output = null, error = null
  try {
    switch (action.kind) {
      case 'begin': output = controller.beginFrame(state, { local: input }, action.now); break
      case 'accept': controller.accept(state, action.intent, action.context ?? context); break
      case 'apply': { const inputs = { local: input }; controller.apply(inputs); output = inputs; break }
      case 'observe': output = controller.observe(state, action.playerId); break
      case 'save': checkpoint = controller.checkpoint(); break
      case 'restore': controller.restore(checkpoint); break
      case 'release': controller.release(action.playerId); break
      case 'clear': controller.clear(); break
      case 'xp': state = simulation.grantGameSimulationPlayerExperience(state, 'local', action.amount); break
      case 'select': state = selectModPlayerSkill(state, action.fields, action.context ?? context); break
      case 'uses': output = usesPlayerControlRule(action.value); break
    }
  } catch (caught) { error = caught.message }
  const offer = simulation.getPlayerProgression(state, 'local').pendingOffer
  cases.push({ input: action, output, error, leases: [...controller.checkpoint()].map(([playerId,lease]) => ({playerId,...lease})), offer })
}
record({ kind: 'begin', now: 0 })
record({ kind: 'observe', playerId: 'local' })
record({ kind: 'accept', intent: intent({ movement: { x: 0.6, y: 0.8 }, aim: { x: 90, y: 20 }, primary: true, quickbar: 0 }) })
record({ kind: 'apply' })
record({ kind: 'save' })
record({ kind: 'accept', intent: { ...intent({ release: true }), modId: 'other' } })
record({ kind: 'release', playerId: 'local' })
record({ kind: 'restore' })
for (const now of [99, 100, 249.999, 250]) { record({ kind: 'begin', now }); record({ kind: 'apply' }) }
for (const fields of [
  { movement: { x: 0, y: 0 }, extra: 1 },
  { release: true, movement: { x: 0, y: 0 } },
  { movement: { x: 1, y: 1 } },
  { movement: { x: 1.001, y: 0 } },
  { movement: { x: 0, y: 0, z: 0 } },
  { primary: 'true', movement: { x: 0, y: 0 } },
  { quickbar: 1.5, movement: { x: 0, y: 0 } },
  { quickbar: 8, movement: { x: 0, y: 0 } },
  { movement: { x: 0 } },
  { aim: 3, movement: { x: 0, y: 0 } },
  { movement: { x: 0, y: 0 }, primary: null, quickbar: null },
]) record({ kind: 'accept', intent: intent(fields) })
record({ kind: 'accept', intent: intent({ release: true }), context: { ...context, scope: { ...context.scope, id: 'stale' } } })
record({ kind: 'accept', intent: intent({ release: true }) })
record({ kind: 'select', fields: { choice_index: 0, offer_sequence: 0, skill_id: 16 } })
// Source advances only when experience is strictly above its 90-point threshold.
record({ kind: 'xp', amount: 91 })
record({ kind: 'observe', playerId: 'local' })
const offer = simulation.getPlayerProgression(state, 'local').pendingOffer
record({ kind: 'select', fields: { choice_index: 0, offer_sequence: offer.sequence + 1, skill_id: offer.options[0].skillId } })
record({ kind: 'select', fields: { choice_index: 0, offer_sequence: offer.sequence, skill_id: offer.options[0].skillId } })
record({ kind: 'observe', playerId: 'local' })
record({ kind: 'clear' })
record({ kind: 'apply' })
for (const value of [null, 1, { operation: 'rules.on', fields: { event: 'other' } }, { outer: [{ operation: 'rules.on', fields: { event: 'player.control' } }] }]) record({ kind: 'uses', value })
writeFixture('mod-player-control-goldens.json', 'mod-player-control-goldens.mjs', 'host/mod-player-control.ts', { loaded, cases })
