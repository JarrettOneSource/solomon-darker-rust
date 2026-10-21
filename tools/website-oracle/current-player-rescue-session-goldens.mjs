#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const configs = { owner:{discipline:'arcane',displayName:'Owner',element:'ether'}, peer:{discipline:'arcane',displayName:'Peer',element:'fire'} }
const loaded = { choice:{id:'empty',name:'Empty',source:'default'}, geometrySha256:'b'.repeat(64),
  sourceSha256:'c'.repeat(64), runId:'rescue-session', seed:'charms', scene:{
    bounds:{x:0,y:0,w:500,h:500}, environmentMode:2, fences:[], name:'Rescue', objects:[], roads:[],
    solomonDig:null, spawn:{facingDeg:180,x:250,y:250}, sprites:[], terrain:[],
  } }
const snapshot = state => ({ tick:state.tick,
  players:state.playerEntities.identities.map(({playerId},index) => {
    const p=state.playerEntities.progressions[index]
    return {playerId,health:p.currentHealth,life:p.lifeState,protection:p.rescueProtection,
      hagatha:p.hagathaRuntime,blindness:state.playerEntities.lightings[index].blindnessTicksRemaining}
  }), events:state.world.kind==='boneyard'?state.world.enemyEvents:[], flashes:state.screenFlashes,
  painterOrder:state.worldManagerOrder, rng:state.secondaryAbilities.rng,
  actors:state.secondaryAbilities.actors.map(actor=>({id:actor.id,kind:actor.kind,ageTicks:actor.ageTicks,
    ownerId:actor.ownerId,worldKey:actor.worldKey,position:actor.position,alpha:actor.alpha,
    radius:actor.radius,quantity:actor.quantity,phase:actor.phase,slowFactor:actor.slowFactor,damage:actor.damage,
    lightRegistration:actor.lightRegistration,painterRegistrations:actor.painterRegistrations})),
})
const specs = [
  {name:'until-hurt-once',perks:[24,25],amounts:[0,1,1],blindness:0,hub:false},
  {name:'charm-loss-before-rescue',perks:[7,24,25],amounts:[100],blindness:0,hub:false},
  {name:'blindness-preserves-charge',perks:[7,24,25],amounts:[100],blindness:5,hub:false},
  {name:'hub-rescue-without-scene-birth',perks:[7,24,25],amounts:[100],blindness:0,hub:true},
]
const cases = specs.map(spec=>{
  let state=game.createGameSimulation(configs,{gameRngSeed:17,combatRngSeed:91})
  if(!spec.hub) state=game.enterBoneyardWorld(state,loaded)
  const economy=entities.playerEconomyAt(state.playerEntities,'owner')
  state={...state,playerEntities:entities.replacePlayerEconomy(state.playerEntities,'owner',{
    ...economy,firstMixedSelectors:[...spec.perks],ownedPerkSelectors:[...spec.perks],revision:economy.revision+1,
  })}
  state={...state,playerEntities:entities.applyPlayerEntityHagathaPurchaseEffects(state.playerEntities,'owner',spec.perks,random.createNativeRng(1)).store}
  state={...state,playerEntities:{...state.playerEntities,lightings:state.playerEntities.lightings.map((row,index)=>
    index===0?{...row,blindnessTicksRemaining:spec.blindness}:row)}}
  const frames=[{request:null,expected:snapshot(state)}]
  for(const amount of spec.amounts){state=game.damageGameSimulationPlayer(state,'owner',amount,state.tick)
    frames.push({request:{kind:'damage',amount},expected:snapshot(state)})}
  for(let tick=1;tick<=5;tick++){state=game.stepGameSimulationTick(state,{})
    frames.push({request:{kind:'step'},expected:snapshot(state)})}
  return{spec,frames}
})
writeFixture('current-player-rescue-session-goldens.json','current-player-rescue-session-goldens.mjs',
  'core-server/game-simulation.ts damageGameSimulationPlayer and normal rescue/contact/resource phases',
  {sourceRevision,cases})
