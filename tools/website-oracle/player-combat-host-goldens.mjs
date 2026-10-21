import { loadKernel, writeFixture } from './oracle.mjs'
import { setup, snapshot } from './player-entity-fixture.mjs'
const entities = await loadKernel('core-server/player-entity-store.ts')
const nativeRng = await loadKernel('core-kernels/native-rng.ts')
const damages=[]
for (const perks of [[],[15],[7],[15,7],[24,25],[16]]) for (const removePotions of [false,true]) {
  for (const amount of [0,9,60,90]) {
    const spec={perks,removePotions,combat:{currentHealth:50,currentMana:10}}
    const store=setup(spec)
    const result=entities.damagePlayerEntityWithResult(store,'local',amount,201,false,amount!==9,0.75)
    damages.push({spec,amount,tick:201,damageAlreadyScaled:false,recordHit:amount!==9,hitStrength:0.75,
      flags:{autoHealthPotionUsed:result.autoHealthPotionUsed,cheatDeathTriggered:result.cheatDeathTriggered},expected:snapshot(result.store)})
  }
}
const debits=[]
for (const perks of [[],[15]]) for (const removePotions of [false,true]) for (const lifeState of ['alive','dying']) {
  for (const cost of [0,20,100,101]) {
    const spec={perks,removePotions,combat:{currentMana:10,lifeState}}
    const result=entities.tryDebitPlayerEntityMana(setup(spec),'local',cost)
    debits.push({spec,cost,flags:{accepted:result.accepted,autoManaPotionUsed:result.autoManaPotionUsed},expected:snapshot(result.store)})
  }
}
const specs=[
  {name:'recovery',combat:{currentHealth:20,currentMana:10},ticks:200},
  {name:'poison-and-cold',combat:{currentHealth:1,currentMana:10,poisonTicksRemaining:90,poisonDamagePerTick:0.5,coldSlowTicksRemaining:50,poisonBeforeCold:false},ranks:[[53,3],[68,5]],ticks:100},
  {name:'empty-poison-flash',combat:{currentHealth:0,currentMana:10,poisonTicksRemaining:90,poisonDamagePerTick:0},ranks:[[53,3]],ticks:5},
  {name:'last-word-death',perks:[12],combat:{currentHealth:-10,lifeState:'lethal-pending'},ticks:510},
  {name:'dying-potion-clocks',combat:{currentHealth:-11,lifeState:'dying'},potionClocks:{damageX4TicksRemaining:3,mindChugTicksRemaining:3,poisonImmunityTicksRemaining:3},ticks:510},
  {name:'meditation',combat:{currentMana:10},ranks:[[58,3]],runtime:{concentrationSkillIdA:58},ticks:650,manaCeiling:75},
  {name:'harden-water',primaryId:32,ranks:[[32,1],[36,3]],primaryCast:{held:true,channelActive:true,actionTick:0.25},ticks:650},
  {name:'weak-water',primaryId:32,ranks:[[32,1],[36,3]],primaryCast:{held:true,channelActive:true,underpowered:true,actionTick:0.25},ticks:250},
  {name:'hurricane-air',primaryId:24,ranks:[[24,1],[28,3]],runtime:{hurricaneCharge:1,hurricaneRefreshed:true},primaryCast:{held:true,channelActive:true,actionTick:0.25},ticks:250},
  {name:'moving-staff',ranks:[[58,3]],runtime:{concentrationSkillIdA:58,meditationIdleElapsedTicks:300},velocity:{x:0.05,y:0},actingStaff:true,ticks:200},
  {name:'poison-reverie',perks:[24,25],combat:{currentHealth:10,poisonTicksRemaining:3,poisonDamagePerTick:2},ticks:5},
]
const journeys=[]
for (const spec of specs) {
  let store=setup(spec),rng=nativeRng.createNativeRng(917)
  const samples=[]
  for(let tick=1;tick<=spec.ticks;tick++) {
    const result=entities.stepPlayerEntityCombatTick(store,rng,new Set(spec.actingStaff?['local']:[]),
      spec.manaCeiling===undefined?{}:{manaCeiling:()=>spec.manaCeiling},tick)
    store=result.store;rng=result.rng
    const flags=Object.fromEntries(Object.entries(result).filter(([key])=>key!=='store'&&key!=='rng'))
    if(tick<=5||tick%25===0||tick>=spec.ticks-2||Object.values(flags).some(value=>value.length>0)) samples.push({tick,flags,expected:snapshot(store),rng})
  }
  journeys.push({spec,samples})
}
writeFixture('player-combat-host-goldens.json','tools/website-oracle/player-combat-host-goldens.mjs',
  'Website core-server/player-entity-store.ts damage/mana/stepPlayerEntityCombatTick (pinned ed0a2d59)',{damages,debits,journeys})
