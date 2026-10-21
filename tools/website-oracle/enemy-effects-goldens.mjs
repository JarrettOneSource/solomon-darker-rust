#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { loadKernel,websiteDir,writeFixture } from './oracle.mjs'
const sourceRevision=execFileSync('git',['rev-parse','HEAD'],{cwd:websiteDir,encoding:'utf8'}).trim()
const stores=await loadKernel('core-server/boneyard-enemy-store.ts')
const workModule=await loadKernel('core-server/enemies/work.ts')
const effects=await loadKernel('core-server/enemies/death-effects.ts')
const transients=await loadKernel('core-server/boneyard-transient-effects.ts')
const random=await loadKernel('core-server/enemies/random.ts')
const managers=await loadKernel('core-kernels/native-world-manager-order.ts')
const kinds=['banish','bouncer','smoky-bouncer','fade','fade-additive','fade-perspective',
  'fade-perspective-clipped','fade-scale-perspective','fade-scale','fire-array','late-splat',
  'move-fade','move-fade-perspective','sprite-array','unbind','banish-black','scrap',
  'black-smoky-bouncer','move-fade-sin']
const owner={id:11,position:{x:200.5,y:300.25}}
const order=managers.createNativeWorldManagerOrder()
const work=workModule.createEnemyWork(stores.createBoneyardEnemyStore('effect-oracle'),
  {tick:0,registerWorldPainter:order.register},true)
for (const [index,kind] of kinds.entries()) {
  if (kind==='bouncer'||kind==='smoky-bouncer'||kind==='black-smoky-bouncer') {
    effects.spawnBouncer(work,owner,0,()=>92+random.drawInteger(work,4),`oracle-${kind}`,()=>({
      kind,height:-5,scale:1.25,opacityTimer:2,
      velocity:random.radialVector(random.drawUnit(work)*360,1.5),
    }))
  } else {
    effects.spawnSimpleDeathEffect(work,owner,0,{
      alpha:1,alphaLossPerTick:0.02,atlas:'BadGuys',blendMode:'add',entry:92,
      kind,lifetimeTicks:42,role:`oracle-${kind}`,scale:1.25,
      opacityTimer:2,frameCount:4,frameVelocity:kind==='move-fade-sin'?15:0.5,
      frameVelocityDamping:0.95,frameTicks:2,velocity:{x:0.6,y:-0.25},velocityDamping:0.9,
      scaleY:0.75,scaleMultiplier:1.1,rotationDeg:10,angularVelocityDeg:2,
      ...(index===0?{painterSortBias:-0.25}:{}),
      ...(index===3?{presentationOwner:'pre-world-queue'}:{}),
      ...(index===4?{spawnDelayTicks:2}:{}),
      ...(kind==='scrap'?{scrapOscillation:{phaseDeg:10,stepDeg:11,amplitudeDeg:30}}:{}),
    })
  }
}
const births=work.deathEffects.map(effect=>structuredClone(effect))
work.deathEffects=transients.stepBoneyardPreWorldEffectBirths(work.deathEffects,0,()=>random.drawUnit(work))
const snapshots=[]
for (const tick of [0,1,2,3,4,7,10,20,24,40,60,120]) {
  const stepped=transients.stepBoneyardTransientEffects(work.deathEffects,[],tick,()=>random.drawUnit(work),
    work.nextDeathEffectId,order.register)
  work.deathEffects=stepped.deathEffects
  work.nextDeathEffectId=stepped.nextDeathEffectId
  snapshots.push({tick,effects:structuredClone(work.deathEffects),rng:work.rngState,
    nextId:work.nextDeathEffectId,painterOrder:order.state().nextRegistrationOrdinal})
}
writeFixture('enemy-effects-goldens.json','tools/website-oracle/enemy-effects-goldens.mjs',
  'Website/frontend/src/game/core-server: enemies/death-effects.ts, boneyard-transient-effects.ts',
  {sourceRevision,kinds,births,snapshots})
console.log(`enemy effects: ${kinds.length} native programs, ${births.length} birth records, ${snapshots.length} snapshots through tick120`)
