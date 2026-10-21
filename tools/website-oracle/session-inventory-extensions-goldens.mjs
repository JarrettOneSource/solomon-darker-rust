// Canonical public inventory transactions, including the prepared-owner return value.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, writeFixture, fixturesDir } from './oracle.mjs'
import { setup, snapshot } from './player-entity-fixture.mjs'

const game=await loadKernel('core-server/game-simulation.ts')
const entities=await loadKernel('core-server/player-entity-store.ts')
const content=await loadKernel('modding/content/mod-content-catalog.ts')
const hostFixture=JSON.parse(readFileSync(resolve(fixturesDir,'hosted-live-goldens.json'),'utf8'))
const catalog=hostFixture.rows.find(row=>row.case==='package-lua-catalog').catalog[0]
const selection={kind:'mod',modId:'hosted.content',contentId:'123'}
const definition={selection,label:'Careful',instruction:'Avoid potions',statement:'I will abstain',
  failureProducers:['potion-use','mana-underflow'],randomSkillChoices:true,scoreMultiplier:1.75,successWave:2}
const specs=[
  {name:'absent-prepared-consumable',mod:true,admitted:false,repeats:1},
  {name:'accepted-without-vfx',mod:true,admitted:true,repeats:1},
  {name:'same-owner-content-replaces-vfx',mod:true,admitted:true,repeats:2,vfx:true,durationMs:25.25},
  {name:'zero-duration-vfx-still-lives-one-tick',mod:true,admitted:true,repeats:1,vfx:true,durationMs:0},
  {name:'stock-mana-suppressed',manaScale:0,repeats:1},
  {name:'stock-mana-amplified',manaScale:2,repeats:1},
  {name:'stock-mana-negative',manaScale:-1,repeats:1},
  {name:'stock-mana-invalid',manaScale:1,manaAdd:1_000_001,repeats:1},
  {name:'authored-boast-unknown',boast:true,admitted:false,repeats:1},
  {name:'authored-boast-potion-failure',boast:true,admitted:true,repeats:1},
]
const cases=[]
for(const spec of specs) {
  let state=game.createGameSimulation({local:{displayName:'Oracle',element:'ether',discipline:'arcane'}},{gameRngSeed:17})
  state={...state,playerEntities:setup({combat:{currentMana:10}})}
  const acceptedCatalog=structuredClone(catalog)
  if(spec.vfx)acceptedCatalog.content={...acceptedCatalog.content,durationMs:spec.durationMs,consumeVfx:{kind:'spell_glow',color:[0.1,0.2,0.3,0.4]}}
  if(spec.mod)for(let index=0;index<spec.repeats;index++) {
    const added=entities.insertPlayerEntityLootItem(state.playerEntities,'local',content.modConsumableInventoryItem(acceptedCatalog))
    if(!added.accepted)throw new Error('source fixture inserts a real addressed potion')
    state={...state,playerEntities:added.store}
  }
  const calls=[]
  const extensions={createLootItems:()=>[],filterDamage:input=>input.amount,
    filterMana:input=>{calls.push({kind:'mana',...input});return input.delta*(spec.manaScale??1)+(spec.manaAdd??0)},
    hasConsumable:id=>spec.admitted===true&&id===acceptedCatalog.content.contentId,
    resolveBoast:requested=>spec.admitted&&requested.kind===selection.kind&&requested.modId===selection.modId&&requested.contentId===selection.contentId?definition:null}
  const steps=[]
  const transact=action=>{
    let result=null,error=null
    try{result=game.applyGameSimulationHubAction(state,'local',action,extensions);state=result.state}
    catch(caught){error=caught.message}
    steps.push({action,error,accepted:result?.accepted??null,reason:result?.reason??null,
      modConsumption:result?.modConsumption??null,calls:calls.splice(0),player:snapshot(state.playerEntities),
      modEffects:state.modEffects,nextUseId:state.nextModConsumableUseId})
  }
  if(spec.boast)transact({type:'select-boast',boastId:selection})
  if(!spec.boast||spec.admitted)for(let index=0;index<spec.repeats;index++) {
    const item=state.playerEntities.economies[0].backpack.find(item=>spec.mod
      ?item.modContent?.contentId===acceptedCatalog.content.contentId
      :item.nativeTypeId===7001&&item.nativeSubtype===1)
    transact({type:'consume',itemId:item.id})
  }
  cases.push({spec,catalog:acceptedCatalog,steps})
}
writeFixture('session-inventory-extensions-goldens.json','tools/website-oracle/session-inventory-extensions-goldens.mjs',
  'core-server/game-simulation.ts::applyGameSimulationHubAction authored admission, filters, Boasts and post-transaction consumption',{selection,cases})
