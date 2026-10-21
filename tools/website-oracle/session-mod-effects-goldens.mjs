// Actual public Session lifecycle around retained authored effect rows.
import {loadKernel,writeFixture} from './oracle.mjs'
const game=await loadKernel('core-server/game-simulation.ts')
const {materializeStockTutorial}=await loadKernel('host/boneyard-catalog.ts')
const selection={discipline:'arcane',element:'ether',displayName:'Effects'}
const initial=()=>game.createGameSimulation({alpha:selection},{gameRngSeed:731})
const effects=[
 {color:[1,.5,0,1],contentId:'mod:short',expiresTick:1,playerId:'alpha',startedTick:0,useId:3},
 {color:[0,.5,1,.75],contentId:'mod:long',expiresTick:3,playerId:'alpha',startedTick:0,useId:4},
]
let state={...initial(),modEffects:effects,nextModConsumableUseId:5}
const rows=[]
const record=operation=>rows.push({operation,tick:state.tick,effects:state.modEffects,nextUseId:state.nextModConsumableUseId})
record('initial')
for(let i=0;i<4;i++){state=game.stepGameSimulationTick(state,{});record('step')}
state={...initial(),modEffects:effects,nextModConsumableUseId:5}
state=game.enterBoneyardWorld(state,materializeStockTutorial(Buffer.alloc(16)));record('enter')
state={...state,modEffects:effects}
state=game.returnGameSimulationToHub(state);record('return')
// Removal and partition retain effect lifetimes, even after their owner departs.
state={...initial(),modEffects:effects,nextModConsumableUseId:5}
state=game.partitionGameSimulationPlayers(state,[]).selected;record('partition-empty')
writeFixture('session-mod-effects-goldens.json','tools/website-oracle/session-mod-effects-goldens.mjs',
 'core-server/game-simulation.ts::{stepGameSimulationTick,enterBoneyardWorld,returnGameSimulationToHub,partitionGameSimulationPlayers}',{rows})
