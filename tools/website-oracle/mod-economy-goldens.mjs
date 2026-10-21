import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir,loadKernel,writeFixture } from './oracle.mjs'

const economy=await loadKernel('core-kernels/hub-economy.ts')
const boast=await loadKernel('core-kernels/boast.ts')
const npc=await loadKernel('core-kernels/native-hub-npc.ts')
const content=await loadKernel('modding/content/mod-content-catalog.ts')
const {PreparedModAssetCatalog}=await loadKernel('modding/assets/mod-asset-compiler.ts')
const fixture=JSON.parse(readFileSync(resolve(fixturesDir,'mod-content-goldens.json'),'utf8'))
const input=fixture.cases.find(row=>row.name==='all-sixteen-families')
const catalog=content.compileModContentCatalog(input.mods,new PreparedModAssetCatalog(input.assets))
const byKey=key=>catalog.all().find(row=>row.key===key)
const factories=[]
for(const [key,quantity] of [['token',1],['token',12],['token',13],['token',0],['robe',1],['robe',2]]) {
  let item=null,error=null
  try {item=content.modItemInventoryItem(byKey(key).catalog,quantity)} catch(caught) {error=caught.message}
  factories.push({key,quantity,item,error})
}
const potion=content.modConsumableInventoryItem(byKey('tonic').catalog)
const loot=[]
for(const seed of [0,1,2,17,77,123456789,9007199254740991]) {
  for(const boss of [false,true]) loot.push({seed,boss,items:catalog.createLootItems(seed,boss)})
}
const identities=[]
for(const [name,base,mutate] of [
  ['token',content.modItemInventoryItem(byKey('token').catalog),()=>{}],
  ['token-too-many',content.modItemInventoryItem(byKey('token').catalog),item=>item.quantity=13],
  ['wearable',content.modItemInventoryItem(byKey('robe').catalog),()=>{}],
  ['wearable-effects',content.modItemInventoryItem(byKey('robe').catalog),item=>item.nativeEffects=[]],
  ['wearable-missing-trim',content.modItemInventoryItem(byKey('robe').catalog),item=>delete item.modItemContent.iconTrimImagePath],
  ['wearable-wrong-path',content.modItemInventoryItem(byKey('robe').catalog),item=>item.modItemContent.wearable.wornImagePath='levels/wrong.png'],
]) {
  const item=structuredClone(base);mutate(item)
  identities.push({name,item,valid:economy.modItemInventoryIdentityIsValid(item)})
}
const affix={contentId:byKey('lucky').contentId,modId:'asset.test',name:'Lucky',modifiers:[{key:'mana_spend',operation:'multiply',value:0.5}]}
const foreign={...affix,contentId:'7000000000000000001',modId:'other.mod',name:'Other'}
let state=economy.createHubEconomy(77)
const steps=[]
function snapshot() { return {backpack:state.backpack,storage:state.storage,equipment:state.equipment,nextItemId:state.nextItemId,revision:state.revision,rng:state.rng,boast:state.npc.boast,valid:economy.hubEconomyInventoryIsValid(state)} }
function insert(label,item) {const result=economy.insertLootInventoryItem(state,item);state=result.state;steps.push({label,accepted:result.accepted,state:snapshot()})}
function action(label,result) {state=result.state;steps.push({label,accepted:result.accepted,state:snapshot()})}
insert('token-six',content.modItemInventoryItem(byKey('token').catalog,6))
insert('token-twelve',content.modItemInventoryItem(byKey('token').catalog,6))
insert('token-overflow',content.modItemInventoryItem(byKey('token').catalog,1))
insert('potion',potion)
insert('same-content-stale-subtype',{...potion,nativeSubtype:potion.nativeSubtype+17})
insert('robe-one',content.modItemInventoryItem(byKey('robe').catalog))
insert('robe-two',content.modItemInventoryItem(byKey('robe').catalog))
const robeId=state.backpack.find(item=>item.modItemContent?.key==='robe').id
insert('dye',economy.createFomentiusInventoryItem(economy.FOMENTIUS_STOCK_DEFINITIONS.find(row=>row.kind==='dye'),0,2))
const dyeId=state.backpack.find(item=>item.kind==='dye').id
const openedDye=economy.beginInventoryDyeSession(state,dyeId,'authored-paint')
action('open-authored-dye',openedDye)
action('dye-authored-clothing',economy.dyeInventoryClothing(state,openedDye.session,robeId,'trim',[1,8]))
action('reforge-authored-clothing',economy.reforgeModEquipment(state,robeId,[affix,foreign]))
action('reforge-reject-duplicate',economy.reforgeModEquipment(state,robeId,[affix,affix]))
action('equip-authored-clothing',economy.equipInventoryItem(state,robeId,'robe',{playerLevel:1,creativityRank:0}))
const potionId=state.backpack.find(item=>item.kind==='mod-potion').id
action('consume-authored-potion',economy.consumeInventoryItem(state,potionId))
const tokenId=state.backpack.find(item=>item.kind==='mod-item').id
action('transfer-authored-item',economy.transferInventoryItem(state,tokenId,'to-storage'))
state={...state,npc:{...state.npc,boast:{...boast.createBoastState(),selected:byKey('vow').selection}}}
state=economy.reconcileHubEconomyModPackages(state,['ASSET.TEST'])
steps.push({label:'reconcile-foreign-affix',accepted:true,state:snapshot()})
state=economy.reconcileHubEconomyModPackages(state,[])
steps.push({label:'reconcile-missing-owner',accepted:true,state:snapshot()})

const boasts=[]
const resolveBoast=selection=> {
  if(typeof selection==='number') return npc.resolveNativeBoast(selection)
  const definition=catalog.boast(selection.contentId)
  return definition?.modId===selection.modId ? definition : null
}
for(const selection of [3,byKey('vow').selection,byKey('painted-vow').selection]) {
  let state=boast.selectBoast(boast.createBoastState(),resolveBoast(selection))
  const events=[{type:'wave',wave:4},{type:'wave',wave:5},{type:'wave',wave:30},{type:'fail',producer:'potion-use'},{type:'fail',producer:'secondary-cast'}]
  const rows=[]
  for(const event of events) {
    state=event.type==='wave' ? boast.succeedBoast(state,event.wave,resolveBoast) : boast.failBoast(state,event.producer,resolveBoast)
    rows.push({event,state,score:boast.scoreBoast(16777217,state,resolveBoast),random:boast.boastUsesRandomSkillChoices(state,resolveBoast),failure:boast.boastFailureText(state,resolveBoast)})
  }
  boasts.push({selection,rows})
}
writeFixture('mod-economy-goldens.json','mod-economy-goldens.mjs','website-rust-current/frontend/src/game/core-kernels/{hub-economy,boast}.ts and modding/content/mod-content-catalog.ts',{
  sourceRevision:'fa08ae249ff5563570bdb2a653e2ef9358ce7b83',factories,potion,loot,identities,affix,foreign,steps,boasts})
