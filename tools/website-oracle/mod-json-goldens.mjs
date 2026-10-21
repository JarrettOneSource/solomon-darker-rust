import { loadKernel, writeFixture } from './oracle.mjs'
const { compileWebLuaDefinition } = await loadKernel('modding/definition/web-lua-definition-compiler.ts')
const words=['a-b','a.b','a_b','a0','a1','a10','a2','aaa','A','a','ä','á','a\u0301','z','雷','é','e','É','E','_','-','.','!',' ']
const values=[1,1e-7,1e-6,1e20,1e21,-0,1.2345678901234567,Number.MIN_VALUE,Number.MIN_VALUE*2,Number.MAX_VALUE,-Number.MAX_VALUE,Number.MAX_SAFE_INTEGER,0.0000010000000000000002]
const numbers=values.map(value=>{
  const view=new DataView(new ArrayBuffer(8)); view.setFloat64(0,value)
  return {bits:view.getBigUint64(0).toString(),json:JSON.stringify(value)}
})
const graphs=[]
for(const [index,modifiers] of [
  {'a_b':1,'a-b':2,'a.b':3,'A':4,'a':5,'ä':6,'雷':7,'10':8,'2':9,'01':10},
  { small:1e-7,minimum:Number.MIN_VALUE,millionth:1e-6,large:1e21,integer:1,zero:-0 },
].entries()) {
  const compiled=compileWebLuaDefinition({id:`graph.test${index}`,name:'Graph tést',version:'1.0.0'}, {
    api:'1.0.0',assets:[],reducers:[],rules:[],content:[{kind:'content-definition',contentKind:'status',key:'status',fields:{modifiers},source:{file:'scripts/main.lua',line:10,column:0,node:3}}],
  })
  const { apiVersion,assets,content,identity,reducers,rules }=compiled
  graphs.push({graph:{apiVersion,assets,content,identity,reducers,rules},canonicalJson:compiled.canonicalJson,graphSha256:compiled.graphSha256})
}
writeFixture('mod-json-goldens.json','mod-json-goldens.mjs','modding/definition/web-lua-definition-compiler.ts',{
  locale:new Intl.Collator().resolvedOptions(),words,sorted:[...words].sort((a,b)=>a.localeCompare(b)),numbers,graphs,
})
