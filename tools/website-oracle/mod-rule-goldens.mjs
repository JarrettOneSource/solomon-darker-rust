// Real Source Lua reducers, immutable rule trees, timer checkpoints and circuits.
import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const require = createRequire(resolve(websiteDir, 'frontend/package.json'))
const wasmPath = require.resolve('wasmoon/dist/glue.wasm')
const { WebLuaDefinitionRuntime, compileWebLuaDefinition } = await loadKernel('modding/definition/index.ts')
const { ModRuleEngine } = await loadKernel('modding/runtime/mod-rule-engine.ts')
const scenarios = []
const scope = { kind: 'party-run', id: 'run-1' }
const clone = value => JSON.parse(JSON.stringify(value))

async function scenario(name, packages, drive, clockStep = 0) {
  let now = 0
  const engine = new ModRuleEngine({ now: () => { const value = now; now += clockStep; return value } })
  const mods = []
  for (const [id, code] of packages) {
    const identity = { id, name: id, version: '1.0.0' }
    const runtime = await WebLuaDefinitionRuntime.create({ entryScript: 'scripts/main.lua', identity, wasmPath })
    const compiled = compileWebLuaDefinition(identity, runtime.run(code))
    engine.register(compiled, runtime)
    // Timer node IDs belong to these actual compiled occurrences. Persist that
    // accepted graph, not a guessed Lua hash-table iteration order.
    mods.push({ identity, code, compiled })
  }
  const cases = []
  function record(input) {
    let output = null, error = null
    try {
      switch (input.kind) {
        case 'dispatch': output = engine.dispatch(input.input); break
        case 'due': output = engine.dueTimerIds(input.tick); break
        case 'fire': output = engine.fireTimer(input.id, input.tick); break
        case 'cancel': engine.cancelTimer(input.id); break
        case 'restore': engine.restore(input.checkpoint); break
        case 'rollback': engine.rollback(input.checkpoint); break
        case 'close-run': engine.closeRun(input.runId); break
        case 'close-mod': output = engine.closeMod(input.modId); break
        case 'close': engine.close(); break
      }
    } catch (caught) { error = caught.message }
    let checkpoint = null, checkpointError = null
    try { checkpoint = engine.checkpoint() } catch (caught) { checkpointError = caught.message }
    cases.push({ input, output, error, checkpoint, checkpointError, traces: engine.traces() })
    return output
  }
  const dispatch = (event, tick, context = {}, payload = {}, selectedScope = scope) => record({ kind: 'dispatch', input: { context, event, payload, scope: selectedScope, tick } })
  await drive({ engine, mods, record, dispatch })
  engine.close()
  scenarios.push({ name, mods, clockStep, cases })
}

const counters = `
local state = sd.schema.object{count=sd.schema.integer{default=0,min=0,max=99}}
local remembered
sd.advanced.reducer{key="good",scope="party-run",schema_version=1,state=state,on={"enemy.death"},reduce=function(s,e,c)
 local old=remembered and remembered("reward") or -1
 local a=c.random("reward");local b=c.random("reward");local other=c.random("other")
 remembered=c.random
 return {count=s.count+1},{sd.intent.state{key="rolls",value={a,b,other,old}},sd.intent.grant{item=sd.ref("item","token","rules.alpha")}}
end}
sd.advanced.reducer{key="bad",scope="party-run",schema_version=1,state=state,on={"enemy.death"},reduce=function(s,e,c)
 return {count=200},{sd.intent.state{key="never",value=true}}
end}
sd.on("enemy.death",sd.effect.state{key="rule",value=true})
`
await scenario('real-callbacks-named-streams-circuit-and-owner-order', [
  ['rules.zulu', counters],
  ['rules.alpha', 'sd.item{key="token",name="Token"};sd.on("enemy.death",sd.effect.state{key="alpha",value=true})'],
], ({ engine, record, dispatch }) => {
  for (let tick = 10; tick < 15; tick++) dispatch('enemy.death', tick, { participant_id: 'player-1' })
  const checkpoint = clone(engine.checkpoint())
  dispatch('enemy.death', 20)
  record({ kind: 'rollback', checkpoint })
  // VM upvalues are deliberately outside the canonical cell checkpoint, just
  // as Source retains the remembered old named-random callback above.
  dispatch('enemy.death', 20)
  dispatch('enemy.death', 21, {}, {}, { kind: 'session', id: 'host' })
  record({ kind: 'close-run', runId: 'run-1' })
  record({ kind: 'close-mod', modId: 'rules.zulu' })
  record({ kind: 'close-mod', modId: 'rules.zulu' })
  record({ kind: 'close' })
})

