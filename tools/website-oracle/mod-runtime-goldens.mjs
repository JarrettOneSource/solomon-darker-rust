// Actual Wasmoon execution, rather than manually constructed definition tokens.
import { createRequire } from 'node:module'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const require = createRequire(resolve(websiteDir, 'frontend/package.json'))
const wasmPath = require.resolve('wasmoon/dist/glue.wasm')
const { WebLuaDefinitionRuntime } = await loadKernel('modding/definition/web-lua-definition-runtime.ts')
const identity = { id: 'runtime.test', name: 'Runtime Test', version: '1.0.0' }
const sourceTest = readFileSync(resolve(websiteDir, 'frontend/src/game/modding/definition/web-lua-definition-runtime.test.ts'), 'utf8')
const census = sourceTest.split('const definition = runtime.run(`')[1].split('`)')[0]
const cases = []
function plainDefinition(value) {
  return JSON.parse(JSON.stringify({ ...value, reducers: value.reducers.map(({ callback, migrations, ...row }) => row) }))
}
async function record(name, code, options = {}) {
  const logs = []
  const input = { code, scripts: options.scripts ?? [], actions: options.actions ?? [] }
  const runtime = await WebLuaDefinitionRuntime.create({ entryScript: 'scripts/main.lua', identity, scripts: new Map(input.scripts), wasmPath, log: value => logs.push(value) })
  let definition = null
  let issues = []
  const actions = []
  try {
    try { definition = plainDefinition(runtime.run(code)) }
    catch (error) { issues = error.issues ?? [{ message: error.message }] }
    for (const action of input.actions) {
      try {
        let output
        if (action.kind === 'reduce') output = runtime.invokeReducer(action.key, action.state, action.event, action.context)
        else if (action.kind === 'migrate') output = runtime.invokeMigration(action.key, action.version, action.value)
        else if (action.kind === 'run') output = plainDefinition(runtime.run(action.code))
        actions.push({ output })
      } catch (error) { actions.push({ error: error.message }) }
    }
  } finally { runtime.close() }
  cases.push({ name, input, definition, issues, actions, logs, closed_memory_bytes: runtime.memoryBytes })
}
await record('complete-source-family-census', census, { actions: [
  { kind: 'reduce', key: 'counter', state: { count: 0, phase: 'normal' }, event: { kind: 'enemy.death' }, context: { entity: { health_ratio: 0.5 } } },
] })
await record('friendly-aliases-nested-creation-and-implicit-root', `
local ward = sd.status{name = "Kind Ward", duration = "1s"}
sd.potion{name = "My Tonic", icon = "art/tonic.png", status = ward}
sd.on("run.started", sd.all(sd.effect.state{key = "ready", value = true}, sd.after("1s", sd.effect.resource{target = "user", mana = 1})))`)
await record('all-rule-argument-and-schema-shapes', `
local a = sd.effect.state{key="a",value=1}
local b = sd.effect.state{key="b",value=2}
local c = sd.first(a, b)
local d = sd.every("1s", c, 2)
local e = sd.when({event="kind",equals="run.started"}, d, sd.effect.state{key="e",value=3})
sd.on("run.started", e)
sd.advanced.reducer{key="shapes",scope="session",schema_version=1,on={"session.started"},
 state=sd.schema.object{flag=sd.schema.boolean{default=false},n=sd.schema.number{default=1},label=sd.schema.string{default="x"},array=sd.schema.array{items=sd.schema.integer{default=0}},choice=sd.schema.enum{"a","b"}},
 reduce=function(state) return state, {sd.intent.state{key="seen",value=true}} end}`)
