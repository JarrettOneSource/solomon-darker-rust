#!/usr/bin/env node
// Native Spider calls, complete Silk splines/fade, illuminated force handoff and web status.
import { loadKernel,writeFixture } from './oracle.mjs'
const spider=await loadKernel('core-kernels/native-spider.ts')
const silk=await loadKernel('core-kernels/native-silk.ts')
const force=await loadKernel('core-server/enemies/silk-force.ts')
const webbed=await loadKernel('core-kernels/native-webbed.ts')
const nativeRng=await loadKernel('core-kernels/native-rng.ts')
const stores=await loadKernel('core-server/boneyard-enemy-store.ts')
const deadSpider=await loadKernel('core-kernels/native-dead-spider.ts')
const rngRecord=rng=>[rng.indexA,rng.indexB,...rng.words]
const normalized=degrees=>((degrees%360)+360)%360
const silkCases=[
  {seed:1,position:{x:100,y:200},target:{position:{x:500,y:400},velocityPerTick:{x:0.75,y:-0.25}}},
  {seed:731,position:{x:12.25,y:-45.5},target:{position:{x:12.25,y:300},velocityPerTick:{x:0,y:0}}},
  {seed:777,position:{x:100,y:200},target:{position:{x:100,y:200},velocityPerTick:{x:0,y:0}}},
].map(input=>{
  const creation=silk.createNativeSilk(input.position,input.target,45,nativeRng.createNativeRng(input.seed))
  let state=creation.state
  const samples=[{step:0,state}]
  for (let step=1;step<=1800;step+=1) {
    const previous=state
    state=silk.stepNativeSilk(state)
    if (step<=10 || step%50===0 || state===null || state.ended!==previous.ended) samples.push({step,state})
    if (state===null) break
  }
  return {...input,creation,samples,rng:rngRecord(creation.rngState)}
})
const movementProfiles=[
  {name:'wander',target:null,light:1},
  {name:'approach',target:{x:500,y:500},light:1},
  {name:'dark-spit',target:{x:500,y:500},light:0},
  {name:'close-and-bite',target:{x:175,y:250},light:1},
  {name:'webbed-attachment',target:{x:175,y:250},light:1,severity:3},
  {name:'summoned-target',target:{x:175,y:250},light:1,summoned:true},
  {name:'route-following',target:{x:500,y:500},light:0.03,route:true},
]
const movements=movementProfiles.map((profile,index)=>{
  const seed=123+index
  let rng=nativeRng.createNativeRng(seed)
  const initial={...spider.createNativeSpiderState(0.5),family:'spider',phase:'active'}
  let state=initial,position={x:150,y:250},heading=0,shared=0
  const target=profile.target?{position:profile.target,headingDeg:75,radius:25,isPlayer:!profile.summoned,
    webSeverity:profile.severity??0,velocityPerTick:{x:0.5,y:-0.25}}:null
  const samples=[]
  for (let step=0;step<300;step+=1) {
    shared=Math.max(0,shared-1)
    const request={headingDeg:heading,position,speed:3.25,statusFactor:0.75,light:profile.light,target,
      wanderHeadingDeg:210,spitWebs:true,cocoonHealth:45,liveSilkCount:0,sharedSpitTicksRemaining:shared,
      ...(profile.route?{routeGoal:goal=>({goal,followingRoute:true,turnAround:step%50===0})}:{})}
    const result=spider.moveNativeSpider(state,request,rng)
    rng=result.rngState;state=result.state;position=result.goal;heading=normalized(result.headingDeg);shared=result.sharedSpitTicksRemaining
    if (step<5 || step%25===0 || result.bite || result.suck || result.silk) {
      const created=result.silk
      samples.push({step,state,headingDeg:result.headingDeg,position:result.position,goal:result.goal,bite:result.bite,suck:result.suck,
        silk:created?{state:created.state,soundIndex:created.soundIndex,soundPitch:created.soundPitch,soundPosition:created.soundPosition}:null,
        biteSounds:result.biteSounds,sharedSpitTicksRemaining:shared,rng:rngRecord(rng)})
    }
  }
  return {...profile,seed,initial,targetState:target,samples,finalState:state,finalPosition:position,finalHeadingDeg:heading,rng:rngRecord(rng)}
})
const forces=['bright','dark','half'].map((lightMode,index)=>{
  let state=silkCases[index].creation.state
  for (let step=0;step<100;step+=1) state=silk.stepNativeSilk(state)
  const initial=state
  let store={...stores.createBoneyardEnemyStore(`silk-force:${lightMode}`,50),silks:[{
    id:44,ownerActorId:1,spawnTick:0,state,nativeRegistrationOrder:50,nativeCellBindingOrder:50,
    painterRegistration:{managerLane:'actor',registrationOrdinal:17},
  }]}
  let rng=nativeRng.createNativeRng(777)
  const queries=[]
  const lightAt=position=>{
    const light=lightMode==='bright'?0.8:lightMode==='dark'?0:position.x>=100?0.5:0
    queries.push({position,light});return light
  }
  const samples=[]
  for (const [tick,amount] of [[100,0.4],[101,0.6],[102,0.001],[103,2]]) {
    const result=force.applyBoneyardSilkForce(store,44,amount,{x:2,y:-1},tick,rng,lightAt)
    store=result.store;rng=result.rng
    samples.push({tick,silks:store.silks,fragments:store.silkFragments,nextId:store.nextDeathEffectId,rng:rngRecord(rng)})
  }
  for (let tick=104;tick<160;tick+=1) {
    const work={silkFragments:[...store.silkFragments]}
    force.stepSilkFragments(work);store={...store,silkFragments:work.silkFragments}
    if (tick%10===0 || store.silkFragments.length===0) samples.push({tick,silks:store.silks,fragments:store.silkFragments,nextId:store.nextDeathEffectId,rng:rngRecord(rng)})
    if (store.silkFragments.length===0) break
  }
  return {lightMode,initial,queries,samples}
})
let web=null
const webSamples=[]
for (const strength of [10,50]) {web=webbed.applyNativeWebbed(web,strength);webSamples.push({kind:'apply',strength,state:web,scale:webbed.nativeWebbedMovementScale(web)})}
for (let tick=0;tick<20;tick+=1) {web=webbed.stepNativeWebbed(web,{x:1,y:0});webSamples.push({kind:'step',state:web,scale:webbed.nativeWebbedMovementScale(web)})}
for (const strength of [10,20,100]) {web=webbed.applyNativeWebbed(web,strength);webSamples.push({kind:'apply',strength,state:web,scale:webbed.nativeWebbedMovementScale(web)})}
const cocoon=Array.from({length:24},(_,index)=>({heading:index*15,position:webbed.nativeCocoonPosition({x:100,y:200},index*15),tilt:webbed.nativeCocoonTilt(index*15)}))
const remains=[0,177,359].map(heading=>{
  let state=deadSpider.createNativeDeadSpider({x:150,y:250},heading),rng=nativeRng.createNativeRng(731)
  const samples=[{step:0,state,entry:deadSpider.nativeDeadSpiderEntry(state),rng:rngRecord(rng)}]
  for (let step=1;step<=2100;step+=1) {
    const next=deadSpider.stepNativeDeadSpider(state,rng);state=next.state;rng=next.rngState
    if (step<=5 || step%50===0 || state===null) samples.push({step,state,entry:state?deadSpider.nativeDeadSpiderEntry(state):null,rng:rngRecord(rng)})
    if (state===null) break
  }
  return {heading,samples}
})
writeFixture('enemy-spider-goldens.json','tools/website-oracle/enemy-spider-goldens.mjs',
  'Website/frontend/src/game: core-kernels/native-spider.ts, native-silk.ts, native-webbed.ts, native-silk-force.ts, native-natural-spline.ts, core-server/enemies/silk-force.ts',
  {silkCases,movements,forces,webSamples,cocoon,remains})
console.log(`Spider kernels: ${movements.length*300} movement calls, ${silkCases.length} complete Silk lifecycles, ${forces.length} illuminated force handoffs`)
