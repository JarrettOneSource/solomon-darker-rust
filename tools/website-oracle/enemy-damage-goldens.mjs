#!/usr/bin/env node
// Stateful hit sequences exercise the public damage seam, including its RNG and births.
import { readFileSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const sourceRevision=execFileSync('git',['rev-parse','HEAD'],{cwd:websiteDir,encoding:'utf8'}).trim()

const stores = await loadKernel('core-server/boneyard-enemy-store.ts')
const construction = await loadKernel('core-server/enemies/construction.ts')
const workModule = await loadKernel('core-server/enemies/work.ts')
const damage = await loadKernel('core-server/enemies/damage.ts')
const targets = await loadKernel('core-server/spell-combat/targets.ts')
const schema = await loadKernel('core-kernels/boneyard-wave-schema.ts')
const managers = await loadKernel('core-kernels/native-world-manager-order.ts')
const audio = await loadKernel('core-kernels/native-boss-audio.ts')
const configs = JSON.parse(readFileSync(resolve(fixturesDir, 'enemy-config-goldens.json'), 'utf8'))
const recipe = token => structuredClone(configs.cases.find(c => c.token === token && c.authoredRecipe !== null).authoredRecipe)
const rngRecord = rng => [rng.indexA, rng.indexB, ...rng.words]

function intent(token, id = 1, extra = {}) {
  return { enemyToken: token, flags: [], id, locationPolicy: 'anywhere',
    nativeTypeId: schema.BONEYARD_WAVE_ENEMY_TYPES[token], position: {x:150,y:250},
    spawnTick: 10, waveOrdinal: 31, ...(token === 'PORTAL' ? {authoredRecipe:recipe(token)} : {}), ...extra }
}
const hit = (amount, tick, extra = {}) => ({actorId:1,amount,tick,sourcePlayerId:'hero', ...extra})
const profiles = Object.keys(schema.BONEYARD_WAVE_ENEMY_TYPES).filter(token => token !== 'COCOON').map(token => ({
  name:`ordinary-${token}`, intents:[intent(token)], hits:[hit(0.25,10),hit(0.25,11),hit(10000,30,{hasMagicDamage:true}),hit(1,31)],
}))
for (let headgear = 0; headgear <= 5; headgear += 1) {
  const armored = recipe('SKELETON')
  armored.family.headgear = headgear
  profiles.push({name:`armor-${headgear}`,intents:[intent('SKELETON',1,{authoredRecipe:armored,flags:['FLAG_ARMOR','FLAG_HPUP']})],
    setup:{pike:true},hits:[hit(10000,10),hit(10000,11)]})
}
profiles.push(
  {name:'shield-without-overflow',intents:[intent('SKELETON')],setup:{shield:5},
    hits:[hit(2,10),hit(2,11),hit(3,12),hit(1,13),hit(10000,14)]},
  {name:'portal-suppressed-hurt',intents:[intent('PORTAL')],hits:[hit(1,10,{suppressHurtSound:true}),hit(1,40)]},
  {name:'spider-magic-and-capture',intents:[intent('SPIDER')],
    hits:[hit(0.25,10,{magic:true}),hit(0.25,11),hit(10000,12,{etherDrainCapture:true})]},
  {name:'spider-committed-strafe',intents:[intent('SPIDER')],setup:{spiderCommitted:true},hits:[hit(1,10)]},
  {name:'boss-health-gates',intents:[intent('DEMONSKULL',1,{authoredRecipe:{...recipe('DEMONSKULL'),classification:'boss'},enableDiscorporealHealthGates:true})],
    hits:[hit(30,10),hit(30,11),hit(30,12),hit(30,13),hit(100,14)]},
  {name:'faculty-cancel-narration',intents:[intent('DIREFACULTY')],setup:{narration:true},hits:[hit(10000,10)]},
  {name:'archer-light-release',intents:[intent('SKELETONARCHER')],setup:{lighting:true},hits:[hit(10000,10)]},
  {name:'mage-lightning-release',intents:[intent('SKELETONMAGE')],setup:{mageLightning:true},hits:[hit(10000,10)]},
  {name:'bound-cocoon-release',intents:[intent('COCOON',1,{cocoonTargetPlayerId:'hero'}),intent('COCOON',2,{cocoonTargetPlayerId:'hero'})],
    setup:{webbed:true},hits:[hit(3,10,{magic:true}),hit(10,11),hit(1,12)]},
  {name:'unbound-cocoon',intents:[intent('COCOON')],hits:[hit(1,10,{magic:true})]},
)
const player = {alive:true,connected:true,eligible:true,collisionRadius:25,position:{x:500,y:500},velocityPerTick:{x:0,y:0}}
const cases = profiles.map(profile => {
  const seed = `damage:${profile.name}`
  const order = managers.createNativeWorldManagerOrder({nextRegistrationOrdinal:{actor:100,transient:70}})
  const context = {tick:10,players:{hero:player},resolveMovement:request=>request.requestedPosition,registerWorldPainter:order.register}
  const work = workModule.createEnemyWork(stores.createBoneyardEnemyStore(seed,50),context,true)
  work.actors.push(...construction.materializeSpawnIntents(work,context,profile.intents))
  const actor = work.actors[0]
  const setup = profile.setup ?? {}
  if (setup.shield) { actor.shieldHealth = setup.shield; actor.shieldMaximumHealth = setup.shield }
  if (setup.pike) {
    actor.brain.phase = 'attack'; actor.brain.actionProgress = 5; actor.bodyPose = 2
    actor.brain.pike = {playerId:'hero',position:{x:180,y:260},distance:103}
  }
  if (setup.spiderCommitted) { actor.brain.actionState = 3; actor.brain.strafeTicksRemaining = 50 }
  if (setup.lighting) actor.lighting = {charge:0.7,glow:0.25,providerCopies:2}
  if (setup.mageLightning) { actor.brain.phase = 'cast'; actor.brain.lightningTargetPlayerId = 'hero'; actor.brain.lightningTicksRemaining = 4 }
  if (setup.webbed) work.webbedPlayers.hero = {severity:3,strength:10,cocoonHealth:10,hitPulse:0}
  if (setup.narration) {
    work.bossNarration = audio.enqueueNativeBossNarration(work.bossNarration,['faculty-join-us-1'],10,false)
  }
  let store = workModule.finishEnemyStore(work,-1)
  const samples = profile.hits.map(request => {
    const result = damage.damageBoneyardEnemy(store,{...request,registerWorldPainter:order.register})
    store = result.store
    return {request,result:{accepted:result.accepted,healthDamage:result.healthDamage,killed:result.killed},
      actors:store.actors,events:result.events,effects:store.deathEffects,webbedPlayers:store.webbedPlayers,
      targets:targets.primaryTargetRows(store).map(row=>row.target),rng:store.rngState,
      steering:rngRecord(store.steeringRngState),nextDeathEpoch:store.nextDeathEpoch,
      nextDeathEffectId:store.nextDeathEffectId,nextEventId:store.nextEventId,
      featuredBossId:store.featuredBossId,demonSkullEncounter:store.demonSkullEncounter,
      bossNarration:store.bossNarration,painterOrder:order.state().nextRegistrationOrdinal}
  })
  return {...profile,seed,samples}
})
writeFixture('enemy-damage-goldens.json','tools/website-oracle/enemy-damage-goldens.mjs',
  'Current Website/frontend/src/game/core-server: enemies/damage.ts, enemies/cocoon.ts, enemies/skeleton-death.ts, spell-combat/targets.ts', {sourceRevision,cases})
console.log(`enemy damage: ${cases.length} sequences, ${cases.reduce((count,c)=>count+c.samples.length,0)} hits`)
