// Actual public Game ticks with the source's authored mutation callbacks.
import { loadKernel, writeFixture } from './oracle.mjs'
import { setup, snapshot } from './player-entity-fixture.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const specs = [
  { name:'zero-recovery-and-poison', combat:{currentHealth:30,currentMana:12,poisonTicksRemaining:5,poisonDamagePerTick:0.5}, manaScale:0, damageScale:0, ticks:7 },
  { name:'negative-recovery', combat:{currentMana:0.25}, manaScale:1, manaAdd:-0.75, damageScale:1, ticks:5 },
  { name:'recovery-and-meditation', combat:{currentMana:10}, ranks:[[58,3]], runtime:{concentrationSkillIdA:58,meditationIdleElapsedTicks:300}, manaScale:1.5, manaAdd:0.25, damageScale:1, ticks:5 },
  { name:'amplified-poison-with-drinker', perks:[15], combat:{currentHealth:5,currentMana:10,poisonTicksRemaining:3,poisonDamagePerTick:3}, manaScale:1, damageScale:2, ticks:4 },
  { name:'poison-filter-even-without-poison', combat:{currentHealth:20,currentMana:10}, manaScale:1, damageScale:1, damageAdd:2, ticks:3 },
  { name:'rejected-mana-budget', combat:{currentMana:10}, manaScale:1,manaAdd:1_000_001,damageScale:1,ticks:1 },
  { name:'rejected-poison-budget', combat:{currentMana:10}, manaScale:1,damageScale:1,damageAdd:1_000_001,ticks:1 },
  { name:'authored-mana-error', combat:{currentMana:10}, manaError:'Authored mana rule rejected this source.',ticks:1 },
  { name:'authored-damage-error', combat:{currentMana:10}, damageError:'Authored damage rule rejected this source.',manaScale:1,ticks:1 },
]
const cases=[]
for(const spec of specs) {
  let state=game.createGameSimulation({local:{displayName:'Oracle',element:'ether',discipline:'arcane'}},{gameRngSeed:17,combatRngSeed:917})
  state={...state,playerEntities:setup(spec)}
  const calls=[]
  const extensions={
    createLootItems:()=>{throw new Error('Hub resource tick cannot create loot')},
    filterMana:input=>{calls.push({kind:'mana',...input});if(spec.manaError)throw new Error(spec.manaError);return input.delta*spec.manaScale+(spec.manaAdd??0)},
    filterDamage:input=>{calls.push({kind:'damage',...input});if(spec.damageError)throw new Error(spec.damageError);return input.amount*spec.damageScale+(spec.damageAdd??0)},
    hasConsumable:()=>false,
  }
  const samples=[]
  for(let frame=1;frame<=spec.ticks;frame++) {
    let error=null
    try {state=game.stepGameSimulationTick(state,{local:{aim:null,cast:{primary:false,quickbar:null},movement:{x:0,y:0},viewportWidth:1600,viewportHeight:900}},{extensions})}
    catch(caught){error=caught.message}
    samples.push({frame,tick:state.tick,error,calls:calls.splice(0),player:snapshot(state.playerEntities),secondaryRng:state.secondaryAbilities.rng})
    if(error!==null)break
  }
  cases.push({spec,samples})
}
writeFixture('session-extensions-goldens.json','tools/website-oracle/session-extensions-goldens.mjs',
  'core-server/game-simulation.ts::stepGameSimulationTick authored filters and public error boundary',{cases})
