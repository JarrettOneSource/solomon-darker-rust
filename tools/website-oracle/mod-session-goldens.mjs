// The prepared Source owner verifies actual Lua and rolls back rejected batches.
import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const require = createRequire(resolve(websiteDir, 'frontend/package.json'))
const wasmPath = require.resolve('wasmoon/dist/glue.wasm')
const { WebLuaDefinitionRuntime, compileWebLuaDefinition } = await loadKernel('modding/definition/index.ts')
const { prepareModSession } = await loadKernel('modding/runtime/prepared-mod-session.ts')
const { ModIntentExecutor } = await loadKernel('modding/runtime/mod-intent-executor.ts')
const identity = { id: 'prepared.test', name: 'Prepared Test', version: '1.0.0' }
const scope = { kind: 'party-run', id: 'run-1' }
const code = `
local state=sd.schema.object{count=sd.schema.integer{default=0,min=0,max=99}}
sd.advanced.reducer{key="counter",scope="party-run",schema_version=1,state=state,on={"enemy.death","action.shop.purchase"},
 reduce=function(s,e,c)return {count=s.count+1},{sd.intent.state{key="count",value=s.count+1}}end}
sd.advanced.reducer{key="casts",scope="participant-run",schema_version=1,state=sd.schema.integer{default=0},on={"action.content.cast"},
 reduce=function(s,e,c)return s+1,{sd.intent.state{key="cast",value={count=s+1,request=c.request_id,kind=c.action_kind}}}end}
sd.on("run.started",sd.after("10ms",sd.effect.state{key="timer",value=true}))
sd.potion{key="tonic",name="Tonic",duration="1s",on_use=sd.effect.resource{target="user",mana="full"}}
sd.spell{key="pulse",name="Pulse",slot="secondary",behavior=sd.prefab.area{radius=12,duration="1s",every="20ms",effects={sd.effect.damage{target="hostiles_in_area",amount=1}}}}
`
const runtime = await WebLuaDefinitionRuntime.create({ entryScript: 'scripts/main.lua', identity, wasmPath })
const compiled = compileWebLuaDefinition(identity, runtime.run(code))
runtime.close()
const source = { compiled, entryScript: code, entryScriptPath: 'scripts/main.lua', identity }
const scenarios = []
const clone = value => JSON.parse(JSON.stringify(value))

async function scenario(name, drive, clockStep = 0) {
  const calls = []
  let behavior = {}, now = 0
  const adapter = {
    prepare(intents, context) {
      calls.push({ phase: 'prepare', intents, context })
      if (behavior.prepareError) throw new Error(behavior.prepareError)
      return {
        commit() { calls.push({ phase: 'commit' }); if (behavior.commitError) throw new Error(behavior.commitError) },
        rollback(reason) { calls.push({ phase: 'rollback', reason }); if (behavior.rollbackError) throw new Error(behavior.rollbackError) },
      }
    },
  }
  const session = await prepareModSession({ adapter, mods: [source], wasmPath, now: () => { const value = now; now += clockStep; return value } })
  const executor = new ModIntentExecutor(adapter), cases = []
  function record(input) {
    let output = null, error = null
    try {
      switch (input.kind) {
        case 'behavior': behavior = input.value; break
        case 'step': output = session.step(input.input); break
        case 'act': output = session.act(input.input); break
        case 'restore': session.restore(input.checkpoint); break
        case 'project': output = session.project(input.viewerId); break
        case 'catalog': output = session.catalog(); break
        case 'close-run': session.closeRun(input.runId); break
        case 'close': session.close(); break
        case 'execute': output = executor.execute(input.intents, input.context); break
      }
    } catch (caught) { error = caught.message }
    let checkpoint = null, checkpointError = null
    try { checkpoint = session.checkpoint() } catch (caught) { checkpointError = caught.message }
    cases.push({ input, output, error, checkpoint, checkpointError, calls: clone(calls) })
    return output
  }
  const step = (event, tick, selectedScope = scope) => record({ kind: 'step', input: { events: event === null ? [] : [{ context: {}, event, payload: {}, scope: selectedScope }], tick } })
  await drive({ session, record, step })
  session.close()
  scenarios.push({ name, clockStep, cases })
}

