#!/usr/bin/env node
// Pinned kernels execute unchanged. The controller fixture supplies already-resolved
// character positions; actor physics remains the enclosing Hub epoch's responsibility.
import {loadKernel, point, writeFixture} from './oracle.mjs'

const college = await loadKernel('core-kernels/native-college-intro.ts')
const participant = await loadKernel('core-kernels/hub-participant-movement.ts')
const regions = await loadKernel('core-kernels/hub-regions.ts')
const character = await loadKernel('core-kernels/player-character.ts')
const admission = await loadKernel('core-kernels/college-admission-lifecycle.ts')
const appearance = await loadKernel('core-kernels/native-starter-equipment.ts')
const rng = await loadKernel('core-kernels/native-rng.ts')
const simulation = await loadKernel('core-server/game-simulation.ts')
const hubWorld = await loadKernel('core-server/hub-world.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const config = {displayName:'Sirmin',element:'ether',discipline:'arcane'}

const paths = []
for (const phase of ['courtyard-walk','office-walk']) {
  const source = phase === 'courtyard-walk' ? college.NATIVE_COLLEGE_COURTYARD_PATH : college.NATIVE_COLLEGE_OFFICE_PATH
  for (const knot of source) {
    const offset = phase === 'office-walk' ? 102.5 : 0
    for (const cursor of [-1,0,1,2.5,4,5.75,99]) {
      const position={x:knot.x+offset,y:knot.y+offset}
      const target=college.nativeCollegePathTarget(phase,cursor,position)
      paths.push({phase,cursor,position:point(position),pathCursor:target.pathCursor,target:point(target.target),
        heading:college.nativeCollegePathHeadingIndex(phase,cursor,position)})
    }
  }
}
const titles=[]
for(const cursor of [-1,0,.25,1,2,3,4,4.000001,4.5,5,99]) {
  for(const cover of [-1,0,.5,1,2]) titles.push({cursor,cover,result:college.nativeCollegeTitlePresentation(cursor,cover)})
}
const speeds=[]
for(const cursor of [0,4,4.000001,5,6]) {
  for(const speed of [.499,.5,.51,1]) speeds.push({cursor,speed,result:college.nativeCollegeOfficeSpeed(cursor,speed)})
}
const contacts=[]
for(const counter of [0,2,8,10,12]) {
  for(const eligible of [false,true]) contacts.push({counter,eligible,result:college.nativeCollegeContactStep(counter,eligible)})
}

const transition=(phase,source='office',destination='courtyard')=>({alpha:.5,destination,phase,scriptedSpeed:1,scriptedTarget:{x:512,y:2024},sourceRegion:source})
const states=[
 {region:'courtyard',collegeIntro:null,transition:null},
 regions.createHubCollegeIntroParticipantState(),
 {region:'office',collegeIntro:null,transition:null},
 {region:'courtyard',collegeIntro:null,transition:transition('college-loadout')},
 {region:'courtyard',collegeIntro:null,transition:transition('incoming')},
 {region:'office',collegeIntro:null,transition:transition('outgoing')},
 {region:'office',collegeIntro:college.enterNativeCollegeOffice(college.createNativeCollegeIntroState()),transition:transition('incoming','courtyard','office')},
 {region:'office',collegeIntro:college.enterNativeCollegeDialogue(college.createNativeCollegeIntroState()),transition:null},
]
const predicates=[]
for(const state of states) for(const pending of [false,true]) {
 predicates.push({state,pending,unstarted:participant.hubCollegeIntroUnstarted(state),
  preLoadout:admission.hubCollegeAdmissionPreLoadout(state,pending),
  primaryUnset:admission.hubCollegeAdmissionPrimaryUnset(state,pending),
  collisionBypass:hubWorld.hubParticipantOnboardingCollisionBypassActive(state,pending)})
}
const arms=[]
for(const state of states) for(const pending of [false,true]) for(const selectedPrimaryId of [-1,8]) {
 const seed=731942
 let before=simulation.createGameSimulation({p:config})
 before={...before,world:{...before.world,participants:{p:state}},playerEntities:{...before.playerEntities,
  progressions:before.playerEntities.progressions.map(entry=>({...entry,offerSeed:seed}))}}
 const player=simulation.getPlayerCharacter(before,'p')
 const economy=simulation.getPlayerEconomy(before,'p')
 before={...before,playerEntities:entities.replacePlayerEconomy(before.playerEntities,'p',{...economy,collegeIntroPending:pending})}
 before={...before,playerEntities:entities.replacePlayerCharacter(before.playerEntities,'p',{...player,
  position:{x:123,y:456},primaryCast:{...player.primaryCast,castSequence:13,selectedPrimaryId}})}
 const after=simulation.armGameSimulationCollegeIntro(before,'p')
 const final=simulation.getPlayerCharacter(after,'p')
 arms.push({state,pending,selectedPrimaryId,seed,begin:before.world!==after.world,reset:final.primaryCast.castSequence!==13,
  spawn:before.world!==after.world?{position:point(final.position),heading:final.headingIndex}:null,
  appearance:pending?appearance.rollNativeStarterEquipmentAppearance(rng.createNativeRng(seed),'college'):null})
}

let current=regions.createHubCollegeIntroParticipantState()
let player=character.createPlayerCharacter(config,college.NATIVE_COLLEGE_COURTYARD_PATH[0])
player={...player,headingIndex:college.nativeCollegePathHeadingIndex('courtyard-walk',0,player.position)}
let pending=true
let dialogueHold=0
let loadoutHold=0
let exiting=false
const samples=[]
const actions=[]
let completedTick=-1
function snapshot(tick,plan,target,clearPending) {
 return {tick,position:point(player.position),velocity:point(player.velocity),heading:player.headingIndex,
  gait:player.gaitDegrees,walk:player.walkCyclePrimary,participant:current,pending,clearPending,
  plan:plan?{delta:point(plan.delta),active:plan.movementActive,requested:point(plan.requestedVelocity),retained:point(plan.retainedVelocity)}:null,
  target:target?{cursor:target.pathCursor,position:point(target.target)}:null}
}
for(let tick=0;tick<5000;tick++) {
 const beforePhase=current.collegeIntro?.phase??null
 const beforeTransition=current.transition?.phase??null
 if(beforePhase==='arch-dialogue'&&++dialogueHold===6) {
  current={...current,collegeIntro:null};exiting=true;actions.push([tick,'acknowledge'])
 }
 if(beforeTransition==='college-loadout'&&++loadoutHold===8) {
  current={...current,transition:{...current.transition,phase:'incoming'}};actions.push([tick,'confirm'])
 }
 const ready=tick>=12
 const waiting=current.collegeIntro!==null&&!ready
 const movement=participant.planHubParticipantMovement(player,current,waiting)
 const plan=movement.plan??character.planPlayerCharacterTick(player,{movement:{x:0,y:exiting?1:0}},1)
 const position={x:player.position.x+plan.delta.x,y:player.position.y+plan.delta.y}
 const committed=character.commitPlayerCharacterTick(player,plan,position)
 const previous=current
 const stepped=participant.stepHubParticipantMovement(current,committed,movement.collegeTarget,{collegeIntroPending:pending,collegeIntroWaiting:waiting})
 current=stepped.participant;player=stepped.player
 const clearPending=pending&&previous.transition?.phase==='incoming'&&previous.transition.sourceRegion==='office'
  &&previous.transition.destination==='courtyard'&&current.region==='courtyard'&&current.transition===null
 if(clearPending)pending=false
 const changed=beforePhase!==(current.collegeIntro?.phase??null)||beforeTransition!==(current.transition?.phase??null)
 if(tick%7===0||changed||clearPending||tick===11||tick===12) samples.push(snapshot(tick,movement.plan,movement.collegeTarget,clearPending))
 if(!pending){completedTick=tick;break}
}
if(completedTick<0)throw new Error('source College admission did not complete')
writeFixture('college-intro-goldens.json','tools/website-oracle/college-intro-goldens.mjs',[
 'core-kernels/native-college-intro.ts','core-kernels/hub-participant-movement.ts',
 'core-kernels/college-admission-lifecycle.ts','core-kernels/player-character.ts',
 'core-kernels/native-starter-equipment.ts','core-server/game-simulation.ts','core-server/hub-world.ts',
],{paths,titles,speeds,contacts,predicates,arms,journey:{actions,samples,completedTick}})
console.log(JSON.stringify({paths:paths.length,arms:arms.length,samples:samples.length,completedTick,actions}))
