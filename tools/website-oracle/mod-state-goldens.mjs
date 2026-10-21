// Cells retain real Lua migration callbacks, Source schemas and byte budgets.
import {createRequire} from 'node:module'
import {resolve} from 'node:path'
import {loadKernel,websiteDir,writeFixture} from './oracle.mjs'
const require=createRequire(resolve(websiteDir,'frontend/package.json'))
const {WebLuaDefinitionRuntime}=await loadKernel('modding/definition/web-lua-definition-runtime.ts')
const {ModStateStore,schemaDefault,validateSchemaValue}=await loadKernel('modding/runtime/mod-state-store.ts')
const identity={id:'state.test',name:'State Test',version:'1.0.0'}
const code=`
sd.advanced.reducer{key="counter",scope="session",schema_version=1,on={"session.started"},state=sd.schema.object{
 count=sd.schema.integer{default=1,min=0,max=9},flag=sd.schema.boolean{},tag=sd.schema.string{default="é",max_bytes=6},
 items=sd.schema.array{item=sd.schema.enum{"a","b"},max_items=2}},reduce=function(s)return s,{}end}
sd.advanced.reducer{key="text",scope="session",schema_version=1,on={"session.started"},state=sd.schema.string{max_bytes=70000},reduce=function(s)return s,{}end}
sd.advanced.reducer{key="migrated",scope="session",schema_version=2,on={"session.started"},state=sd.schema.object{count=sd.schema.integer{default=0}},
 migrations={function(old)return {count=old.old+1}end},reduce=function(s)return s,{}end}
`
const runtime=await WebLuaDefinitionRuntime.create({entryScript:'scripts/main.lua',identity,wasmPath:require.resolve('wasmoon/dist/glue.wasm')})
runtime.run(code)
const definitions=new Map()
for(const key of ['counter','text','migrated']) {
 const row=runtime.reducer(key)
 definitions.set(key,{key,modId:identity.id,schema:row.state,schemaVersion:row.schemaVersion,scope:row.scope,migrations:row.migrations})
}
const migrated=definitions.get('migrated')
const old={...migrated,schemaVersion:1,migrations:{},schema:{kind:'schema-definition',schemaKind:'object',fields:{fields:{old:{kind:'schema-definition',schemaKind:'integer',fields:{default:2},source:{file:'old.lua',line:0,column:0}}}},source:{file:'old.lua',line:0,column:0}}}
definitions.set('old',old)
const store=new ModStateStore(),cells=new Map(),cases=[],scope={kind:'session',id:'host'}
function record(input) {
 let output=null,error=null
 try {switch(input.kind) {
  case 'cell': {const cell=store.cell({...definitions.get(input.definition),...(input.key?{key:input.key}:{})},input.scope);cells.set(input.name,cell);output=cell.get();break}
  case 'set':output=cells.get(input.name).set(input.value);break
  case 'get':output=cells.get(input.name).get();break
  case 'snapshot':output=store.snapshot(input.scopes);break
  case 'restore':store.restore(input.checkpoint,input.definitions.map(key=>definitions.get(key)));break
  case 'rollback':store.rollback(input.checkpoint);break
  case 'close-scope':output=store.closeScope(input.scope);break
  case 'default':output=schemaDefault(input.schema,input.field);break
  case 'validate':output=validateSchemaValue(input.schema,input.value,input.field);break
  case 'close':store.close();break
 }}catch(caught){error=caught.message}
 let checkpoint=null
 try {checkpoint=store.snapshot()}catch{}
 cases.push({input,output,error,checkpoint})
}
record({kind:'cell',name:'counter',definition:'counter',scope})
record({kind:'set',name:'counter',value:{count:3}})
record({kind:'set',name:'counter',value:{count:3}})
record({kind:'set',name:'counter',value:{count:10}})
record({kind:'set',name:'counter',value:{surprise:1}})
record({kind:'set',name:'counter',value:{count:4,tag:'éééé'}})
record({kind:'set',name:'counter',value:{count:4,items:['a','c']}})
record({kind:'set',name:'counter',value:{count:4,items:['a','b','a']}})
record({kind:'set',name:'counter',value:{count:4,flag:true,tag:'ok',items:['a']}})
const stable=store.snapshot()
record({kind:'cell',name:'other',definition:'counter',scope:{kind:'session',id:'other'}})
record({kind:'rollback',checkpoint:stable})
record({kind:'get',name:'other'})
record({kind:'snapshot',scopes:[scope]})
record({kind:'cell',name:'bad-scope',definition:'counter',scope:{kind:'participant-run',id:'player:run'}})
record({kind:'cell',name:'text-a',definition:'text',key:'text-a',scope})
record({kind:'cell',name:'text-b',definition:'text',key:'text-b',scope})
record({kind:'set',name:'text-a',value:'a'.repeat(30000)})
record({kind:'set',name:'text-b',value:'b'.repeat(30000)})
record({kind:'set',name:'text-a',value:'a'.repeat(36000)})
record({kind:'cell',name:'old',definition:'old',scope})
const oldCheckpoint=store.snapshot()
record({kind:'restore',checkpoint:oldCheckpoint,definitions:['counter','migrated']})
record({kind:'get',name:'old'})
record({kind:'cell',name:'changed',definition:'old',scope})
record({kind:'close-scope',scope})
record({kind:'get',name:'counter'})
for(const [schema,value] of [
 [{kind:'schema-definition',schemaKind:'boolean',fields:{}},2],
 [{kind:'schema-definition',schemaKind:'integer',fields:{min:2,max:4}},1],
 [{kind:'schema-definition',schemaKind:'number',fields:{min:0,max:1}},2],
 [{kind:'schema-definition',schemaKind:'string',fields:{max_bytes:2}},'éé'],
 [{kind:'schema-definition',schemaKind:'array',fields:{}},[]],
 [{kind:'schema-definition',schemaKind:'object',fields:{}},{}],
 [{kind:'schema-definition',schemaKind:'unsupported',fields:{}},null],
]) record({kind:'validate',schema:{...schema,source:{file:'schema.lua',line:0,column:0}},value,field:'value'})
record({kind:'close'})
record({kind:'get',name:'counter'})
const plain=[...definitions].map(([name,{migrations,...definition}])=>({name,definition:{...definition,migrationVersions:Object.keys(migrations).map(Number)}}))
runtime.close()
writeFixture('mod-state-goldens.json','mod-state-goldens.mjs','modding/runtime/mod-state-store.ts',{identity,code,definitions:plain,cases})