await record('include-cache-and-creation-site', `
local first = sd.include("scripts/items.lua")
local second = sd.include("scripts/items.lua")
assert(first == second)
return sd.mod{content={first}}`, { scripts: [['scripts/items.lua', 'print("included")\nreturn sd.item{key="token",name="Token"}']] })
await record('packed-include', `local token = sd.include("scripts/packed.lua")\nreturn sd.mod{content={token}}\n--@sd-bundle ${JSON.stringify({ 'scripts/packed.lua': 'return sd.item{key="packed",name="Packed"}' })}`)
await record('bundle-package-conflict', 'sd.mod{}\n--@sd-bundle {"scripts/one.lua":"return 2"}', { scripts: [['scripts/one.lua', 'return 1']] })
await record('invalid-packed-json', 'sd.mod{}\n--@sd-bundle {')
await record('include-missing', 'sd.include("scripts/itmes.lua")', { scripts: [['scripts/items.lua', 'return true']] })
await record('include-self', 'sd.include("scripts/main.lua")')
await record('include-cycle', 'sd.include("scripts/a.lua")', { scripts: [['scripts/a.lua','sd.include("scripts/b.lua")'],['scripts/b.lua','sd.include("scripts/a.lua")']] })
await record('include-syntax-error', 'sd.include("scripts/bad.lua")', { scripts: [['scripts/bad.lua', 'return sd.item({']] })
await record('unknown-namespace-member', 'sd.effct.state{}')
await record('unknown-global-with-suggestion', 'item{key="a"}')
await record('sandboxed-require', 'require("socket")')
await record('read-only-sdk', 'sd.item = function() end')
await record('read-only-globals', 'global_count = 1')
await record('invalid-key-type', 'sd.item{key=3,name="Bad"}')
await record('missing-content-key', 'sd.item{}')
await record('invalid-derived-content-key', 'sd.item{name="???"}')
await record('invalid-asset-reference', 'sd.art.ref(3)')
await record('invalid-content-reference-kind', 'sd.ref("itme", "token")')
await record('forged-root-node', 'sd.mod{content={{kind="content-definition",source={node=9}}}}')
await record('root-name-conflict', 'local a=sd.item{key="actual",name="A"}; sd.mod{content={wrong=a}}')
await record('unexpected-return', 'return true')
await record('empty-definition', 'local value=1')
await record('syntax-hint', 'return sd.item({key="bad"')
await record('rule-missing-nodes', 'sd.on("run.started")')
await record('reducer-event-rejection', 'sd.advanced.reducer{key="bad",scope="party-run",schema_version=1,state=sd.schema.object{},on={"run.start"},reduce=function(s)return s,{}end}')
await record('reducer-control-scope-rejection', 'sd.advanced.reducer{key="bad",scope="session",schema_version=1,state=sd.schema.object{},on={"player.control"},reduce=function(s)return s,{}end}')
await record('reducer-missing-migration', 'sd.advanced.reducer{key="bad",scope="session",schema_version=2,state=sd.schema.object{},on={"session.started"},reduce=function(s)return s,{}end}')
await record('reducer-callback-migration-and-null-payload', `
local counter=sd.advanced.reducer{key="counter",scope="party-run",schema_version=2,on={"enemy.death"},
 state=sd.schema.object{count=sd.schema.integer{default=0}},
 migrations={function(old) return {count=old.previous+1} end},
 reduce=function(state,event,context)
  assert(rawget(event,"absent")==nil)
  return {count=state.count+context.add}, {sd.intent.resource{target="user",mana=event.amount}}
 end}
return sd.mod{systems={counter}}`, { actions: [
  { kind: 'reduce', key: 'counter', state: { count: 2 }, event: { amount: 3, absent: null }, context: { add: 4 } },
  { kind: 'migrate', key: 'counter', version: 1, value: { previous: 7 } },
  { kind: 'migrate', key: 'counter', version: 2, value: {} },
  { kind: 'reduce', key: 'missing', state: {}, event: {}, context: {} },
  { kind: 'run', code: 'sd.mod{}' },
] })
await record('reducer-function-result-rejection', 'sd.advanced.reducer{key="bad",scope="session",schema_version=1,state=sd.schema.object{},on={"session.started"},reduce=function(s)return {bad=function()end},{}end}', { actions: [{ kind: 'reduce', key: 'bad', state: {}, event: {}, context: {} }] })
await record('reducer-cycle-rejection', 'sd.advanced.reducer{key="bad",scope="session",schema_version=1,state=sd.schema.object{},on={"session.started"},reduce=function(s)local a={};a.loop=a;return a,{}end}', { actions: [{ kind: 'reduce', key: 'bad', state: {}, event: {}, context: {} }] })
await record('explicit-lua-random-seed-is-private-and-supported', 'math.randomseed(123,456); local value=math.random(1,100000); sd.item{key="random",name=tostring(value)}; print(value)')
await record('definition-timeout', 'while true do end')
await record('callback-timeout', 'sd.advanced.reducer{key="slow",scope="session",schema_version=1,state=sd.schema.object{},on={"session.started"},reduce=function(s)while true do end end}', { actions: [{ kind: 'reduce', key: 'slow', state: {}, event: {}, context: {} }] })
writeFixture('mod-runtime-goldens.json', 'mod-runtime-goldens.mjs', 'modding/definition/web-lua-definition-runtime.ts', { identity, cases })
