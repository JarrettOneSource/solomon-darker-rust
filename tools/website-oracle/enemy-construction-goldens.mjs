#!/usr/bin/env node
// All native enemy-family births, constructor caps, grouped placements and direct children.
import { readFileSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import { resolve } from 'node:path'
import { loadKernel,fixturesDir,websiteDir,writeFixture } from './oracle.mjs'
const sourceRevision=execFileSync('git',['rev-parse','HEAD'],{cwd:websiteDir,encoding:'utf8'}).trim()
const stores=await loadKernel('core-server/boneyard-enemy-store.ts')
const workModule=await loadKernel('core-server/enemies/work.ts')
const construction=await loadKernel('core-server/enemies/construction.ts')
const schema=await loadKernel('core-kernels/boneyard-wave-schema.ts')
const managers=await loadKernel('core-kernels/native-world-manager-order.ts')
const nativeRng=await loadKernel('core-kernels/native-rng.ts')
const configs=JSON.parse(readFileSync(resolve(fixturesDir,'enemy-config-goldens.json'),'utf8'))
const recipe=token=>configs.cases.find(c=>c.token===token && c.authoredRecipe!==null).authoredRecipe
const rngRecord=rng=>[rng.indexA,rng.indexB,...rng.words]
function intent(token,index,extra={}) {
  return {enemyToken:token,flags:[],id:index+1,locationPolicy:'anywhere',
    nativeTypeId:schema.BONEYARD_WAVE_ENEMY_TYPES[token],position:{x:100+index*10,y:100},
    spawnTick:10,waveOrdinal:31,...(token==='PORTAL'?{authoredRecipe:recipe('PORTAL')}:{}),...extra}
}
const allTokens=Object.keys(schema.BONEYARD_WAVE_ENEMY_TYPES)
const profiles=[
  {name:'all-families',seed:'enemy-oracle',intents:allTokens.map((token,index)=>intent(token,index))},
  {name:'authored-families',seed:'骨:enemy',sharedLootSeed:731,painterBase:{actor:100,transient:70},intents:[
    ...['SKELETON','SKELETONARCHER','ZOMBIE','DEMONSKULL','DIREFACULTY','HEARTMONGER','PORTAL']
      .map((token,index)=>intent(token,index,{authoredRecipe:{...recipe(token),
        ...(token==='DEMONSKULL'?{classification:'boss'}:{})},
        enableDiscorporealHealthGates:token==='DEMONSKULL',flags:['FLAG_HPUP','FLAG_FAST']})),
    intent('DIREFACULTY',7,{authoredRecipe:recipe('DIREFACULTY')}),
    intent('DIREFACULTY',8,{authoredRecipe:recipe('DIREFACULTY')}),
    intent('SKELETONMAGE',9,{flags:['FLAG_SHIELD','FLAG_SHIELDFAST','FLAG_RANGEEASY']}),
  ]},
  {name:'no-eligible-target',seed:'targetless-construction',noTargets:true,
    intents:['SKELETON','SKELETONARCHER','ZOMBIE'].map((token,index)=>intent(token,index))},
  {name:'imp-cap-and-direct-child',seed:'imp-construction',intents:[
    ...Array.from({length:72},(_,index)=>intent('IMP',index)),
    intent('IMP',72,{portalEjection:{parentPosition:{x:200,y:300},parentHeadingDeg:30,
      childHeadingDeg:135,verticalVelocity:-12,inheritedPrimaryDamage:7}}),
  ]},
  {name:'grouped-placement-and-payloads',seed:'grouped-construction',intents:[
    intent('SKELETON',0,{placementGroupId:1,flags:['FLAG_PIKE','FLAG_ARMOR']}),
    intent('SKELETON',1,{placementGroupId:1}),
    intent('IMP',2,{greenImpSpitDamage:60,nativeTypeId:2044}),
    intent('COCOON',3,{cocoonTargetPlayerId:'hero'}),
    intent('SKELETONARCHER',4,{archerStrafing:true,flanking:false,pathfindingMode:3}),
    intent('IMP',5,{flags:['FLAG_SPLITMANY']}),
    intent('ZOMBIE',6,{zombieBodyType:1}),
  ]},
]
const player={alive:true,connected:true,eligible:true,collisionRadius:25,
  position:{x:500,y:500},velocityPerTick:{x:0,y:0}}
const cases=profiles.map(profile=>{
  const source=stores.createBoneyardEnemyStore(profile.seed,50)
  const order=profile.painterBase?managers.createNativeWorldManagerOrder({nextRegistrationOrdinal:profile.painterBase}):null
  let sharedRng=profile.sharedLootSeed===undefined?null:nativeRng.createNativeRng(profile.sharedLootSeed)
  const context={tick:10,players:{hero:{...player,eligible:!profile.noTargets},ghost:{...player,alive:false,position:{x:100,y:100}}},
    resolveMovement:request=>request.requestedPosition,...(order?{registerWorldPainter:order.register}:{}),
    ...(sharedRng?{rollLootSeed:bound=>{const draw=nativeRng.drawNativeInteger(sharedRng,bound);sharedRng=draw.state;return draw.value}}:{})}
  const work=workModule.createEnemyWork(source,context,true)
  const actors=construction.materializeSpawnIntents(work,context,profile.intents)
  return {...profile,actors,rng:work.rngState,steering:rngRecord(work.steeringRngState),
    locomotion:rngRecord(work.locomotionRngState),headFacing:rngRecord(work.headFacingRngState),
    nextActorId:work.nextActorId,nextDeathEffectId:work.nextDeathEffectId,
    nextNativeCellBindingOrder:work.nextNativeCellBindingOrder,nextNativeRegistrationOrder:work.nextNativeRegistrationOrder,
    nextEventId:work.nextEventId,featuredBossId:work.featuredBossId,
    facultyVoiceController:work.facultyVoiceController,demonSkullEncounter:work.demonSkullEncounter,
    spawned:work.spawnedActorIds,events:work.events,painterOrder:order?.state().nextRegistrationOrdinal??null,
    sharedRng:sharedRng?rngRecord(sharedRng):null}
})
writeFixture('enemy-construction-goldens.json','tools/website-oracle/enemy-construction-goldens.mjs',
  'Current Website/frontend/src/game/core-server: boneyard-enemy-store.ts, enemies/construction.ts, enemies/work.ts',{sourceRevision,cases})
console.log(`enemy construction: ${cases.length} cohorts, ${cases.reduce((n,c)=>n+c.intents.length,0)} requests / ${cases.reduce((n,c)=>n+c.actors.length,0)} accepted actors`)
