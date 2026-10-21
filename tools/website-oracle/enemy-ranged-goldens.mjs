#!/usr/bin/env node
import { loadKernel,point,writeFixture } from './oracle.mjs'
const ranged=await loadKernel('core-kernels/native-enemy-targeting.ts')
const native=await loadKernel('core-kernels/native-rng.ts')
const rngRecord=rng=>[rng.indexA,rng.indexB,...rng.words]
const ranges=[]
for (const family of ['archer','mage']) {
  for (const mode of [0,1,2,3]) {
    for (const seed of [0,71,1999]) {
      const result=ranged.constructNativeRangedAttackRange(family,mode,native.createNativeRng(seed))
      const restored=ranged.restoreNativeRangeEasyAfterVolley(result.range,result.rangeEasyPending)
      ranges.push({family,mode,seed,range:result.range,pending:result.rangeEasyPending,
        restoredRange:restored.range,restoredPending:restored.pending,rng:rngRecord(result.rngState)})
    }
  }
}
const volleys=[]
for (const seed of [1,71,91]) {
  for (const accuracyMode of [0,1,2,3]) {
    for (const multiArrowMode of [0,1,2,3]) {
      const request={accuracyMode,arrowType:seed===1?'normal':seed===71?'fire':'poison',extraArrows:4,
        multiArrowMode,origin:{x:10.25,y:20.75},privateSeed:seed,
        targetPosition:{x:450.5,y:190.125},targetVelocityPerTick:{x:0.75,y:-0.25}}
      const result=ranged.buildNativeArcherVolley(request,native.createNativeRng(128))
      volleys.push({seed,accuracyMode,multiArrowMode,arrowType:request.arrowType,
        aimOffsetRadius:result.aimOffsetRadius,effectiveAccuracyMode:result.effectiveAccuracyMode,
        shotPitch:result.shotPitch,rng:rngRecord(result.sharedRngState),
        arrows:result.arrows.map(arrow=>[arrow.headingDeg,arrow.lifetimeTicks,arrow.position.x,arrow.position.y,
          arrow.speed,arrow.visualHeadingDeg,...point(ranged.nativeEnemyProjectileVelocity(arrow.headingDeg,arrow.speed))])})
    }
  }
}
writeFixture('enemy-ranged-goldens.json','tools/website-oracle/enemy-ranged-goldens.mjs',
  'Website/frontend/src/game/core-kernels: native-enemy-targeting.ts, native-rng.ts',
  {ranges,volleys,fanOffsets:Array.from({length:10},(_,index)=>ranged.nativeArcherFanOffset(index))})
console.log(`enemy ranged: ${ranges.length} range cases, ${volleys.length} volleys / ${volleys.reduce((n,v)=>n+v.arrows.length,0)} arrows`)
