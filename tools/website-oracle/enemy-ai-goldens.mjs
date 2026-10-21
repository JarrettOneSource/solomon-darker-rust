#!/usr/bin/env node
// Actor-level AI traces retain the surrounding hit, head, light, births and VFX writers.
import { readFileSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import assert from 'node:assert/strict'
import { resolve } from 'node:path'
import { fixturesDir,loadKernel,readFixture,websiteDir,writeFixture } from './oracle.mjs'
const sourceRevision=execFileSync('git',['rev-parse','HEAD'],{cwd:websiteDir,encoding:'utf8'}).trim()
const stores = await loadKernel('core-server/boneyard-enemy-store.ts')
const workModule = await loadKernel('core-server/enemies/work.ts')
const construction = await loadKernel('core-server/enemies/construction.ts')
const update = await loadKernel('core-server/enemies/actor-update.ts')
const zombie = await loadKernel('core-server/enemies/zombie.ts')
const projectiles = await loadKernel('core-server/enemies/projectiles.ts')
const registration = await loadKernel('core-server/enemies/registration.ts')
const maggots = await loadKernel('core-server/enemies/maggots.ts')
const damage = await loadKernel('core-server/enemies/damage.ts')
const dampen = await loadKernel('core-server/enemies/dampen.ts')
const spiderRemains = await loadKernel('core-server/enemies/spider-remains.ts')
const webbed = await loadKernel('core-kernels/native-webbed.ts')
const cocoons = await loadKernel('core-server/enemies/cocoon.ts')
const deaths = await loadKernel('core-server/enemies/deaths.ts')
const wraith = await loadKernel('core-server/enemies/wraith.ts')
const heartmonger = await loadKernel('core-server/enemies/heartmonger.ts')
const facultyActions = await loadKernel('core-kernels/native-faculty-actions.ts')
const bossSpells = await loadKernel('core-server/enemies/boss-spells.ts')
const demonSkullActions = await loadKernel('core-server/enemies/demon-skull-actions.ts')
const puppetHits = await loadKernel('core-server/enemies/puppet-hits.ts')
const schema = await loadKernel('core-kernels/boneyard-wave-schema.ts')
const effects = await loadKernel('core-server/boneyard-transient-effects.ts')
const random = await loadKernel('core-server/enemies/random.ts')
const managers = await loadKernel('core-kernels/native-world-manager-order.ts')
const nativeRng = await loadKernel('core-kernels/native-rng.ts')
const profiles = [
  {name:'claw-approach',token:'SKELETON',flags:[],motion:'approach'},
  {name:'claw-contact',token:'SKELETON',flags:[],motion:'near'},
  {name:'armored-claw',token:'SKELETON',flags:['FLAG_ARMOR'],motion:'near'},
  {name:'sword-multi-marker',token:'SKELETON',flags:['FLAG_SWORD','FLAG_FAST'],motion:'near'},
  {name:'pike-restraint',token:'SKELETON',flags:['FLAG_PIKE'],motion:'near'},
  {name:'pike-summon',token:'SKELETON',flags:['FLAG_PIKE'],motion:'near',summoned:true},
  {name:'target-leaves',token:'SKELETON',flags:['FLAG_AXE'],motion:'leaves'},
  {name:'blocked-recovery',token:'SKELETON',flags:[],motion:'approach',blocked:true},
  {name:'zombie-approach',token:'ZOMBIE',flags:[],motion:'approach'},
  {name:'zombie-beat',token:'ZOMBIE',flags:[],motion:'near'},
  {name:'zombie-target-leaves',token:'ZOMBIE',flags:[],motion:'leaves'},
  {name:'rotten-zombie',token:'ZOMBIE',flags:['FLAG_ROTTEN'],motion:'near'},
  {name:'zombie-body-type',token:'ZOMBIE',flags:[],motion:'near',zombieBodyType:1},
  {name:'imp-approach',token:'IMP',flags:[],motion:'approach'},
  {name:'imp-contact',token:'IMP',flags:[],motion:'near'},
  {name:'imp-blocked',token:'IMP',flags:[],motion:'near',blocked:true},
  {name:'wraith-approach',token:'WRAITH',flags:[],motion:'approach'},
  {name:'wraith-contact',token:'WRAITH',flags:[],motion:'near'},
  {name:'wraith-offscreen',token:'WRAITH',flags:[],motion:'near',offscreen:true},
  {name:'archer-normal',token:'SKELETONARCHER',flags:[],motion:'near'},
  {name:'archer-poison-leading',token:'SKELETONARCHER',flags:['FLAG_POISONARROW','FLAG_LEADING'],motion:'near'},
  {name:'archer-fire-easy',token:'SKELETONARCHER',flags:['FLAG_FIREARROW','FLAG_RANGEEASY'],motion:'near'},
  {name:'archer-authored-strafe',token:'SKELETONARCHER',flags:[],motion:'near',authored:true},
  {name:'archer-no-line-of-sight',token:'SKELETONARCHER',flags:[],motion:'approach',blocked:true},
  {name:'mage-fire',token:'SKELETONMAGE',flags:[],motion:'near'},
  {name:'mage-frost',token:'SKELETONMAGE',flags:['FLAG_CASTFROST'],motion:'near'},
  {name:'mage-poison',token:'SKELETONMAGE',flags:['FLAG_CASTPOISON','FLAG_RANGEEASY'],motion:'near'},
  {name:'mage-lightning',token:'SKELETONMAGE',flags:['FLAG_CASTLIGHTNING'],motion:'near'},
  {name:'mage-lightning-wall',token:'SKELETONMAGE',flags:['FLAG_CASTLIGHTNING'],motion:'near',spellBlocked:true},
  {name:'mage-self-shield',token:'SKELETONMAGE',flags:['FLAG_SHIELD','FLAG_SHIELDFAST'],motion:'near',ticks:1100},
  {name:'mage-ally-shield',token:'SKELETONMAGE',flags:['FLAG_SHIELDOTHERS','FLAG_SHIELDFAST'],motion:'near',ticks:1100,allies:true},
  {name:'burning-skeleton',token:'SKELETON',flags:['FLAG_BURNING'],motion:'near'},
  {name:'burning-mage',token:'SKELETONMAGE',flags:['FLAG_BURNING','FLAG_CASTFROST'],motion:'near'},
  {name:'demon-approach',token:'DEMON',flags:[],motion:'approach',ticks:1000},
  {name:'demon-bombs',token:'DEMON',flags:[],motion:'near',ticks:1000},
  {name:'demon-target-leaves',token:'DEMON',flags:[],motion:'leaves',ticks:1000},
  {name:'coffin-population',token:'COFFIN',flags:[],motion:'approach',ticks:1800},
  {name:'coffin-bites',token:'COFFIN',flags:[],motion:'near',ticks:1400},
  {name:'coffin-strong-many',token:'COFFIN',flags:['FLAG_MANYMAGGOTS','FLAG_STRONGMAGGOTS'],motion:'near',ticks:1400},
  {name:'coffin-owner-removed',token:'COFFIN',flags:[],motion:'near',ticks:200,opening:true,removeOwnerAt:80},
  {name:'coffin-maggot-hit',token:'COFFIN',flags:[],motion:'approach',ticks:300,opening:true,maggotHitAt:80},
  {name:'spider-lit-approach',token:'SPIDER',flags:[],motion:'approach',ticks:800,light:1},
  {name:'spider-close-bites',token:'SPIDER',flags:['FLAG_NOSPIT'],motion:'near',ticks:800,light:1},
  {name:'spider-webbed-attachment',token:'SPIDER',flags:['FLAG_NOSPIT'],motion:'near',ticks:400,light:1,webbed:true},
  {name:'spider-dark-silks',token:'SPIDER',flags:['FLAG_COCOON'],motion:'approach',ticks:800,light:0},
  {name:'spider-degraded-dark',token:'SPIDER',flags:[],motion:'approach',ticks:400,light:0,view:'visible'},
  {name:'spider-degraded-offscreen',token:'SPIDER',flags:[],motion:'approach',ticks:400,light:1,view:'offscreen'},
  {name:'spider-basic-effects',token:'SPIDER',flags:[],motion:'near',ticks:400,light:1,view:'basic'},
  {name:'spider-no-target-boundary',token:'SPIDER',flags:[],motion:'approach',ticks:400,light:1,view:'visible',noTargets:true},
  {name:'spider-summoned-target',token:'SPIDER',flags:[],motion:'near',ticks:400,light:1,summoned:true},
  {name:'spider-shared-silk-limit',token:'SPIDER',flags:[],motion:'near',ticks:800,light:0,manySpiders:true},
  {name:'spider-cocoon-contact',token:'SPIDER',flags:['FLAG_COCOON'],motion:'near',ticks:1600,light:0,applyWebContacts:true,stationary:true},
  {name:'cocoon-owner-follows',token:'COCOON',flags:[],motion:'follow',ticks:400,webbed:true,contactBirth:true},
  {name:'cocoon-owner-dies',token:'COCOON',flags:[],motion:'follow',ticks:200,webbed:true,contactBirth:true,ownerDiesAt:100},
  {name:'cocoon-owner-removal',token:'COCOON',flags:[],motion:'follow',ticks:200,webbed:true,contactBirth:true,removeCocoonAt:80},
  {name:'heartmonger-unlit',token:'HEARTMONGER',flags:[],motion:'near',ticks:600,light:0,summonMode:4},
  {name:'heartmonger-offscreen',token:'HEARTMONGER',flags:[],motion:'near',ticks:600,offscreen:true,summonMode:4},
  {name:'heartmonger-no-target',token:'HEARTMONGER',flags:[],motion:'near',ticks:600,noTargets:true,summonMode:4},
  {name:'heartmonger-detached-flock',token:'HEARTMONGER',flags:[],motion:'near',ticks:900,detachAt:250,summonMode:0},
  {name:'heartmonger-detached-unplaced',token:'HEARTMONGER',flags:[],motion:'near',ticks:700,detachAt:11,summonMode:0},
]
for (let summonMode=0;summonMode<5;summonMode+=1) {
  profiles.push({name:`heartmonger-summons-${summonMode}`,token:'HEARTMONGER',flags:[],motion:'near',summonMode,ticks:1200})
}
for (const primary of [0,1,2]) {
  profiles.push({name:`faculty-primary-${primary}`,token:'DIREFACULTY',flags:[],motion:'middle',
    primary,secondary:0,facultyAction:primary===1?'lightning':'throw',ticks:1700,stationary:true,removeActorAt:400})
}
for (const secondary of [0,1,2]) {
  profiles.push({name:`faculty-secondary-${secondary}`,token:'DIREFACULTY',flags:[],motion:'middle',
    primary:2,secondary,facultyAction:'two-hand',ticks:1700,stationary:true,removeActorAt:400})
}
profiles.push({name:'faculty-lightning-wall',token:'DIREFACULTY',flags:[],motion:'middle',
  primary:1,secondary:0,facultyAction:'lightning',ticks:600,stationary:true,removeActorAt:400,spellBlocked:true})
profiles.push({name:'faculty-live-controller',token:'DIREFACULTY',flags:[],motion:'middle',
  primary:0,secondary:1,ticks:6000,stationary:true})
for (const kind of ['bite','scream','flair','eyes','mouth','spit']) {
  profiles.push({name:`demon-skull-${kind}`,token:'DEMONSKULL',flags:[],motion:kind==='bite'?'near':'middle',
    skullAction:kind,capabilities:0,ticks:kind==='mouth'?1400:900,stationary:true,removeActorAt:kind==='mouth'?1150:700})
}
profiles.push({name:'demon-skull-green-imp-spit',token:'DEMONSKULL',flags:[],motion:'middle',
  skullAction:'spit',capabilities:8,ticks:1000,removeActorAt:700})
profiles.push({name:'demon-skull-live-controller',token:'DEMONSKULL',flags:[],motion:'approach',capabilities:7,ticks:4000})
profiles.push({name:'demon-skull-mouth-puppets',token:'DEMONSKULL',flags:[],motion:'middle',skullAction:'mouth',capabilities:0,
  ticks:1100,stationary:true,puppets:true})
for (const token of ['SKELETON','SKELETONARCHER','SKELETONMAGE','IMP','PORTAL','ZOMBIE','WRAITH','DEMON','COFFIN','SPIDER','HEARTMONGER','DIREFACULTY','DEMONSKULL']) {
  profiles.push({name:`terminal-${token.toLowerCase()}`,token,flags:token==='DEMON'?['FLAG_DEATHIMPS']:token==='IMP'?['FLAG_SPLIT']:token==='ZOMBIE'?['FLAG_ROTTEN']:[],
    motion:'near',frequency:0,summonMode:0,primary:0,secondary:0,capabilities:0,killAt:30,
    ticks:token==='DEMONSKULL'?900:token==='DIREFACULTY'?600:token==='HEARTMONGER'?900:400,terminal:true})
}
profiles.push({name:'terminal-mega-discorporeal',token:'DEMONSKULL',flags:[],motion:'near',capabilities:0,megaDeath:true,killAt:30,ticks:2800,terminal:true})
profiles.push({name:'terminal-captured-spider',token:'SPIDER',flags:[],motion:'near',killAt:30,etherCapture:true,ticks:300,terminal:true})
for (const flags of [['FLAG_PIKE','FLAG_HOODED'],['FLAG_ARMOR','FLAG_SWORD','FLAG_HELM'],['FLAG_FLAIL','FLAG_HORNED'],['FLAG_MACE'],['FLAG_AXE']]) {
  profiles.push({name:`terminal-skeleton-${flags.join('-')}`,token:'SKELETON',flags,motion:'near',killAt:30,ticks:300,terminal:true})
}
for (const initialImps of [68,69]) profiles.push({name:`terminal-imp-cap-${initialImps}`,token:'IMP',flags:['FLAG_SPLIT'],motion:'near',killAt:30,ticks:100,initialImps,terminal:true})
for (let frequency=0;frequency<6;frequency+=1) {
  profiles.push({name:`portal-frequency-${frequency}`,token:'PORTAL',flags:[],motion:'near',frequency,ticks:1400})
}
for (const token of ['SKELETON','SKELETONARCHER','SKELETONMAGE','ZOMBIE','DEMON','SPIDER','HEARTMONGER','DIREFACULTY','DEMONSKULL','PORTAL']) {
  profiles.push({name:`world-status-${token.toLowerCase()}`,token,flags:[],motion:'near',ticks:700,
    frequency:0,summonMode:0,primary:0,secondary:0,capabilities:0,statuses:true})
}
profiles.push({name:'world-mage-dampen',token:'SKELETONMAGE',flags:['FLAG_CASTLIGHTNING'],motion:'near',dampenAt:40,ticks:800})
profiles.push({name:'world-faculty-dampen',token:'DIREFACULTY',flags:[],motion:'near',primary:1,secondary:1,dampenAt:40,ticks:800})
profiles.push({name:'world-pause-late-births',token:'SKELETONARCHER',flags:['FLAG_FIREARROW'],motion:'near',pause:true,lateBirths:true,ticks:600})
profiles.push({name:'world-shared-loot-mage',token:'SKELETONMAGE',flags:['FLAG_CASTPOISON'],motion:'near',sharedLootSeed:29,ticks:900})
profiles.push({name:'world-shared-loot-discorporeal',token:'DEMONSKULL',flags:[],motion:'near',capabilities:15,sharedLootSeed:39,ticks:1500})
// Game's two registration producers are joined only after primary/contact phases
// (game-simulation.ts:2085,3225). A real Dampen call between them distinguishes
// shared allocation from prematurely assigning projectile ordinals.
profiles.push({name:'world-deferred-fire-dampen',token:'SKELETONMAGE',flags:[],motion:'middle',deferred:true,dampenAfterTrail:true,ticks:1000})
profiles.push({name:'world-deferred-spider',token:'SPIDER',flags:['FLAG_COCOON'],motion:'approach',deferred:true,light:0,ticks:900})
profiles.push({name:'world-deferred-demon',token:'DEMON',flags:[],motion:'near',deferred:true,ticks:1400})
// The retained 128 programs passed the historical key, which current Source
// deliberately ignores. Keep those inputs and states; add distinct current-key
// programs rather than silently relabeling their degraded-movement coverage.
const currentMovementProfiles = profiles.filter(profile => profile.view).map(profile => ({
  ...profile, name: `current-${profile.name}`, movementViewInput: 'nativeMovementView',
}))
const movementViewInputs = Object.fromEntries([
  ...profiles.filter(profile => profile.view).map(profile => [profile.name, 'spiderMovementView']),
  ...currentMovementProfiles.map(profile => [profile.name, profile.movementViewInput]),
])
const appendMovementViews = process.env.SOLOMON_APPEND_ENEMY_MOVEMENT_VIEWS === '1'
const retained = appendMovementViews ? readFixture(resolve(
  process.env.SOLOMON_RETAINED_ENEMY_FIXTURES??fixturesDir, 'enemy-ai-goldens.json.zlib',
)) : null
if (retained) {
  assert.equal(retained.sourceRevision, sourceRevision)
  assert.deepEqual(retained.cases.map(row => row.name), profiles.map(row => row.name))
}
const recordingProfiles = appendMovementViews ? currentMovementProfiles : [...profiles, ...currentMovementProfiles]
const configs=JSON.parse(readFileSync(resolve(fixturesDir,'enemy-config-goldens.json'),'utf8'))
const portalRecipe=configs.cases.find(c=>c.token==='PORTAL' && c.authoredRecipe!==null).authoredRecipe
const heartmongerRecipe=configs.cases.find(c=>c.token==='HEARTMONGER' && c.authoredRecipe!==null).authoredRecipe
const facultyRecipe=configs.cases.find(c=>c.token==='DIREFACULTY' && c.authoredRecipe!==null).authoredRecipe
const skullRecipe=configs.cases.find(c=>c.token==='DEMONSKULL' && c.authoredRecipe!==null).authoredRecipe
const rngRecord = rng => [rng.indexA,rng.indexB,...rng.words]
const recordedCases = recordingProfiles.map(profile => {
  const seed = `ground:${profile.name}`
  const order = managers.createNativeWorldManagerOrder({nextRegistrationOrdinal:{actor:100,transient:70}})
  const player = {alive:true,connected:true,eligible:!profile.noTargets,collisionRadius:25,summoned:profile.summoned??false,
    position:{x:500,y:500},velocityPerTick:profile.stationary?{x:0,y:0}:{x:0.75,y:-0.25}}
  let sharedLootRng = profile.sharedLootSeed===undefined ? null : nativeRng.createNativeRng(profile.sharedLootSeed)
  const context = {tick:10,enhancedEffects:profile.enhancedEffects??true,resolveSpawnIntents:()=>[],players:{hero:player},registerWorldPainter:order.register,
    navigation:{isPathClear:()=>!profile.blocked,findRoute:()=>null},
    resolveMovement:request=>profile.blocked?request.position:request.requestedPosition,
    nativeVisibility:()=>({admitted:!profile.offscreen,intensity:profile.light??1}),
    projectedPointVisible:position=>position.x>=0&&position.x<=1000&&position.y>=0&&position.y<=1000,
    ...(['DIREFACULTY','DEMONSKULL'].includes(profile.token)?{nativeViewBounds:{x:0,y:0,w:1000,h:1000}}:{}),
    projectileWorldBlocked:()=>false,
    clipSpellSegment:({start,end})=>profile.spellBlocked?{x:(start.x+end.x)*0.5,y:(start.y+end.y)*0.5}:end,
    lightAt:()=>profile.light??1,
    ...(profile.view?{[profile.movementViewInput??'spiderMovementView']:{arenaBounds:{x:0,y:0,w:1000,h:1000},
      cameras:[profile.view==='offscreen'?{x:2000,y:2000,w:500,h:500}:{x:0,y:0,w:1000,h:1000}],enhancedEffects:profile.view!=='basic'}}:{})}
  if (sharedLootRng) context.rollLootSeed = bound => {
    const draw = nativeRng.drawNativeInteger(sharedLootRng,bound)
    sharedLootRng = draw.state
    return draw.value
  }
  if (profile.puppets) context.puppetTargets=['scenery','goodie','meteor','leviathan','arrow','firebolt'].map((hitKind,index)=>({
    active:true,actorFlags:hitKind==='meteor'?8:hitKind==='leviathan'?0x200:4,attachment:{x:0,y:0},bodyRadius:0,
    cellBindingOrder:10+index,hitKind,id:`fixture:${hitKind}`,kind:'projectile',nativePriority:0,pendingRemove:false,
    position:{x:275+index*20,y:250},queryLane:index>=4?'transient':'grid',registrationOrder:10+index,
    ...(hitKind==='meteor'?{gridPosition:{x:0,y:0}}:{})}))
  const input = {enemyToken:profile.token,flags:profile.flags,id:1,locationPolicy:'anywhere',
    nativeTypeId:schema.BONEYARD_WAVE_ENEMY_TYPES[profile.token],position:{x:150,y:250},spawnTick:10,waveOrdinal:31,
    zombieBodyType:profile.zombieBodyType??0,
    ...(profile.token==='PORTAL'?{authoredRecipe:{...portalRecipe,family:{kind:'portal',frequency:profile.frequency}}}:{}),
    ...(profile.authored?{authoredRecipe:configs.cases.find(c=>c.token==='SKELETONARCHER' && c.authoredRecipe!==null).authoredRecipe}:{}),
    ...(profile.token==='HEARTMONGER'?{authoredRecipe:{...heartmongerRecipe,
      family:{...heartmongerRecipe.family,summonMode:profile.summonMode}}}:{}),
    ...(profile.token==='DIREFACULTY'?{authoredRecipe:{...facultyRecipe,
      family:{...facultyRecipe.family,primary:profile.primary,secondary:profile.secondary}}}:{}),
    ...(profile.token==='DEMONSKULL'?{authoredRecipe:{...skullRecipe,family:{...skullRecipe.family,capabilities:profile.capabilities}}}:{}),
    ...(profile.token==='COCOON'?{cocoonTargetPlayerId:'hero'}:{})}
  const lateIntents = profile.lateBirths ? [21,71,180].map((spawnTick,index)=>({
    enemyToken:'IMP',flags:[],id:500+index,locationPolicy:'anywhere',nativeTypeId:1004,
    position:{x:190,y:250},spawnTick,waveOrdinal:31})) : []
  const initial = workModule.createEnemyWork(stores.createBoneyardEnemyStore(seed,50),context,true)
  const intents=profile.contactBirth?[]:[input]
  if (profile.initialImps) for (let index=1;index<profile.initialImps;index+=1) intents.push({...input,id:index+1,flags:[],position:{x:150+(index%10)*30,y:250+Math.floor(index/10)*30}})
  if (profile.manySpiders) intents.push({...input,id:2,position:{x:165,y:255}})
  if (profile.allies) {
    intents.push({...input,enemyToken:'SKELETON',nativeTypeId:1001,flags:[],id:2,position:{x:100,y:250}})
    intents.push({...input,enemyToken:'ZOMBIE',nativeTypeId:1006,flags:[],id:3,position:{x:200,y:250}})
  }
  initial.actors.push(...construction.materializeSpawnIntents(initial,context,intents))
  if (profile.megaDeath) initial.actors[0]={...initial.actors[0],brain:{...initial.actors[0].brain,capabilities:initial.actors[0].brain.capabilities|16}}
  if (profile.facultyAction) {
    const created=facultyActions.createNativeFacultyAction(profile.facultyAction,initial.steeringRngState)
    initial.steeringRngState=created.rng
    initial.actors[0]={...initial.actors[0],brain:{...initial.actors[0].brain,action:created.action,
      handMask:created.action.handMask,primaryPoll:10000,secondaryCooldown:10000}}
  }
  if (profile.skullAction) initial.actors[0]=demonSkullActions.beginDemonSkullAction(initial,initial.actors[0],profile.skullAction,context,initial.actors[0].config.attackSpeed)
  if (profile.webbed) initial.webbedPlayers.hero={severity:3,strength:45,cocoonHealth:45,hitPulse:0}
  if (profile.opening) initial.actors[0].brain={...initial.actors[0].brain,phase:'opening',phaseTicksRemaining:1,phaseTick:45}
  let store = workModule.finishEnemyStore(initial,10)
  if (profile.contactBirth) store=construction.addNativeCocoon(store,'hero',player.position,context.players,10,order.register).store
  const samples = []
  let dampenedAfterTrail = false
  for (let tick=11;tick<=10+(profile.ticks??400);tick+=1) {
    const near = profile.motion==='near' || profile.motion==='leaves'
    context.tick = tick
    context.paused = profile.pause && tick>=50 && tick<90
    context.dialogueBusy = profile.statuses && tick>=400 && tick<450
    const liveCounts = []
    context.resolveSpawnIntents = (enemies,zombies,bosses) => {
      liveCounts.push({enemies,zombies,bosses})
      return lateIntents.filter(intent=>intent.spawnTick===tick)
    }
    context.abilityEffects = profile.statuses ? Object.fromEntries(store.actors.map(actor=>[actor.id,{
      timeScale:tick>=100&&tick<140?0:tick>=40&&tick<100?.35:1,
      weakenFactor:tick>=60&&tick<180?.25:1,disruptedTicks:tick>=180&&tick<210?1:0,
      fleeTicks:tick>=260&&tick<310?1:0,dazzleTicks:tick>=360&&tick<390?1:0}])) : undefined
    if (tick===profile.dampenAt) store=dampen.dampenBoneyardCasters(store,store.actors.map(actor=>actor.id),tick,order.register)
    player.position = profile.motion==='middle'?{x:375,y:250}:profile.motion==='follow'?{x:100+Math.floor((tick-11)/20)*5,y:200+(tick-11)%40}:
      near?{x:185,y:250}:{x:500+Math.floor((tick-11)/100)*50,y:500}
    if (profile.motion==='follow') player.headingDeg=Math.floor((tick-11)/10)*15%360
    player.eligible = !profile.noTargets && (profile.motion!=='leaves' || tick<120 || tick>=230)
    if (tick===profile.ownerDiesAt) player.alive=false
    if (tick===profile.removeCocoonAt) store=cocoons.removeCocoonOwner(store,'hero')
    if (tick===profile.removeOwnerAt) store={...store,actors:[]}
    if (tick===profile.removeActorAt) store={...store,actors:[]}
    if (tick===profile.killAt) store=damage.damageBoneyardEnemy(store,{actorId:store.actors[0].id,amount:1e9,
      tick,sourcePlayerId:'hero',etherDrainCapture:profile.etherCapture??false,registerWorldPainter:order.register}).store
    if (tick===profile.maggotHitAt) {
      const target=store.maggots.find(maggot=>maggot.combatActive && maggot.lifeState==='alive')
      if (!target) throw new Error(`${profile.name} requires an admitted Maggot before its recorded hit`)
      store=damage.damageBoneyardEnemy(store,{actorId:target.id,amount:500,tick,sourcePlayerId:'hero',registerWorldPainter:order.register}).store
    }
    const previousPhase=store.actors[0]?.brain.phase
    if (tick===profile.detachAt) {
      const detached = workModule.createEnemyWork(store,context,true)
      heartmonger.detachHeartmongerCrows(detached,store.actors[0],context)
      detached.actors=[]
      store=workModule.finishEnemyStore(detached,store.lastStepTick)
    }
    const deferred = profile.deferred ? managers.createDeferredNativeWorldManagerRegistrations() : null
    context.registerProjectileWorldPainter = deferred?.register
    const work = stores.stepBoneyardEnemyStore(store,context)
    store = work.store
    const dampenAfterTrail = profile.dampenAfterTrail && !dampenedAfterTrail
      && store.projectileEffects.some(effect=>effect.kind==='firebolt-trail' && effect.spawnTick===tick)
    if (dampenAfterTrail) {
      store=dampen.dampenBoneyardCasters(store,store.actors.map(actor=>actor.id),tick,order.register)
      dampenedAfterTrail=true
    }
    deferred?.commit(order)
    if (profile.applyWebContacts) {
      for (const damage of work.playerDamage) {
        if (damage.webbedStrength===undefined) continue
        const prior=store.webbedPlayers[damage.playerId]??null
        const web=webbed.applyNativeWebbed(prior,damage.webbedStrength)
        store={...store,webbedPlayers:{...store.webbedPlayers,[damage.playerId]:web}}
        if (web.severity===3 && (prior?.severity??0)<3) store=construction.addNativeCocoon(store,damage.playerId,player.position,context.players,tick,order.register).store
      }
    }
    const portal = profile.token==='PORTAL'
    const coffin = profile.token==='COFFIN'
    const birthBoundary = work.spawnedActorIds.length>0 || work.events.some(event=>event.sound==='portal-open')
    // Portal children soon produce a vocal most ticks. Retain every ejection and a
    // periodic complete state, rather than repeating their whole population per vocal.
    const openingBoundary=coffin && previousPhase==='opening' && store.actors[0]?.brain.phase==='open'
    const faculty=['DIREFACULTY','DEMONSKULL'].includes(profile.token)||profile.terminal
    if (tick<=14 || tick===10+(profile.ticks??400) || tick%((portal||coffin||faculty)?100:20)===0 || (portal?birthBoundary:coffin?openingBoundary:faculty?false:work.events.length>0)
      || tick===profile.removeOwnerAt || tick===profile.maggotHitAt
      || tick===profile.killAt || tick===profile.killAt+1 || work.retired.length>0 || dampenAfterTrail) {
      samples.push({tick,liveCounts,dampenAfterTrail,sharedLootRng:sharedLootRng?rngRecord(sharedLootRng):null,actors:store.actors,events:work.events,damage:work.playerDamage,
        knockbacks:work.playerKnockbacks.map(({actorId,delta,eventId,playerId})=>({actorId,delta,eventId,playerId})),
        retainedKnockbacks:store.projectileKnockbacks,positions:work.playerPositions,effects:store.deathEffects,
        rng:store.rngState,steering:rngRecord(store.steeringRngState),headFacing:rngRecord(store.headFacingRngState),
        nextEventId:store.nextEventId,nextDeathEffectId:store.nextDeathEffectId,painterOrder:order.state().nextRegistrationOrdinal,
        nextActorId:store.nextActorId,nextSyntheticSpawnIntentId:store.nextSyntheticSpawnIntentId,
        nextNativeCellBindingOrder:store.nextNativeCellBindingOrder,nextNativeRegistrationOrder:store.nextNativeRegistrationOrder,
        projectiles:store.projectiles,projectileEffects:store.projectileEffects,lightning:store.mageLightningPulses,
        nextProjectileId:store.nextProjectileId,nextProjectileEffectId:store.nextProjectileEffectId,nextMageLightningPulseId:store.nextMageLightningPulseId,
        maggots:store.maggots,retired:work.retired,spawned:work.spawnedActorIds,nextDeathEpoch:store.nextDeathEpoch,
        silks:store.silks,silkFragments:store.silkFragments,webbedPlayers:store.webbedPlayers,spiderSpitTicksRemaining:store.spiderSpitTicksRemaining,
        spiderRemains:store.spiderRemains,detachedCrows:store.detachedCrows,bossSpells:store.bossSpells,
        demonSkullEncounter:store.demonSkullEncounter,puppetHits:store.puppetHits,rewards:work.rewards,
        facultyVoiceController:store.facultyVoiceController,bossNarration:store.bossNarration})
    }
  }
  if (profile.dampenAfterTrail && !dampenedAfterTrail) throw new Error(`${profile.name} must reach a real competing primary-phase producer`)
  if (profile.applyWebContacts && (store.webbedPlayers.hero?.severity!==3 || !store.actors.some(actor=>actor.config.enemyToken==='COCOON'))) {
    throw new Error(`${profile.name} must reach a real contact-owned Cocoon`)
  }
  return {...profile,seed,intents,lateIntents,samples}
})
const cases = [...(retained?.cases??[]), ...recordedCases]
writeFixture('enemy-ai-goldens.json.zlib','tools/website-oracle/enemy-ai-goldens.mjs',
  'Current Website/frontend/src/game/core-server/boneyard-enemy-store.ts::stepBoneyardEnemyStore; actual Game deferred projectile registration boundary; family/projectile/terminal producers', {sourceRevision,cases,movementViewInputs})
console.log(`enemy AI: ${cases.length} sequences / ${cases.reduce((count,c)=>count+(c.ticks??400),0)} ticks / ${cases.reduce((count,c)=>count+c.samples.length,0)} snapshots`)