const trees = `
sd.on("run.started",sd.all(
 sd.after("30ms",sd.effect.state{key="after",value=true}),
 sd.every("20ms",sd.effect.state{key="every",value=true},2)))
sd.on("wave.completed",
 sd.when({context="wave",at_least=3},sd.effect.state{key="late",value=true}),
 sd.when({any={{context="wave",equals=1},{context="boss",equals=true}}},sd.effect.state{key="special",value=true}),
 sd.when({none={{context="wave",above=10}}},sd.effect.state{key="early",value=true}),
 sd.when({all={{context="wave",below=3},{context="label",not_equals="skip"}}},sd.effect.state{key="fresh",value=true}),
 sd.first(sd.when(false,sd.effect.state{key="never",value=true}),sd.effect.state{key="first",value=true},sd.effect.state{key="later",value=true}))
sd.potion{key="tonic",name="Tonic",duration="1s",on_use=sd.effect.resource{target="user",mana=4}}
sd.spell{key="pulse",name="Pulse",slot="secondary",behavior=sd.prefab.area{radius=12,duration="1s",every="20ms",effects={sd.effect.damage{target="enemies",amount=1}}}}
`
await scenario('timers-predicates-first-and-content-actions', [['rules.trees', trees]], ({ engine, mods, record, dispatch }) => {
  dispatch('run.started', 10, { participant_id: 'player-1' }, { run_id: 'run-1' })
  record({ kind: 'due', tick: 11 })
  record({ kind: 'fire', id: 1, tick: 11 })
  record({ kind: 'fire', id: 2, tick: 12 })
  const checkpoint = clone(engine.checkpoint())
  record({ kind: 'due', tick: 13 })
  record({ kind: 'fire', id: 1, tick: 13 })
  record({ kind: 'rollback', checkpoint })
  record({ kind: 'fire', id: 1, tick: 13 })
  record({ kind: 'fire', id: 2, tick: 40 })
  for (const context of [{ wave: 3 }, { label: 'skip', wave: 1 }, { boss: true, wave: 11 }, { label: 'go', wave: 2 }, {}]) dispatch('wave.completed', 50, context)
  for (const content of mods[0].compiled.content) {
    const action = content.contentKind === 'potion' ? 'use' : 'cast'
    dispatch(`action.content.${action}.${content.contentId}`, 60, {}, { content_id: content.contentId })
  }
})

await scenario('reducer-budget-is-checked-before-entry', [['rules.budget', counters.replaceAll('rules.alpha', 'rules.budget')]], ({ record, dispatch }) => {
  dispatch('enemy.death', 1)
  record({ kind: 'due', tick: 9_007_199_254_740_992 })
}, 10)

const migration = `
sd.advanced.reducer{key="counter",scope="party-run",schema_version=2,on={"enemy.death"},
 state=sd.schema.object{count=sd.schema.integer{default=0},phase=sd.schema.enum{"ready","done"}},
 migrations={function(old)return {count=old.count+1,phase="ready"}end},
 reduce=function(s)return {count=s.count+1,phase=s.phase},{sd.intent.state{key="migrated",value=s.count+1}}end}
sd.on("run.started",sd.after("0ms",sd.effect.state{key="wake",value=true}))
`
await scenario('migrations-and-exact-checkpoint-rejections', [['rules.restore', migration]], ({ engine, record, dispatch }) => {
  const empty = clone(engine.checkpoint())
  record({ kind: 'restore', checkpoint: { ...empty, state: { revision: 1, cells: [{ key: 'counter', modId: 'rules.restore', schemaVersion: 1, scope, value: { count: 4 } }] } } })
  dispatch('enemy.death', 10)
  dispatch('run.started', 10)
  const stable = clone(engine.checkpoint())
  for (const change of [
    value => { value.nextIntentSequence = 0 },
    value => { value.timers[0].nodeId = 'r01' },
    value => { value.timers.push({ ...value.timers[0] }) },
    value => { value.timers[0].intervalTicks = 0 },
    value => { value.reducerHealth[0].disabled = true },
    value => { value.reducerHealth = [] },
  ]) {
    const checkpoint = clone(stable); change(checkpoint)
    record({ kind: 'restore', checkpoint })
    record({ kind: 'restore', checkpoint: stable })
  }
  dispatch('run.started', 20, {}, {}, { kind: 'participant-run', id: 'player:run-1' })
  dispatch('run.started', 20, {}, {}, { kind: 'participant-run', id: 'player:run-2' })
  record({ kind: 'close-run', runId: 'run-1' })
  record({ kind: 'cancel', id: 3 })
})

const many = `
for i=1,70 do sd.advanced.reducer{key="r"..i,scope="session",schema_version=1,on={"session.started"},
state=sd.schema.integer{default=0},reduce=function(s)return s+1,{}end}end
`
await scenario('sixty-four-real-callback-invocation-budget', [['rules.many', many]], ({ dispatch }) => dispatch('session.started', 1, {}, {}, { kind: 'session', id: 'host' }))

writeFixture('mod-rule-goldens.json', 'mod-rule-goldens.mjs', 'modding/runtime/mod-rule-engine.ts', { scenarios })
