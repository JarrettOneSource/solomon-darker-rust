import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, writeFixture } from './oracle.mjs'
const {compileModContentCatalog}=await loadKernel('modding/content/mod-content-catalog.ts')
const {PreparedModAssetCatalog}=await loadKernel('modding/assets/mod-asset-compiler.ts')
const {stableWebLuaContentId}=await loadKernel('modding/definition/web-lua-content-identity.ts')
const {ModEnemyEngine}=await loadKernel('modding/content/mod-enemy-engine.ts')
const {ModPowerupEngine}=await loadKernel('modding/content/mod-powerup-engine.ts')
const {ModSceneEngine}=await loadKernel('modding/content/mod-scene-engine.ts')
const {ModPortalEngine}=await loadKernel('modding/content/mod-portal-engine.ts')
const source=JSON.parse(readFileSync(resolve(fixturesDir,'mod-content-goldens.json'))).cases[0]
const mods=structuredClone(source.mods),modId=mods[0].identity.id,id=key=>stableWebLuaContentId(modId,key)
const ref=(key,targetKind)=>({kind:'resolved-content-reference',contentId:id(key),modId,key,targetKind})
const room=structuredClone(mods[0].content.find(row=>row.key==='first'));room.key='second';room.contentId=id('second');mods[0].content.push(room)
mods[0].content.find(row=>row.key==='hall').fields.rooms.push(ref('second','room'))
const vault=structuredClone(mods[0].content.find(row=>row.key==='hall'));vault.key='vault';vault.contentId=id('vault');vault.fields.rooms=[ref('second','room'),ref('first','room')];mods[0].content.push(vault)
mods[0].content.find(row=>row.key==='extra').fields.features=[{kind:'rule-definition',operation:'prefab.portal',source:{file:'scripts/main.lua',line:1,column:0},fields:{selector:{object_kind:'monument'},destination:ref('hall','scene')}}]
const catalog=compileModContentCatalog(mods,new PreparedModAssetCatalog(source.assets))
const enemies=new ModEnemyEngine(catalog,100),powerups=new ModPowerupEngine(catalog),scenes=new ModSceneEngine(catalog),portals=new ModPortalEngine(catalog,scenes)
const cases=[]
function record(input) {
 let output=null,error=null
 try {switch(input.kind) {
  case 'enemy-spawn':output=enemies.spawn(input.contentId,input.x,input.y,input.tick);break
  case 'enemy-tick':output=enemies.tick({tick:input.tick,players:input.players,move:(_,requested)=>input.clamp?{x:Math.min(110,Math.max(0,requested.x)),y:Math.min(110,Math.max(0,requested.y))}:requested});break
  case 'enemy-damage':output=enemies.damage(input.id,input.amount,input.tick,input.sourcePlayerId);break
  case 'enemy-restore':enemies.restore(input.checkpoint);break
  case 'enemy-clear':enemies.clear();break
  case 'powerup-spawn':output=powerups.spawn(input.contentId,input.x,input.y,input.tick);break
  case 'candidates':output=powerups.candidates(input.players);break
  case 'collect':output=powerups.collect(input.id,input.playerId,input.tick);break
  case 'powerup-tick':output=powerups.tick(input.tick);break
  case 'powerup-restore':powerups.restore(input.checkpoint);break
  case 'powerup-clear':powerups.clear();break
  case 'enter':output=scenes.enter(input.ownerId,input.contentId);break
  case 'room':output=scenes.selectRoom(input.ownerId,input.roomIndex);break
  case 'return':output=scenes.return(input.ownerId);break
  case 'scene-restore':scenes.restore(input.checkpoint);break
  case 'portal':output=portals.activate(input);break
 }}catch(caught){error=caught.message}
 cases.push({input,output,error,enemies:enemies.checkpoint(),powerups:powerups.checkpoint(),collections:powerups.projectCollections(),scenes:scenes.checkpoint()})
}
record({kind:'enemy-spawn',contentId:id('mote'),x:100,y:100,tick:0})
record({kind:'enemy-spawn',contentId:id('mote'),x:105,y:105,tick:0})
record({kind:'enemy-tick',tick:1,players:[{id:'B',x:1000,y:100},{id:'a',x:1000,y:100}],clamp:false})
record({kind:'enemy-tick',tick:2,players:[{id:'a',x:150,y:100}],clamp:true})
record({kind:'enemy-tick',tick:25,players:[{id:'a',x:150,y:100}],clamp:false})
record({kind:'enemy-damage',id:1,amount:20,tick:26,sourcePlayerId:'B'})
record({kind:'enemy-damage',id:1,amount:200,tick:27,sourcePlayerId:null})
record({kind:'enemy-damage',id:1,amount:1,tick:28,sourcePlayerId:'a'})
record({kind:'enemy-tick',tick:76,players:[],clamp:false})
record({kind:'enemy-tick',tick:77,players:[],clamp:false})
record({kind:'enemy-restore',checkpoint:enemies.checkpoint()})
record({kind:'enemy-damage',id:2,amount:-1,tick:78,sourcePlayerId:null})
record({kind:'enemy-spawn',contentId:'missing',x:0,y:0,tick:78})
record({kind:'enemy-clear'})
record({kind:'enemy-spawn',contentId:id('mote'),x:0,y:0,tick:80})
record({kind:'powerup-spawn',contentId:id('orb'),x:0,y:0,tick:1})
record({kind:'candidates',players:[{id:'boundary',x:16,y:0},{id:'inside',x:15,y:0}]})
record({kind:'collect',id:999,playerId:'inside',tick:2})
record({kind:'collect',id:1,playerId:'inside',tick:2})
record({kind:'powerup-tick',tick:51})
record({kind:'powerup-tick',tick:52})
record({kind:'powerup-spawn',contentId:id('orb'),x:4,y:8,tick:53})
record({kind:'powerup-restore',checkpoint:powerups.checkpoint()})
record({kind:'powerup-clear'})
for(const ownerId of ['B','á','a'])record({kind:'enter',ownerId,contentId:id('hall')})
record({kind:'room',ownerId:'a',roomIndex:0})
record({kind:'room',ownerId:'a',roomIndex:1})
record({kind:'enter',ownerId:'a',contentId:id('vault')})
record({kind:'return',ownerId:'a'})
record({kind:'scene-restore',checkpoint:scenes.checkpoint()})
record({kind:'return',ownerId:'a'})
record({kind:'return',ownerId:'a'})
record({kind:'room',ownerId:'B',roomIndex:2})
record({kind:'enter',ownerId:'B',contentId:'missing'})
const portalId=portals.portals()[0].id
record({kind:'portal',actorKind:'monument',confirmedByLeader:false,ownerId:'party',portalId,scene:'stock.boneyard'})
record({kind:'portal',actorKind:'grave',confirmedByLeader:true,ownerId:'party',portalId,scene:'stock.boneyard'})
record({kind:'portal',actorKind:'monument',confirmedByLeader:true,ownerId:'party',portalId,scene:'stock.boneyard'})
for(let i=0;i<10;i++)record({kind:'enter',ownerId:'deep',contentId:id('hall')})
record({kind:'return',ownerId:'deep'})
writeFixture('mod-actors-goldens.json','mod-actors-goldens.mjs','modding/content/mod-enemy-engine.ts,mod-powerup-engine.ts,mod-scene-engine.ts,mod-portal-engine.ts',{catalog:catalog.all(),portals:portals.portals(),cases})
