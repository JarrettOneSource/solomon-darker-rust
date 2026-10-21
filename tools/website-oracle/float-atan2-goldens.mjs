#!/usr/bin/env node
// Exact JavaScript primitive values, including every fdlibm reduction/quadrant boundary.
import { writeFixture } from './oracle.mjs'

const buffer = Buffer.alloc(8)
const bits = value => {
  buffer.writeDoubleBE(value)
  return buffer.readBigUInt64BE().toString(16).padStart(16, '0')
}
const value = word => {
  buffer.writeBigUInt64BE(BigInt(`0x${word}`))
  return buffer.readDoubleBE()
}
const inputs = [0, Number.MIN_VALUE, 2 ** -1022, 1e-300, 2 ** -61, 2 ** -60,
  2 ** -28, 2 ** -27, 0.4375, 0.6875, 1, 1.1875, 2.4375, 2 ** 60, 2 ** 66, 1e300, Infinity, NaN]
for (const boundary of [0.4375, 0.6875, 1, 1.1875, 2.4375, 2 ** 66]) {
  const word = BigInt(`0x${bits(boundary)}`)
  inputs.push(value((word - 1n).toString(16)), value((word + 1n).toString(16)))
}
const signed = inputs.flatMap(input => [input, -input])
const pairs = signed.flatMap(y => signed.map(x => [y, x]))
// The real map-nine witness is not replaced with a rounded or nearby argument.
pairs.push([-4.039242219063453, -0.028232210609985486])
let state = 0x5eed1234
const nextWord = () => {
  state ^= state << 13
  state ^= state >>> 17
  state ^= state << 5
  return state >>> 0
}
const nextDouble = () => value(((BigInt(nextWord()) << 32n) | BigInt(nextWord())).toString(16))
for (let index = 0; index < 4096; index++) pairs.push([nextDouble(), nextDouble()])
const cases = pairs.map(([y, x]) => {
  const result = Math.atan2(y, x)
  return [bits(y), bits(x), Number.isNaN(result) ? 'NaN' : bits(result)]
})
writeFixture('float-atan2-goldens.json', 'tools/website-oracle/float-atan2-goldens.mjs',
  `JavaScript Math.atan2; Node ${process.versions.node}, V8 ${process.versions.v8}`, { cases })
