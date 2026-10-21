#!/usr/bin/env node
// Complete public secondary epochs with actual EtherDrain construction and child clocks.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
import { allLearnedBook, authority, context, factors } from './secondary-inputs.mjs'
const secondary=await loadKernel('core-kernels/native-secondary-abilities.ts')
const belt=await loadKernel('core-kernels/native-belt.ts')
const painter=await loadKernel('core-kernels/native-world-manager-order.ts')
const sourceRevision=execFileSync('git',['rev-parse','HEAD'],{cwd:websiteDir,encoding:'utf8'}).trim()
const bits=value=>{
  if(typeof value==='number'){const buffer=Buffer.alloc(8);buffer.writeDoubleBE(value);return `f64:${buffer.toString('hex')}`}
  if(Array.isArray(value))return value.map(bits)
  if(value&&typeof value==='object')return Object.fromEntries(Object.entries(value).map(([key,value])=>[key,bits(value)]))
  return value
}
const actorFields=['ageTicks','alpha','damage','enhanced','endpoint','frame','freezeTicks','golem','hitTargetIds','id','kind',
  'lifetimeTicks','lightRegistration','midpoint','miscLightAppendOrdinal','ownerId','painterRegistrations','phase','position',
  'presentationRng','quantity','radius','rank','rotationRadians','scale','skillId','slowFactor','targetId','variant','velocity','worldKey','etherDrain']
const project=(state,order,result)=>({
  actors:state.actors.map(actor=>Object.fromEntries(actorFields.filter(key=>actor[key]!==undefined).map(key=>[key,actor[key]]))),
  events:state.events, nextActorId:state.nextActorId,nextEventId:state.nextEventId,rng:state.rng,
  contacts:result.etherDrainContacts,animationContacts:result.etherDrainWorldAnimationContacts,order:order.state(),
})
const cases=[]
for(const enhanced of [false,true]) for(const spatial of [false,true]){
  const player=authority(allLearnedBook(),factors(false))
  player.alive=true;player.enhancedEffects=enhanced
  player.belt=belt.bindNativeBeltSkill(player.belt,player.skillBook,74,0)
  let state=secondary.createNativeSecondarySimulation(91)
  const order=painter.createNativeWorldManagerOrder()
  const frames=[]
  const scenery=spatial?Array.from({length:8},(_,id)=>({id,position:{x:250+id*23,y:250},typeId:2001,secondaryVariant:id})):[]
  const targets=spatial?[
    {activationDelayTicks:0,forceFactor:1,nativeFlags:2,position:{x:258,y:250},ref:{kind:'enemy',id:9,registrationOrdinal:17}},
    {activationDelayTicks:0,forceFactor:1,nativeFlags:0x801,position:{x:261,y:250},ref:{kind:'player',id:'peer'}},
    {activationDelayTicks:0,forceFactor:.25,nativeFlags:0x800,position:{x:270,y:250},ref:{kind:'golem',id:5}},
    {activationDelayTicks:0,forceFactor:1,nativeFlags:0x400,position:{x:251,y:250},ref:{kind:'loot',id:3,registrationOrdinal:20}},
  ]:[]
  const animationTargets=spatial?['enemy-death-effect','primary-transient','secondary-actor','death-weapon','golem-fragment'].map((kind,index)=>({
    bounceVelocity:0,position:{x:252+index,y:250},ref:kind==='golem-fragment'?{kind,id:2,index:1}:{kind,id:2},
  })):[]
  for(let tick=1;tick<=1080;tick++){
    const currentTargets=tick<170?targets:targets.filter(target=>target.ref.kind!=='loot')
    const currentPlayer={...player,input:{...player.input,aim:{x:250,y:250},cast:{primary:false,quickbar:tick===1?0:null}}}
    const result=secondary.stepNativeSecondaryAbilities(state,{...context(currentPlayer,tick),
      registerWorldPainter:order.register,etherDrainTargets:()=>currentTargets,
      etherDrainWorldAnimations:()=>animationTargets,sceneryTargets:()=>scenery})
    state=result.state
    if(spatial&&tick===10)state=secondary.captureNativeEtherDrainImage(state,1,{atlas:'Demon',entry:80,tint:0xffffff,bodyYOffset:0},tick)
    if(spatial&&tick===16)state=secondary.captureNativeEtherDrainImage(state,1,null,tick)
    if(tick<=20||tick%10===0||tick>=1035)frames.push({tick,expected:bits(project(state,order,result))})
  }
  cases.push({enhanced,spatial,player,targets,animationTargets,scenery,frames})
}
writeFixture('current-ether-drain-field-goldens.json','tools/website-oracle/current-ether-drain-field-goldens.mjs',
  'core-kernels/native-secondary-abilities.ts complete EtherDrain epochs, explicit query callbacks and actual capture API',
  {sourceRevision,cases})
console.log(`recorded ${cases.length} actual constructor journeys / ${cases.length*1080} epochs`)
