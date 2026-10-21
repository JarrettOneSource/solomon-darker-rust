#!/usr/bin/env node
// Reuse real constructor and world traces, then run the actual host projection and
// enemy presenters with the registered stock attachment tables.
import { readFileSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import assert from 'node:assert/strict'
import { resolve } from 'node:path'
import { registerHooks } from 'node:module'
import { fixturesDir, gameDir, loadKernel, readFixture, websiteDir, writeFixture } from './oracle.mjs'
const sourceRevision=execFileSync('git',['rev-parse','HEAD'],{cwd:websiteDir,encoding:'utf8'}).trim()
const appendMovementViews = process.env.SOLOMON_APPEND_ENEMY_MOVEMENT_VIEWS === '1'
const retained = appendMovementViews ? readFixture(resolve(
  process.env.SOLOMON_RETAINED_ENEMY_FIXTURES??fixturesDir, 'enemy-presentation-goldens.json',
)) : null
if (retained) assert.equal(retained.sourceRevision, sourceRevision)

// Vite permits the stock manifest's older JSON imports. Supply only that loader metadata;
// the pinned renderer and all of its draw calculations run unmodified.
registerHooks({load(url,context,nextLoad) {
  if (url.endsWith('.json')) return nextLoad(url,{...context,importAttributes:{type:'json'}})
  return nextLoad(url,context)
}})

const projection = await loadKernel('host/project-boneyard-enemies.ts')
const presentation = await loadKernel('renderer/native-enemy-presentation.ts')
const maggots = await loadKernel('renderer/native-maggot-presentation.ts')
const puppet = await loadKernel('core-kernels/native-puppet-hit.ts')
const underlay = await loadKernel('renderer/native-enemy-underlay.ts')
const projectileEffects = await loadKernel('renderer/native-enemy-projectile-effect-presentation.ts')
const deathEffects = await loadKernel('renderer/native-enemy-death-effect-presentation.ts')
const deathViews = await loadKernel('renderer/native-enemy-death-effect-view.ts')
const bossSpells = await loadKernel('renderer/native-boss-spell-presentation.ts')
const ultraBanish = await loadKernel('renderer/native-ultra-banish-presentation.ts')
const mageLightning = await loadKernel('renderer/native-mage-lightning-pulse-presentation.ts')
const darkLightning = await loadKernel('renderer/native-dark-lightning.ts')
const silkPresentation = await loadKernel('core-kernels/native-silk-presentation.ts')
const nativeRng = await loadKernel('core-kernels/native-rng.ts')
const tendrilPresentation = await loadKernel('renderer/native-heartmonger-tendrils.ts')
const rainPresentation = await loadKernel('renderer/native-rain-cloud.ts')
const attackPresentation = await loadKernel('renderer/native-enemy-attack-effect.ts')
const demonArticulation = await loadKernel('core-kernels/boneyard-demon-articulation.ts')
const atlasNames = ['BadGuys','DeadHawg','Demon','Faculty','Heartmonger','Unholy']
const points = Object.fromEntries(atlasNames.map(atlas => {
  const manifest = JSON.parse(readFileSync(resolve(gameDir,'..','editor','manifest',`${atlas.toLowerCase()}.json`),'utf8'))
  return [atlas,manifest.entries.map(entry=>entry.extras??[])]
}))
const authoredPoints = (atlas,entry) => {
  const record = points[atlas][entry]
  if (!record) throw new Error(`missing stock point record ${atlas}:${entry}`)
  return record
}
const artRecords = Object.fromEntries(atlasNames.map(atlas => {
  const manifest = JSON.parse(readFileSync(resolve(gameDir,'..','editor','manifest',`${atlas.toLowerCase()}.json`),'utf8'))
  return [atlas,manifest.entries.map(record => ({width:record.rect.w,height:record.rect.h,
    anchorX:record.rect.w/2-record.origin.x,anchorY:record.rect.h/2-record.origin.y}))]
}))
const shadowRecords = [
  {baseAlpha:.6,behindScalar:.2,direction:{x:.8,y:.6},distanceFraction:.3,
    projectionDistance:175,sourcePosition:{x:40,y:50},sourceRadius:200},
  {baseAlpha:.4,behindScalar:.8,direction:{x:-.6,y:.8},distanceFraction:.75,
    projectionDistance:75,sourcePosition:{x:400,y:350},sourceRadius:300},
]
const actors = retained?.actors??[]
const children = retained?.maggots??[]
const effects = retained?.effects??{projectile:[],death:[],boss:[]}
const effectCounts = new Map()
for (const [lane, rows] of Object.entries(effects)) {
  for (const row of rows) {
    const key = `${lane}:${row.kind}`
    effectCounts.set(key, (effectCounts.get(key)??0)+1)
  }
}
const meshes = retained?.meshes??[]
const meshCounts = new Map()
for (const row of meshes) meshCounts.set(row.kind, (meshCounts.get(row.kind)??0)+1)
const tendrilStates = new Map()
const rainStates = new Map()
const attackEffects = retained?.attackEffects??[]
for (const eventId of retained?[]:[1,317,0xffffffff]) {
  for (let ageTicks=0;ageTicks<=26;ageTicks+=1) {
    for (const kind of ['demon-bomb-muzzle','imp-contact','imp-landing-flare']) {
      attackEffects.push({kind,eventId,ageTicks,expected:kind==='imp-landing-flare'
        ?attackPresentation.nativeImpLandingFlareSample(eventId,ageTicks)
        :attackPresentation.nativeEnemyRawFireBurstSample(kind,eventId,ageTicks)})
    }
  }
}
const normalize = value => JSON.parse(JSON.stringify(value,(_key,value)=>ArrayBuffer.isView(value)?Array.from(value):value))
function recordMesh(kind,name,tick,id,presentationTick,expected) {
  const count = meshCounts.get(kind)??0
  if (count>=12) return
  meshCounts.set(kind,count+1)
  meshes.push({kind,name,tick,id,presentationTick,expected:normalize(expected)})
}
const deathViewRoot = {addChild() {}}
const deathViewTextures = {base:new Proxy({}, {get:()=>({})})}
const banishViews = new deathViews.NativeEnemyDeathEffectViews(deathViewRoot,deathViewTextures,deathViewRoot)

// Banish is drawn in the real retained view rather than a pure presenter. Capture that
// method's sprite and gradient commands through a sink, without a GPU or copied formulas.
function banishPlan(effect) {
  banishViews.update([effect],{x:-10000000,y:-10000000,w:1,h:1},900)
  const view = banishViews.views.get(effect.id)
  const gradients = []
  view.banishGraphics = {clear() {}}
  view.gradientRect = (x,y,width,height,start,end) => {
    if (width<=0||height<=0) return
    const parse = color => {
      const [r,g,b,a] = color.match(/[\d.]+/g).map(Number)
      return {color:r<<16|g<<8|b,alpha:a}
    }
    const a=parse(start),b=parse(end)
    gradients.push({x,y,width,height,startColor:a.color,endColor:b.color,startAlpha:a.alpha,endAlpha:b.alpha})
  }
  const sprite = () => {
    const value = {anchor:{set() {}},position:{},scale:{}}
    value.position.set = (x,y) => {value.position.x=x;value.position.y=y}
    value.scale.set = (x,y) => {value.scale.x=x;value.scale.y=y}
    return value
  }
  view.banishSprites = Array.from({length:4},sprite)
  view.updateBanish(effect,900)
  const layers = view.banishSprites.map((sprite,index)=>({alpha:sprite.alpha,atlas:'BadGuys',
    blendMode:sprite.blendMode,entry:Number(sprite.label.split(':')[2]),
    offset:{x:sprite.position.x,y:sprite.position.y},role:index<2?`banish-core-${index}`:`banish-upper-${index-2}`,
    rotationRadians:sprite.rotation,scale:sprite.scale.x,...(index<2?{}:{scaleY:sprite.scale.y}),tint:sprite.tint}))
  // The temporary command sink owns no Pixi resources and is not retained for another sample.
  banishViews.views.clear()
  return {gradients,layers}
}

function recordEffect(lane,kind,name,tick,id,expected,extra={}) {
  const key = `${lane}:${kind}`
  const count = effectCounts.get(key)??0
  if (count>=12) return
  effectCounts.set(key,count+1)
  effects[lane].push({kind,name,tick,id,expected,...extra})
}

function recordActor(group,name,tick,actor,players,exposure,hitOverride=false) {
  if (actor.brain.family==='spider' && actor.brain.phase==='captured') return
  const source = hitOverride ? {...actor,hitFeedback:puppet.receiveNativePuppetHit(tick)} : actor
  const visual = projection.projectBoneyardEnemies({actors:[source]},tick,{players,lightAt:()=>exposure})[0]
  const complexLighting = actor.id%2===0
  const applicationTick = tick+1234
  const target = actor.targetPlayerId===null ? null : players[actor.targetPlayerId]??null
  const pike = actor.brain.family==='skeleton' ? actor.brain.pike : null
  const context = {tick,applicationTick,complexLighting,spiderExposure:exposure,
    spiderTarget:target===null?null:{position:target.position,primaryTint:target.primaryTint},
    pikeTargetPosition:pike===null?null:players[pike.playerId]?.position??pike.position}
  const plan = presentation.nativeEnemyPresentationPlan(visual,tick,authoredPoints,complexLighting,applicationTick)
  const {facing,rootScale,layers,segments,spawnAgeTicks} = plan
  let attackEffectOrigin = actor.config.enemyToken==='IMP'
    ?attackPresentation.nativeImpContactBurstOrigin(actor.position,actor.headingDeg):null
  const controller = layers.find(layer=>layer.role==='demon-controller-body')
  if (controller) {
    const pose = Math.trunc((controller.entry-19)/18)
    const {nativeDemonBombBearing} = demonArticulation
    const entry = 19+pose*18+nativeDemonBombBearing(actor.headingDeg)/20
    const muzzle = authoredPoints('Demon',entry)[5]
    attackEffectOrigin = attackPresentation.nativeDemonBombMuzzleOrigin(actor.position,actor.headingDeg,
      {x:muzzle.x+controller.offset.x*visual.scale,y:muzzle.y},controller.offset.y*visual.scale)
  }
  let tendrils = null
  if (actor.brain.family==='heartmonger') {
    const key = `${group}:${name}:${actor.id}`
    const state = tendrilStates.get(key)??tendrilPresentation.createHeartmongerTendrils(
      actor.position,actor.headingDeg,nativeRng.createNativeRng(actor.id+actor.spawnTick)).state
    const phase = actor.brain.legPhase
    const entry = 200+Math.trunc(phase)*18+((Math.trunc((actor.headingDeg+10)/20)%18)+18)%18
    const moved = tendrilPresentation.moveHeartmongerTendrils(state,phase,actor.position,authoredPoints('Heartmonger',entry))
    tendrilStates.set(key,moved)
    tendrils = tendrilPresentation.heartmongerTendrilLayers(moved,actor.position,shadowRecords[0].direction)
  }
  actors.push({group,name,tick,id:actor.id,hitOverride,context,expected:{facing,rootScale,layers,segments,spawnAgeTicks},
    underlay:underlay.nativeEnemyUnderlayPlan(visual,tick,shadowRecords,.8,complexLighting),tendrils,attackEffectOrigin})
}

const construction = JSON.parse(readFileSync(resolve(fixturesDir,'enemy-construction-goldens.json'),'utf8'))
for (const source of retained?[]:construction.cases) {
  for (const actor of source.actors) {
    const players = {hero:{position:{x:500,y:500},primaryTint:0x224466},ghost:{position:{x:100,y:100},primaryTint:0x335577}}
    recordActor('construction',source.name,10,actor,players,1)
    recordActor('construction',source.name,10,actor,players,1,true)
  }
}
const ai = readFixture('enemy-ai-goldens.json.zlib')
for (const source of ai.cases) {
  if (retained && source.movementViewInput !== 'nativeMovementView') continue
  if (retained) assert.ok(!retained.actors.some(row => row.name === source.name))
  const stride = Math.max(1,Math.ceil(source.samples.length/7))
  for (const [index,sample] of source.samples.entries()) {
    for (const effect of projection.projectBoneyardEnemyProjectileEffects({projectileEffects:sample.projectileEffects})) {
      const pointGain = effect.id%2===0?.5:2
      recordEffect('projectile',effect.kind,source.name,sample.tick,effect.id,
        projectileEffects.nativeEnemyProjectileEffectPlan(effect,pointGain),{pointGain})
    }
    for (const effect of sample.effects) {
      if (effect.kind==='black-smoky-bouncer') continue
      const visual = projection.projectBoneyardEnemyDeathEffect(effect)
      recordEffect('death',effect.kind,source.name,sample.tick,effect.id,
        effect.kind==='banish'||effect.kind==='banish-black'?banishPlan(visual):deathEffects.nativeEnemyDeathEffectPlan(visual))
    }
    for (const spell of sample.bossSpells) {
      let rain = null
      if (spell.kind==='rain-of-bones') {
        const key = `${source.name}:${spell.id}`
        const state = rainStates.get(key)??new rainPresentation.NativeRainCloud(spell,sample.tick)
        rainStates.set(key,state)
        rain = state.update(spell,sample.tick)
      }
      recordEffect('boss',spell.kind,source.name,sample.tick,spell.id,{
        layers:bossSpells.nativeBossSpellLayers(spell,sample.tick),
        gradients:spell.kind==='ultra-banish'?ultraBanish.nativeUltraBanishPlan(spell,sample.tick,900).gradients:[],
      },{rain})
      if (spell.kind==='blightning') recordMesh('dark-lightning',source.name,sample.tick,spell.id,sample.tick,
        darkLightning.nativeDarkLightningBody(spell))
    }
    for (const pulse of sample.lightning) {
      for (const age of [0,1,2,4,5]) recordMesh(`mage-${pulse.contact.kind}-${age}`,source.name,sample.tick,
        pulse.id,pulse.tick+age,mageLightning.nativeMageLightningPulsePlan(pulse,pulse.tick+age))
    }
    for (const silk of sample.silks) {
      const presentationTick = sample.tick+1234
      recordMesh(silk.state.ended?'silk-ended':'silk-flight',source.name,sample.tick,silk.id,presentationTick,
        silkPresentation.nativeSilkMesh(silk.state,p=>Math.max(0,Math.min(1,p.x/800+p.y/1600)),
          nativeRng.createNativeRng(silk.id^presentationTick)))
    }
    if (index!==source.samples.length-1 && index%stride!==0) continue
    const tick = sample.tick
    const motion = source.motion
    const position = sample.positions.hero ?? (motion==='middle'?{x:375,y:250}:
      motion==='follow'?{x:100+Math.floor((tick-11)/20)*5,y:200+(tick-11)%40}:
      motion==='near'||motion==='leaves'?{x:185,y:250}:{x:500+Math.floor((tick-11)/100)*50,y:500})
    const players = {hero:{position,primaryTint:0x224466}}
    for (const actor of sample.actors.slice(0,3)) recordActor('ai',source.name,tick,actor,players,source.light??1)
    for (const maggot of sample.maggots.slice(0,2)) {
      const visual = projection.projectBoneyardMaggots({maggots:[maggot]},tick)[0]
      children.push({name:source.name,tick,id:maggot.id,complexLighting:true,
        expected:maggots.nativeMaggotPresentationPlan(visual,true).layers})
    }
  }
}
writeFixture('enemy-presentation-goldens.json','tools/website-oracle/enemy-presentation-goldens.mjs',
  'Website host/project-boneyard-enemies.ts and renderer/native-enemy-presentation.ts with all actual family presenters; stock editor manifests',
  {sourceRevision,points,artRecords,shadowRecords,actors,maggots:children,effects,meshes,attackEffects})
console.log(`enemy presentation: ${actors.length} actor plans and ${children.length} independent Maggot plans`)
console.log(`effect plans: ${JSON.stringify(Object.fromEntries(effectCounts))}`)
console.log(`meshes: ${JSON.stringify(Object.fromEntries(meshCounts))}`)
