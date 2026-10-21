// Exact JavaScript powers, including the actual Fire envelope and IEEE boundaries.
import { writeFixture } from './oracle.mjs'
const buffer = Buffer.alloc(8)
const bits = value => { buffer.writeDoubleBE(value); return buffer.readBigUInt64BE().toString(16).padStart(16, '0') }
const fromBits = word => { buffer.writeBigUInt64BE(word); return buffer.readDoubleBE() }
const inputs = [[0.95, 10]]
for (const x of [0, -0, 1, -1, 2, -2, 0.5, -0.5, Infinity, -Infinity, NaN, Number.MIN_VALUE, Number.MAX_VALUE]) {
  for (const y of [0, -0, 1, -1, 2, -2, 0.5, -0.5, 3, -3, Infinity, -Infinity, NaN, 2 ** 31, 2 ** 53, 2 ** 65]) inputs.push([x, y])
}
for (const x of [0.95, 1.02, 1.045, 1.1, 0.1, 0.5, 2]) {
  for (let tick = -32; tick <= 1200; tick++) inputs.push([x, tick / 4])
}
for (const word of [0x0000000000000001n, 0x0010000000000000n, 0x3fefffff00000000n, 0x3ff0000000000000n, 0x3ff3988e00000000n, 0x3ffbb67a00000000n, 0x4000000000000000n]) {
  for (let delta = -2n; delta <= 2n; delta++) {
    if (word + delta < 0n) continue
    for (const y of [-1075, -1074, -1022, -2, -1, -0.5, 0.5, 1, 2, 1023, 1024, 2 ** 31 + 1, 2 ** 64 + 4096]) inputs.push([fromBits(word + delta), y])
  }
}
let state = 0x51ac05e1
function nextWord() { state ^= state << 13; state ^= state >>> 17; state ^= state << 5; return state >>> 0 }
for (let i = 0; i < 8192; i++) {
  const x = fromBits((BigInt(nextWord()) << 32n) | BigInt(nextWord()))
  const y = fromBits((BigInt(nextWord()) << 32n) | BigInt(nextWord()))
  inputs.push([x, y])
}
const unique = new Map(inputs.map(([x, y]) => [`${bits(x)}:${bits(y)}`, [x, y]]))
const cases = [...unique.values()].map(([x, y]) => {
  const result = Math.pow(x, y)
  return [bits(x), bits(y), Number.isNaN(result) ? 'NaN' : bits(result)]
})
writeFixture('float-pow-goldens.json', 'tools/website-oracle/float-pow-goldens.mjs',
  `JavaScript Math.pow; Node ${process.versions.node}, V8 ${process.versions.v8}`, { cases })
console.log(JSON.stringify({ cases: cases.length, witness: cases[0] }))
