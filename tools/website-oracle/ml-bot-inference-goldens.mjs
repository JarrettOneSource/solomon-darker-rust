#!/usr/bin/env node
// Actual deployed v7 checkpoint and unchanged Source Float32 inference kernels.
import { readFileSync } from 'node:fs'
import { createHash } from 'node:crypto'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const checkpointKernel=await loadKernel('core-server/ml-bot-policy/checkpoint.ts')
const {MlBotPolicyRuntime}=await loadKernel('core-server/ml-bot-policy/runtime.ts')
const encoded=new Uint8Array(readFileSync(resolve(websiteDir,'frontend/server-assets/ml-bot-policy-v7-selected.sdml')))
const checkpoint=checkpointKernel.decodeMlBotPolicyCheckpoint(encoded)
const runtime=new MlBotPolicyRuntime(checkpoint)
const sha=bytes=>createHash('sha256').update(bytes).digest('hex')
let word=0x51acb077
function random(){word^=word<<13;word^=word>>>17;word^=word<<5;return(word>>>0)/4294967296}
const floats=(n,magnitude=1)=>Float32Array.from({length:n},()=>Math.fround((random()*2-1)*magnitude))
const array=x=>Array.from(x)
function result(value){
 return {...value,latent:array(value.latent),probabilities:value.probabilities instanceof Float32Array?array(value.probabilities):Object.fromEntries(Object.entries(value.probabilities).map(([k,v])=>[k,array(v)])),...(value.masks?{masks:Object.fromEntries(Object.entries(value.masks).map(([k,v])=>[k,array(v)]))}:{})}
}
const observations=[new Float32Array(3026),Float32Array.from({length:3026},(_,i)=>Math.fround((i%17-8)/8)),...Array.from({length:6},()=>floats(3026))]
const inferences=[]
for(let i=0;i<observations.length;i++) {
 const observation=observations[i]
 const masks={movement:Uint8Array.from({length:9},(_,j)=>Number(j%3!==(i%3)||j===0)),target:Uint8Array.from({length:9},(_,j)=>Number(j<Math.min(9,i+1))),ability:Uint8Array.from({length:22},(_,j)=>Number(j%4!==(i%4)||j===0)),aim:Uint8Array.from({length:9},(_,j)=>Number(j%2===i%2||j===0))}
 const plan={movement:masks.movement,target:masks.target,abilityByTarget:Array.from({length:9},(_,t)=>Uint8Array.from({length:22},(_,j)=>Number(j===0||j%3===t%3))),aimByAbility:Array.from({length:22},(_,a)=>Uint8Array.from({length:9},(_,j)=>Number(j===0||j%3===a%3)))}
 for(const kind of ['infer','autoregressive']) {
  const options={mode:i%3===0?'sample':'argmax',temperature:[0.5,1,2.25][i%3]}
  const draws=[0,0.5,0.9999999999999999,0.125];let cursor=0
  const selection={...options,random:()=>draws[cursor++]}
  const output=kind==='infer'?runtime.infer(observation,masks,selection):runtime.inferAutoregressive(observation,plan,t=>plan.abilityByTarget[t],(_t,a)=>plan.aimByAbility[a],selection)
  inferences.push({kind,observation:array(observation),masks:Object.fromEntries(Object.entries(masks).map(([k,v])=>[k,array(v)])),plan:{movement:array(plan.movement),target:array(plan.target),abilityByTarget:plan.abilityByTarget.map(array),aimByAbility:plan.aimByAbility.map(array)},options,draws,output:result(output)})
 }
}
const choices=[]
for(const count of [1,3,7])for(const mode of ['argmax','sample']) {
 const observation=observations[count%observations.length],descriptors=floats(count*138,2)
 const mask=Uint8Array.from({length:count},(_,i)=>Number(i===0||i%2===0))
 const options={mode,...(count===7?{temperature:0.75}:{})}
 const draw=count===1?0:0.9999999999999999
 const output=runtime.choose(observation,descriptors,mask,{...options,random:()=>draw})
 choices.push({observation:array(observation),descriptors:array(descriptors),mask:array(mask),options,draw,output:result(output)})
}
const headerLength=new DataView(encoded.buffer).getUint32(8,true)
const header=JSON.parse(new TextDecoder().decode(encoded.subarray(12,12+headerLength)))
const errors=[]
function changedHeader(name,edit){
 const next=structuredClone(header);edit(next)
 const bytes=new TextEncoder().encode(JSON.stringify(next));const nextEncoded=new Uint8Array(12+bytes.length+encoded.length-12-headerLength)
 nextEncoded.set(encoded.subarray(0,12));new DataView(nextEncoded.buffer).setUint32(8,bytes.length,true);nextEncoded.set(bytes,12);nextEncoded.set(encoded.subarray(12+headerLength),12+bytes.length)
 try{const decoded=checkpointKernel.decodeMlBotPolicyCheckpoint(nextEncoded);errors.push({name,header:next,accepted:true,encodedSha256:sha(checkpointKernel.encodeMlBotPolicyCheckpoint(decoded))})}
 catch(error){errors.push({name,header:next,error:error.message})}
}
for(const [field,value] of [['modelFormat','wrong'],['architecture','wrong'],['modelVersion',6],['observationVersion',6],['mainTrajectoryVersion',6],['choiceTrajectoryVersion',6],['choiceHiddenSize',64],['hiddenSizes',[256,512]],['choiceTemperature',0],['choicePolicyMode','other'],['seed',-1],['trainedUpdates',-1],['trainedEnvironmentSteps',0.5],['choiceCoverage',{'':1}],['choiceCoverage',{ether:-1}],['observationNames',[]],['optionDescriptorNames',[]],['primaryCurriculum',[]]]) changedHeader(`metadata ${field} ${JSON.stringify(value)}`,h=>h.metadata[field]=value)
changedHeader('wrong movement names',h=>h.metadata.actionHeads.movement.reverse())
changedHeader('duplicated tensor',h=>h.tensors.push(h.tensors[0]))
changedHeader('unknown tensor',h=>h.tensors[0].name='unknown')
changedHeader('wrong shape',h=>h.tensors[0].shape=[23])
changedHeader('wrong byte length',h=>h.tensors[0].byteLength--)
changedHeader('negative byte offset',h=>h.tensors[0].byteOffset=-1)
changedHeader('fractional byte offset',h=>h.tensors[0].byteOffset=0.5)
changedHeader('truncated tensor',h=>h.tensors[0].byteOffset=encoded.length)
changedHeader('missing tensor',h=>h.tensors.shift())
changedHeader('source accepts overlapping retained tensor slices',h=>h.tensors[1].byteOffset=0)
const malformed=[]
function bytesCase(name,edit){const next=edit(encoded.slice());try{checkpointKernel.decodeMlBotPolicyCheckpoint(next);throw new Error('expected rejection')}catch(error){malformed.push({name,error:error.message})}}
bytesCase('magic',b=>{b[0]=0;return b})
bytesCase('prefix',b=>b.subarray(0,11))
bytesCase('header truncated',b=>{new DataView(b.buffer).setUint32(8,b.length,true);return b})
bytesCase('header JSON',b=>{b[12]=0xff;return b})
bytesCase('tensor NaN',b=>{new DataView(b.buffer).setUint32(12+headerLength,0x7fc00000,true);return b})
bytesCase('trailing payload',b=>new Uint8Array([...b,0]))
writeFixture('ml-bot-inference-goldens.json','tools/website-oracle/ml-bot-inference-goldens.mjs','Website core-server/ml-bot-policy/{checkpoint,runtime,spec}.ts; actual frontend/server-assets/ml-bot-policy-v7-selected.sdml',
 {modelSha256:sha(encoded),sourceEncodedSha256:sha(checkpointKernel.encodeMlBotPolicyCheckpoint(checkpoint)),header,errors,malformed,inferences,choices})
console.log(JSON.stringify({bytes:encoded.length,inferences:inferences.length,choices:choices.length,headerCases:errors.length,malformed:malformed.length}))
