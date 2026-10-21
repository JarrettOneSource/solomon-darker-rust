import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, fixturesDir, writeFixture } from './oracle.mjs'
const revision = execFileSync('git', ['-C', websiteDir, 'rev-parse', 'HEAD'], { encoding:'utf8' }).trim()
assert.equal(revision, 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83')
const primary = await loadKernel('core-kernels/primary-spells.ts')
const factory = await loadKernel('core-kernels/native-weld-flame-birth.ts')
const character = await loadKernel('core-kernels/player-character.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const managers = await loadKernel('core-kernels/native-world-manager-order.ts')
const profile = JSON.parse(readFileSync(resolve(fixturesDir,'primary-profile-goldens.json'),'utf8')).profiles.find(row=>row.name==='weld-1003-frozen').profile
const target = { active:true,actorFlags:2,attachment:{x:0,y:-3},bodyRadius:15,cellBindingOrder:0,headingDeg:0,
  id:'enemy:1',kind:'enemy',nativePriority:0,pendingRemove:false,position:{x:450,y:200},queryLane:'grid',registrationOrder:0 }
const cases=[]
for (const [name,bounds,targets,enhancedEffects,mana] of [
  ['target-clipped',{x:20,y:50,w:250,h:200},[target],true,1000],
  ['target-collapsed',{x:0,y:0,w:20,h:20},[target],true,1000],
  ['weak-coarse',null,[],false,.001],
  ['untargeted',null,[],true,1000],
]) {
  let player=character.createPlayerCharacter({element:'ether',discipline:'arcane',displayName:'Flame oracle'},{x:100,y:200})
  let spells=primary.createPrimarySpellSimulation(),rng=random.createNativeRng(731)
  const order=managers.createNativeWorldManagerOrder({nextRegistrationOrdinal:{actor:49,transient:3}})
  for(let tick=0;tick<80;tick++) {
    const result=primary.stepPrimarySpells({enhancedEffects,canPlaceProjectile:()=>true,canTraverseProjectile:()=>true,
      castAuthority:{hero:{weaponKind:'staff',alive:true,availableMana:mana,castProgressFactor:1,eligible:true,planeActive:false,primarySkill:profile}},
      inputs:{hero:{aim:{x:700,y:200},cast:{primary:true,quickbar:null},movement:{x:0,y:0},viewportWidth:800,viewportHeight:600}},
      players:{hero:player},previousPlayers:{hero:player},registerWorldPainter:order.register,rng,spells,tick,viewScale:1.2,
      spellObstructionPoint:()=>null,spellRangeEndpoint:(_owner,start,direction)=>({x:start.x+direction.x*600,y:start.y+direction.y*600}),
      spellTargets:()=>targets,worldKeyForPlayer:()=> 'hub:courtyard',playerLightRegistration:()=>({managerLane:'actor',registrationOrdinal:48}),
      ...(bounds?{flameLashViewBounds:()=>bounds}:{}),
    })
    player=result.players.hero;spells=result.spells;rng=result.rng
    if(result.channelEmissions.length===0)continue
    const emission=result.channelEmissions[0]
    const before={nextId:spells.nextId,actors:spells.transients.length,rng}
    const finalized=factory.finalizeHubFlameLashVisuals(spells,result.channelEmissions,tick,rng,enhancedEffects,order.register)
    cases.push({name,bounds,targets,enhancedEffects,mana,tick,profile,presentation:emission.flameLashPresentation,before,
      after:{nextId:finalized.spells.nextId,rng:finalized.rng,order:order.state().nextRegistrationOrdinal,
        roots:finalized.spells.transients.map(actor=>({id:actor.id,kind:actor.kind,painterRegistrations:actor.painterRegistrations,
          lightRegistration:actor.lightRegistration,variant:actor.variant}))}})
    break
  }
}
assert.equal(cases.length,4)
writeFixture('flame-cast-goldens.json','tools/website-oracle/flame-cast-goldens.mjs',
  `Website@${revision}/core-kernels/primary-spells.ts and native-weld-flame-birth.ts`,{cases})