await scenario('commits-actions-restores-and-content-identity-admission', ({ session, record, step }) => {
  step('enemy.death', 1)
  record({ kind: 'act', input: { action: 'shop.purchase', context: {}, payload: {}, requestId: 7, scope, tick: 2 } })
  record({ kind: 'project', viewerId: 'player-1' })
  record({ kind: 'catalog' })
  const stable = clone(session.checkpoint())
  const participantScope = { kind: 'participant-run', id: 'player-1:run-1' }
  for (const content of compiled.content) record({ kind: 'act', input: { action: content.contentKind === 'spell' ? 'content.cast' : 'content.use', context: { participant_id: 'player-1' }, payload: { content_id: content.contentId }, requestId: 8, scope: participantScope, tick: 4 } })
  record({ kind: 'restore', checkpoint: stable })
  record({ kind: 'restore', checkpoint: { ...stable, graphSha256: ['0'.repeat(64)] } })
  for (const payload of [null, {}, { content_id: '01' }, { content_id: '9'.repeat(20) }, { content_id: '1' }]) record({ kind: 'act', input: { action: 'content.use', context: {}, payload, requestId: 9, scope: participantScope, tick: 5 } })
  const tonic = compiled.content.find(content => content.contentKind === 'potion')
  record({ kind: 'act', input: { action: 'content.cast', context: {}, payload: { content_id: tonic.contentId }, requestId: 9, scope: participantScope, tick: 5 } })
  record({ kind: 'close-run', runId: 'run-1' })
  record({ kind: 'close' })
  record({ kind: 'catalog' })
})

await scenario('adapter-failures-restore-cells-and-cancel-failed-timers', ({ session, record, step }) => {
  record({ kind: 'behavior', value: { prepareError: 'preparation denied' } })
  step('enemy.death', 1)
  record({ kind: 'behavior', value: { commitError: 'inventory full', rollbackError: 'rollback also failed' } })
  step('enemy.death', 2)
  record({ kind: 'behavior', value: {} })
  step('enemy.death', 3)
  step('run.started', 4)
  const scheduled = clone(session.checkpoint())
  record({ kind: 'behavior', value: { commitError: 'scheduled rejected' } })
  step(null, 5)
  record({ kind: 'restore', checkpoint: scheduled })
  record({ kind: 'behavior', value: {} })
  step(null, 5)
  record({ kind: 'step', input: { events: [{ context: {}, event: 'enemy.death', payload: {}, scope }, { context: {}, event: 'enemy.death', payload: {}, scope: { kind: 'party-run', id: 'run-2' } }], tick: 6 } })
})

await scenario('event-budget-rolls-back-the-event-transaction', ({ step }) => step('enemy.death', 1), 10)

await scenario('executor-validates-before-prepare-and-preserves-commit-errors', ({ record }) => {
  const context = { context: {}, scope, tick: 1 }
  const intent = { fields: { key: 'x', value: 1 }, kind: 'state', modId: identity.id, owner: 'rule', scope, sequence: 1 }
  for (const intents of [[], [intent], [intent, intent], [{ ...intent, scope: { ...scope, id: 'other' } }], Array.from({ length: 257 }, (_, index) => ({ ...intent, sequence: index + 1 }))]) record({ kind: 'execute', intents, context })
  record({ kind: 'behavior', value: { prepareError: 'prepare failed' } })
  record({ kind: 'execute', intents: [intent], context })
  record({ kind: 'behavior', value: { commitError: 'commit failed', rollbackError: 'secondary failure' } })
  record({ kind: 'execute', intents: [intent], context })
})

const preparation = []
for (const input of [
  { ...source, compiled: { ...compiled, graphSha256: '0'.repeat(64) } },
  { ...source, entryScript: 'sd.item{key="different",name="Changed"}' },
]) {
  let error = null
  try {
    const session = await prepareModSession({ adapter: { prepare() { throw new Error('unreachable') } }, mods: [input], wasmPath })
    session.close()
  } catch (caught) { error = caught.message }
  preparation.push({ input, error })
}
writeFixture('mod-session-goldens.json', 'mod-session-goldens.mjs', 'modding/runtime/prepared-mod-session.ts;mod-intent-executor.ts', { source, scenarios, preparation })
