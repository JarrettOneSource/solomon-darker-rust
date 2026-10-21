#!/usr/bin/env node
// Complete projectile trajectories, query order, contacts, hazard handoff and retirement.
import { execFileSync } from 'node:child_process'
import { loadKernel,websiteDir,writeFixture } from './oracle.mjs'
const sourceRevision=execFileSync('git',['rev-parse','HEAD'],{cwd:websiteDir,encoding:'utf8'}).trim()
const stores=await loadKernel('core-server/boneyard-enemy-store.ts')
const workModule=await loadKernel('core-server/enemies/work.ts')
const construction=await loadKernel('core-server/enemies/construction.ts')
const emission=await loadKernel('core-server/enemies/projectile-emission.ts')
const motion=await loadKernel('core-server/enemies/projectiles.ts')
const arrowEffects=await loadKernel('core-server/enemies/projectile-effects.ts')
const registration=await loadKernel('core-server/enemies/registration.ts')
const nativeRng=await loadKernel('core-kernels/native-rng.ts')
const managers=await loadKernel('core-kernels/native-world-manager-order.ts')
const profiles=[
  {name:'arrow-fall-and-fade',kind:'arrow',ticks:400},
  {name:'arrow-contact',kind:'arrow',target:{x:150,y:210},ticks:30},
  {name:'arrow-fire-double-impact',kind:'arrow',target:{x:150,y:210},lineBlocked:true,payload:'fire',ticks:30},
  {name:'arrow-bounds',kind:'arrow',boundsBlocked:true,ticks:20},
  {name:'arrow-chill-tumble',kind:'arrow',tumble:true,ticks:100},
  {name:'firebolt-trail-and-view',kind:'firebolt',viewBlocked:true,ticks:150},
  {name:'firebolt-expiry',kind:'firebolt',ticks:420},
  {name:'firebolt-contact',kind:'firebolt',target:{x:150,y:180},ticks:60},
  {name:'firebolt-wall',kind:'firebolt',lineBlocked:true,ticks:50},
  {name:'guided-cold-contact',kind:'guided-missile',target:{x:150,y:170},payload:'cold',ticks:150},
  {name:'guided-poison-contact',kind:'guided-missile',target:{x:150,y:170},payload:'poison',ticks:150},
  {name:'guided-cell-order',kind:'guided-missile',alternateTargets:true,ticks:60},
  {name:'guided-expiry',kind:'guided-missile',ticks:1320},
  {name:'guided-bounds',kind:'guided-missile',boundsBlocked:true,ticks:30},
  {name:'bomb-natural-settle',kind:'demon-bomb',ticks:1200},
  {name:'bomb-contact-and-fire',kind:'demon-bomb',target:{x:150,y:230},ticks:600},
  {name:'bomb-wall-and-fire',kind:'demon-bomb',pointBlocked:true,ticks:600},
  {name:'poison-pool-contacts-and-fade',kind:'poison-pool',target:{x:170,y:250},ticks:3220},
]
const rngRecord=rng=>[rng.indexA,rng.indexB,...rng.words]
const cases=profiles.map(profile=>{
  const seed=`projectile:${profile.name}`
  const queries=[],explosions=[]
  const order=managers.createNativeWorldManagerOrder({nextRegistrationOrdinal:{actor:100,transient:70}})
  const player=position=>({alive:true,connected:true,eligible:true,collisionRadius:25,position,velocityPerTick:{x:0,y:0}})
  const players=profile.target?{hero:player(profile.target)}:{}
  if (profile.alternateTargets) {
    players.first=player({x:170,y:245})
    players.closer=player({x:152,y:243})
  }
  if (profile.kind==='poison-pool') players.summon={...player({x:150,y:250}),summoned:true}
  let tick=10
  const context={tick,players,registerWorldPainter:order.register,resolveMovement:request=>request.requestedPosition,
    projectileWorldBlocked:query=>{
      const blocked=query.kind==='line'?!!profile.lineBlocked:query.kind==='point'?!!profile.pointBlocked:
        query.kind==='bounds'?!!profile.boundsBlocked:!!profile.viewBlocked && query.position.y< -50
      queries.push({tick,request:query,blocked})
      return blocked
    },onProjectileExplosion:position=>explosions.push({tick,position})}
  const token=profile.kind==='poison-pool'?'ZOMBIE':'SKELETONARCHER'
  const intent={enemyToken:token,flags:token==='ZOMBIE'?['FLAG_ROTTEN']:[],id:1,locationPolicy:'anywhere',nativeTypeId:token==='ZOMBIE'?1006:1002,
    position:{x:150,y:250},spawnTick:10,waveOrdinal:31}
  const initial=workModule.createEnemyWork(stores.createBoneyardEnemyStore(seed,50),context,true)
  const actor=construction.materializeSpawnIntents(initial,context,[intent])[0]
  initial.actors.push(actor)
  const options={headingDeg:0,payload:profile.payload??(profile.kind==='poison-pool'?'poison':'none'),secondaryDamage:7,
    coldSlowTicks:profile.payload==='cold'?250:0,poisonDamage:profile.payload==='poison'?2:0,poisonDuration:profile.payload==='poison'?10:0,
    ...(profile.kind==='arrow'?{speed:6,minimumSpeed:6,lifetimeTicks:5}:{})}
  const projectile=emission.spawnProjectile(initial,actor,10,profile.kind,8,options)
  if (profile.alternateTargets) initial.projectiles=[{...projectile,targetPlayerId:null}]
  let store=workModule.finishEnemyStore(initial,10)
  let combatRng=nativeRng.createNativeRng(801)
  const samples=[{tick,store,events:initial.events,damage:[],knockbacks:[],combat:rngRecord(combatRng),painterOrder:order.state().nextRegistrationOrdinal}]
  for (tick=11;tick<=10+profile.ticks;tick+=1) {
    context.tick=tick
    let pointEvents=[]
    if (profile.tumble && tick===18) store=arrowEffects.setBoneyardArrowChillTumbleAccumulator(store,1,0.75)
    if (profile.tumble && (tick===20 || tick===21)) {
      const result=arrowEffects.tumbleBoneyardArrow(store,1,{x:2,y:-1},tick,combatRng,order.register)
      store=result.store;combatRng=result.rng;pointEvents=result.events
    }
    const work=workModule.createEnemyWork(store,context,true)
    registration.bindEnemyTargets(work,players)
    motion.stepProjectiles(work,context)
    store=workModule.finishEnemyStore(work,tick)
    const events=[...pointEvents,...work.events]
    if (tick<=25 || tick%50===0 || events.length>0 || (profile.tumble && tick===18)) {
      samples.push({tick,store,events,damage:work.playerDamage,knockbacks:work.playerKnockbacks.map(({actorId,delta,eventId,playerId})=>({actorId,delta,eventId,playerId})),
        combat:rngRecord(combatRng),painterOrder:order.state().nextRegistrationOrdinal})
    }
  }
  return {...profile,seed,intent,options,players,playerOrder:Object.keys(players),queries,explosions,samples:samples.map(sample=>({
    tick:sample.tick,events:sample.events,damage:sample.damage,knockbacks:sample.knockbacks,
    projectiles:sample.store.projectiles,effects:sample.store.projectileEffects,retainedKnockbacks:sample.store.projectileKnockbacks,
    bindings:sample.store.targetCellBindings,rng:sample.store.rngState,steering:rngRecord(sample.store.steeringRngState),combat:sample.combat,
    nextProjectileId:sample.store.nextProjectileId,nextProjectileEffectId:sample.store.nextProjectileEffectId,nextEventId:sample.store.nextEventId,
    nextNativeCellBindingOrder:sample.store.nextNativeCellBindingOrder,nextNativeRegistrationOrder:sample.store.nextNativeRegistrationOrder,
    painterOrder:sample.painterOrder,
  }))}
})
writeFixture('enemy-projectiles-goldens.json','tools/website-oracle/enemy-projectiles-goldens.mjs',
  'Website/frontend/src/game/core-server: enemies/projectiles.ts, enemies/projectile-emission.ts, enemies/projectile-effects.ts, enemies/projectile-hazards.ts, boneyard-transient-effects.ts',{sourceRevision,cases})
console.log(`enemy projectiles: ${cases.length} trajectories / ${cases.reduce((n,c)=>n+c.ticks,0)} ticks / ${cases.reduce((n,c)=>n+c.queries.length,0)} world queries`)
