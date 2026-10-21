import { loadKernel, writeFixture } from './oracle.mjs'
const {ModSpellEffectEngine}=await loadKernel('modding/content/mod-spell-effect-engine.ts')
const engine=new ModSpellEffectEngine(100),scope={id:'player:run',kind:'participant-run'},cases=[]
const damage={fields:{amount:3,target:'hostiles_in_area'},kind:'rule-definition',operation:'effect.damage'}
const resource={fields:{mana:2,target:'caster'},kind:'rule-definition',operation:'effect.resource'}
const base={contentId:'1001',modId:'effects.test',origin:{x:0,y:0},ownerPlayerId:'player',scope,target:{x:10,y:0},tick:0}
function record(input) {
 let output=null,error=null
 try {switch(input.kind) {
  case 'spawn':output=engine.spawn(input.input);break
  case 'tick':output=engine.tick({tick:input.tick,targets:input.targets,players:new Map(input.players.map(({id,x,y})=>[id,{x,y}]))});break
  case 'retire':output=engine.retire(input.id);break
  case 'restore':engine.restore(input.checkpoint);break
 }}catch(caught){error=caught.message}
 cases.push({input,output,error,checkpoint:engine.checkpoint()})
}
record({kind:'spawn',input:{...base,fields:{prefab:'area',duration:'100ms',every:'50ms',radius:20,effects:[damage,resource]}}})
record({kind:'tick',tick:0,players:[],targets:[{id:1,kind:'native-enemy',x:30,y:0},{id:2,kind:'native-enemy',x:31,y:0}]})
record({kind:'tick',tick:1,players:[],targets:[]})
record({kind:'tick',tick:5,players:[],targets:[]})
const saved=engine.checkpoint()
record({kind:'tick',tick:10,players:[],targets:[]})
record({kind:'restore',checkpoint:saved})
record({kind:'retire',id:1})
record({kind:'retire',id:999})
record({kind:'spawn',input:{...base,target:{x:0,y:0},fields:{prefab:'projectile',effects:[damage]}}})
record({kind:'spawn',input:{...base,target:{x:100,y:0},fields:{prefab:'projectile',radius:6,speed:1000,duration:'1s',effects:[damage]}}})
record({kind:'tick',tick:10,players:[],targets:[{id:3,kind:'native-enemy',x:50,y:0},{id:9,kind:'mod-enemy',x:50,y:0},{id:2,kind:'mod-enemy',x:50,y:0}]})
record({kind:'spawn',input:{...base,target:{x:100,y:0},fields:{prefab:'projectile',effects:[damage]}}})
record({kind:'tick',tick:20,players:[],targets:[]})
record({kind:'spawn',input:{...base,target:{x:100,y:0},fields:{prefab:'channel',width:5,duration:'1s',every:'100ms',effects:[damage]}}})
record({kind:'tick',tick:1,players:[{id:'player',x:5,y:0}],targets:[{id:7,kind:'native-enemy',x:50,y:5},{id:8,kind:'native-enemy',x:50,y:5.01}]})
record({kind:'tick',tick:2,players:[],targets:[]})
record({kind:'spawn',input:{...base,fields:{prefab:'area',radius:100,duration:'20ms',effects:[damage]}}})
record({kind:'tick',tick:0,players:[],targets:Array.from({length:257},(_,id)=>({id,kind:'mod-enemy',x:10,y:0}))})
record({kind:'tick',tick:2,players:[],targets:[]})
for(const fields of [
 {prefab:'bad',effects:[damage]},
 {prefab:'area',effects:[damage],surprise:true},
 {prefab:'area',effects:[damage],duration:'01s'},
 {prefab:'area',effects:[damage],duration:'25h'},
 {prefab:'area',effects:[damage],duration:86400001},
 {prefab:'area',effects:[damage],radius:0},
 {prefab:'projectile',effects:[damage],speed:0},
 {prefab:'area',effects:[]},
 {prefab:'area',effects:[{}]},
])record({kind:'spawn',input:{...base,fields}})
record({kind:'spawn',input:{...base,fields:{prefab:'area',effects:[damage]}}})
const checkpoint=engine.checkpoint()
record({kind:'restore',checkpoint:{...checkpoint,effects:[...checkpoint.effects,checkpoint.effects[0]]}})
record({kind:'restore',checkpoint:{...checkpoint,nextSequence:0}})
record({kind:'restore',checkpoint})
writeFixture('mod-effects-goldens.json','mod-effects-goldens.mjs','modding/content/mod-spell-effect-engine.ts',{cases})
