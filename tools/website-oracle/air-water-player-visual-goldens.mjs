import { execFileSync } from 'node:child_process'
// Full public cast → player visual synchronization → finalization journeys.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const primary = await loadKernel('core-kernels/primary-spells.ts')
const character = await loadKernel('core-kernels/player-character.ts')
const host = await loadKernel('core-server/air-water-player-visual-system.ts')
const presentation = await loadKernel('renderer/primary-spell-air-water-native.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const managers = await loadKernel('core-kernels/native-world-manager-order.ts')
const profiles = JSON.parse(readFileSync(resolve(fixturesDir,'primary-profile-goldens.json'),'utf8')).profiles
const profile = { ...profiles.find(value=>value.name==='water-fresh').profile,
  auraRadiusScale:1,hailThreshold:125,hailDamageMinimum:0.25,hailDamageMaximum:1.5,widenHalfDegrees:10 }
const point = value=>[value.x,value.y]
const reg = value=>value?[value.managerLane,value.registrationOrdinal]:null
const physicsKeys=['alphaDecay','durationTicks','initialRotationDegrees','rotationStepDegrees',
  'charge','contactCharge','damageMaximum','damageMinimum','enhancedEffects','phaseDegrees',
  'bounceProgress','bounceSoundIndex','bounceSoundPitch','bounceSoundSequence','height','life',
  'rotationDegrees','savedBounceVelocity','scale','verticalVelocity']
function actorRow(actor) {
  const fields=Object.fromEntries(physicsKeys.filter(key=>key in actor).map(key=>[key,actor[key]]))
  for(const key of ['origin','position','horizontalVelocity'])if(key in actor)fields[key]=point(actor[key])
  if(actor.lanes)fields.lanes=actor.lanes.map(lane=>[lane.angleDegrees,lane.angularVelocityDegrees,lane.radius,lane.verticalOffset])
  return {age:actor.ageTicks,birth:actor.birthTick??null,id:actor.id,kind:actor.kind,owner:actor.ownerId,world:actor.worldKey,
    light:reg(actor.lightRegistration),painters:(actor.painterRegistrations??[]).map(reg),fields}
}
function visualRow(actor) {
  if(actor.kind==='air-hurricane') {
    const plan = value=>presentation.nativeHurricaneVisualPlan(value).map(draw=>[
      draw.alpha,draw.blend==='add',point(draw.position),draw.role,draw.rotationRadians,point(draw.scale),draw.tint])
    return {full:plan(actor),reduced:plan({...actor,enhancedEffects:false})}
  }
  if(actor.kind==='water-aura')return presentation.nativeWaterAuraVisualPlan(actor)
  if(actor.kind==='water-hail')return presentation.nativeHailVisualPlan(actor)
  return null
}
const cases=[]
const screens=[]
for(const mode of ['water-aura-hail','underpowered-water','hurricane-owner-world-switch']) {
  const order=managers.createNativeWorldManagerOrder({nextRegistrationOrdinal:{actor:50,transient:3}})
  let player=character.createPlayerCharacter({element:'water',discipline:'arcane',displayName:'Weather oracle'},{x:150,y:250})
  let spells=primary.createPrimarySpellSimulation(),rng=random.createNativeRng(931),mana=mode==='underpowered-water'?0.001:1000
  const frames=[]
  for(let tick=0;tick<=180;tick++) {
    const worldKey=tick<80?'boneyard:weather':'boneyard:next'
    const result=primary.stepPrimarySpells({
      canPlaceProjectile:()=>true,canTraverseProjectile:()=>true,
      castAuthority:{hero:{weaponKind:'staff',alive:true,availableMana:mana,castProgressFactor:1,
        eligible:true,planeActive:false,primarySkill:profile}},
      inputs:{hero:{aim:{x:700,y:250},cast:{primary:mode!=='hurricane-owner-world-switch'&&tick<40,quickbar:null},movement:{x:0,y:0}}},
      players:{hero:player},previousPlayers:{hero:player},registerWorldPainter:order.register,
      rng,spells,tick,viewScale:1.2,spellObstructionPoint:()=>null,
      spellRangeEndpoint:(_owner,start,direction)=>({x:start.x+direction.x*600,y:start.y+direction.y*600}),
      spellTargets:()=>[],worldKeyForPlayer:()=>worldKey,
    })
    player=result.players.hero;spells=result.spells;rng=result.rng;mana-=result.manaSpent.hero
    const owners=[]
    if(tick<150)owners.push({hurricaneContactCharge:mode.startsWith('hurricane')?(tick%20===0?0:0.25):0,
      hurricaneCharge:mode.startsWith('hurricane')?0.5:0,hurricaneDamageMaximum:4,hurricaneDamageMinimum:2,
      ownerId:'hero',position:{x:150+tick*0.5,y:250-tick*0.25},worldKey})
    if(mode.startsWith('hurricane')&&tick<100)owners.push({hurricaneContactCharge:1,hurricaneCharge:1,
      hurricaneDamageMaximum:6,hurricaneDamageMinimum:3,ownerId:'other',position:{x:300,y:300},worldKey:'hub:courtyard'})
    const synchronized=host.synchronizeAirWaterPlayerVisualActors(spells,owners,tick,rng,result.channelEmissions,order.register)
    const finished=host.finalizeAirWaterPlayerVisualActors(synchronized.spells,result.channelEmissions,tick,synchronized.rng,order.register)
    spells=finished.spells;rng=finished.rng
    if([0,1,2,6,12,29,39,40,60,80,100,120,149,150,160,180].includes(tick))frames.push({tick,mana,nextId:spells.nextId,
      actors:spells.transients.map(actorRow),visuals:spells.transients.map(visualRow),rng,order:order.state().nextRegistrationOrdinal})
    if((mode==='water-aura-hail'&&[29,80,120].includes(tick))||(mode.startsWith('hurricane')&&tick===39))screens.push({mode,tick,spells,worldKey})
  }
  cases.push({mode,frames})
}
writeFixture('air-water-player-visual-goldens.json','tools/website-oracle/air-water-player-visual-goldens.mjs',
  'Website/frontend/src/game/core-server/air-water-player-visual-system.ts and air-water-spell-actors.ts',{sourceRevision,profile,cases,screens})
