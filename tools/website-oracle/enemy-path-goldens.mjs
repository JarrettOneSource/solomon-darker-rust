#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { loadKernel, point, websiteDir, writeFixture } from './oracle.mjs'
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const path = await loadKernel('core-kernels/native-enemy-pathfinding.ts')
const native = await loadKernel('core-kernels/native-rng.ts')
const rngRecord = rng => [rng.indexA,rng.indexB,...rng.words]
// Optional vectors carry a presence word followed by their coordinates; absent values
// contribute zero coordinates. This preserves every path-state field without sentinels.
const stateRecord = state => [
  state.baseTurnRate,state.flankAngleDeg,state.flankRadius,state.flankTicksRemaining,
  state.reorientationTicksRemaining,state.routePreviousVector?1:0,
  state.routePreviousVector?.x??0,state.routePreviousVector?.y??0,
  state.routeRefreshTicksRemaining,state.routeTicksRemaining,state.routeWaypointIndex,
  state.routeWaypoints?1:0,state.routeWaypoints?.[0].x??0,state.routeWaypoints?.[0].y??0,
  state.routeWaypoints?.[1].x??0,state.routeWaypoints?.[1].y??0,
  state.speedFactor,state.stalledMovementTicks,state.turnFactor,state.wanderHeadingDeg,
]
const constructions = [-7,0,1,71,91,1999].map(seed => {
  const created = path.createNativeEnemyPathState(native.createNativeRng(seed))
  return {seed,state:stateRecord(created.state),rng:rngRecord(created.rngState)}
})
const steering = []
for (const seed of [1,71]) {
  const created = path.createNativeEnemyPathState(native.createNativeRng(seed))
  for (const flank of [false,true]) {
    const state = {...created.state,flankTicksRemaining:flank?3:0}
    for (const target of [null,[100,0],[-100,0],[0,-100],[0,100],[100,100]]) {
      for (const [radial,tangent] of [[1,0],[-1,0],[0,1],[0,-1]]) {
        const request = {actorHeadingDeg:359,actorPosition:{x:10,y:20},cadenceTicks:2,
          movementPerTick:0.375,radialDirection:radial,statusFactor:0.75,tangentDirection:tangent,
          targetHeadingDeg:90,targetPosition:target?{x:target[0],y:target[1]}:null}
        const goal = path.nativeEnemySteeringGoal(state,request)
        const result = path.buildNativeEnemySteering(state,request)
        steering.push({seed,flank,target,radial,tangent,goal:point(goal),
          delta:point(result.delta),heading:result.headingDeg,state:stateRecord(result.state)})
      }
    }
  }
}
const recovery = []
for (const seed of [1,71,91,125]) {
  const created = path.createNativeEnemyPathState(native.createNativeRng(seed))
  let rng = created.rngState
  let state = {...created.state,speedFactor:1.5,turnFactor:Math.fround(1.4)}
  const flankingEnabled = seed !== 125
  const samples = []
  for (let index=0;index<160;index+=1) {
    const blocked = index<70 || index>=100&&index<140
    const request = {flankingEnabled,requestedDistance:1,statusFactor:0.5,
      tick:index*2,traveledDistance:blocked?0:1}
    const result = path.stepNativeEnemyPathRecovery(state,rng,request)
    state = result.state
    rng = result.rngState
    samples.push([stateRecord(state),result.triggeredState0D])
  }
  recovery.push({seed,flankingEnabled,samples,rng:rngRecord(rng)})
}
const routes = []
for (const mode of ['direct','detour','missing','short']) {
  const created = path.createNativeEnemyPathState(native.createNativeRng(71))
  let state = created.state
  const samples = []
  for (const position of [[0,0],[85,0],[95,0],[100,85],[100,95],[200,200]]) {
    const request = {actorPosition:{x:position[0],y:position[1]},bodyRadius:20,cadenceTicks:2,
      navigationClearance:45,rawGoal:{x:200,y:50},targetPosition:{x:200,y:200},targetRefreshTicks:300,
      directPathClear:()=>mode==='direct',
      findRoute:()=>mode==='missing'?null:mode==='short'?[{x:0,y:0},{x:200,y:200}]
        :[{x:0,y:0},{x:100,y:0},{x:100,y:100},{x:200,y:200}]}
    const result = path.resolveNativeEnemyPathGoal(state,request)
    state=result.state
    samples.push({position,goal:point(result.goal),state:stateRecord(state),turnAround:result.turnAround})
  }
  routes.push({mode,samples})
}
const reorientation = []
for (const target of [null,[0,0],[100,0],[-100,100]]) {
  const created=path.createNativeEnemyPathState(native.createNativeRng(71))
  let state={...created.state,reorientationTicksRemaining:2}
  let heading=17
  const samples=[]
  for (let tick=0;tick<4;tick+=1) {
    const result=path.stepNativeEnemyReorientation(state,heading,{x:0,y:0},target?{x:target[0],y:target[1]}:null)
    state=result.state;heading=result.headingDeg
    samples.push({heading,state:stateRecord(state)})
  }
  reorientation.push({target,samples})
}
const ranges = []
for (const seed of [0,71]) {
  for (const [first,second] of [[1,1],[2,1],[-1,0.25],[0.1,0.1000000001]]) {
    const result=native.drawNativeFloatRange(native.createNativeRng(seed),first,second)
    ranges.push({seed,first,second,value:result.value,rng:rngRecord(result.state)})
  }
}
writeFixture('enemy-path-goldens.json','tools/website-oracle/enemy-path-goldens.mjs',
  'Website/frontend/src/game/core-kernels: native-enemy-pathfinding.ts, native-rng.ts',
  {sourceRevision,constructions,steering,recovery,routes,reorientation,ranges,
    targetRefreshTicks:[0,1,2,3].map(path.nativeEnemyTargetRefreshTicks)})
console.log(`enemy path: ${constructions.length} constructors, ${steering.length} steering cases, ${recovery.reduce((n,c)=>n+c.samples.length,0)} recovery epochs, ${routes.length} route scenarios`)
