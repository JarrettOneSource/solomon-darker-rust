#!/usr/bin/env node
import { loadKernel,writeFixture } from './oracle.mjs'
const voices=await loadKernel('core-kernels/native-faculty-voices.ts')
const audio=await loadKernel('core-kernels/native-boss-audio.ts')
const native=await loadKernel('core-kernels/native-rng.ts')
const rngRecord=rng=>[rng.indexA,rng.indexB,...rng.words]
const cases=[]
for (const seed of [1,71,91]) {
  for (let line=0;line<6;line+=1) {
    for (const members of [[],[false],[true],[false,true]]) {
      for (const busy of [false,true]) {
        let state={line,ticksRemaining:1}
        let rng=native.createNativeRng(seed)
        const samples=[]
        for (let index=0;index<3 && state!==null;index+=1) {
          const result=voices.stepNativeFacultyVoices(state,rng,members.map(female=>({female})),busy)
          state=result.state;rng=result.rng
          samples.push({state,cues:result.cues})
        }
        cases.push({seed,line,members,busy,samples,rng:rngRecord(rng)})
      }
    }
  }
}
let narration=audio.createNativeBossNarration()
const narrationSamples=[]
for (let tick=0;tick<=1000;tick+=1) {
  if (tick===0) narration=audio.enqueueNativeBossNarration(narration,
    ['faculty-join-us-1','faculty-dead-is-better-1-female'],tick,true)
  narration=audio.stepNativeBossNarration(narration,tick,tick<5)
  if (tick===100) narration=audio.enqueueNativeBossNarration(narration,['faculty-join-us-2-female'],tick,false)
  if (tick===500) narration=audio.cancelNativeBossNarration(narration)
  if (tick===600) narration=audio.enqueueNativeBossNarration(narration,['faculty-join-us-2'],tick,false)
  if (tick<=30||tick>=365&&tick<=370||tick>=499&&tick<=530||tick>=600&&tick<=605||tick>=972&&tick<=1000) {
    narrationSamples.push({tick,state:structuredClone(narration)})
  }
}
writeFixture('enemy-voices-goldens.json','tools/website-oracle/enemy-voices-goldens.mjs',
  'Website/frontend/src/game/core-kernels: native-boss-audio.ts, native-faculty-voices.ts',
  {durations:Object.entries(audio.NATIVE_BOSS_STREAM_TICKS),cases,narrationSamples})
console.log(`enemy voices: ${cases.length} member/line/busy cases, ${narrationSamples.length} narration samples`)
