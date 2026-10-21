#!/usr/bin/env node
// Exact pinned-V8 primitives. Inputs are retained as IEEE words, never decimal anchors.
import { writeFixture } from './oracle.mjs'
const buffer = Buffer.alloc(8)
const bits = value => { buffer.writeDoubleBE(value); return buffer.readBigUInt64BE().toString(16).padStart(16,'0') }
const fromBits = word => { buffer.writeBigUInt64BE(word); return buffer.readDoubleBE() }
const inputs = [fromBits(0x40021c3f8b5a7e55n),0,-0,Infinity,-Infinity,NaN]
for (const word of [0x3e40000000000000n,0x3fd3333300000000n,0x3fe9000000000000n,
  0x3fe921fc00000000n,0x3ff921fb00000000n,0x4002d97c00000000n,0x413921fc00000000n]) {
  for(let delta=-3n;delta<=3n;delta++)for(const sign of [0n,0x8000000000000000n])inputs.push(fromBits((word+delta)|sign))
}
for(let multiple=1;multiple<=128;multiple++) {
  const word=BigInt(`0x${bits(multiple*Math.PI/2)}`)
  for(let delta=-2n;delta<=2n;delta++)for(const sign of [0n,0x8000000000000000n])inputs.push(fromBits((word+delta)|sign))
}
for(let exponent=-1074;exponent<=1023;exponent++) {
  for(const factor of [1,1.5])for(const sign of [1,-1])inputs.push(sign*factor*2**exponent)
}
for(let degree=-720;degree<=1440;degree++)inputs.push(degree*Math.PI/180)
let state=0x51ac05e1
function nextWord(){state^=state<<13;state^=state>>>17;state^=state<<5;return state>>>0}
for(let index=0;index<8192;index++)inputs.push(fromBits((BigInt(nextWord())<<32n)|BigInt(nextWord())))
const unique=new Map(inputs.map(value=>[bits(value),value]))
const result=value=>Number.isNaN(value)?'NaN':bits(value)
const cases=[...unique].map(([word,value])=>[word,result(Math.sin(value)),result(Math.cos(value))])
writeFixture('float-sincos-goldens.json','tools/website-oracle/float-sincos-goldens.mjs',
  `JavaScript Math.sin/Math.cos; Node ${process.versions.node}, V8 ${process.versions.v8}`,{cases})
console.log(JSON.stringify({cases:cases.length,witness:cases[0]}))
