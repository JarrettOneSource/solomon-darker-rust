#!/usr/bin/env node
// Shared Staff/Golem area movement, including live collision recipients and final RNG draws.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const area = await loadKernel('core-kernels/native-knockback-area.ts')
const staff = await loadKernel('core-kernels/native-player-staff-action.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const bits = value => {
  if (typeof value === 'number') { const b = Buffer.alloc(8); b.writeDoubleBE(value); return `f64:${b.toString('hex')}` }
  if (Array.isArray(value)) return value.map(bits)
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, bits(item)]))
  return value
}
const cases = []
for (const budget of [0, 5, 50, 150]) for (const mode of ['snapshot', 'collision-chain']) {
  const positions = { first: { x: 30, y: -40 }, coincident: { x: 0, y: 0 }, second: { x: 50, y: -20 }, third: { x: -20, y: -60 } }
  const initialPositions = structuredClone(positions)
  const initial = { origin: { x: 0, y: 0 }, remainingDistance: budget, targetIds: ['missing', 'first', 'coincident'] }
  let state = initial
  let rng = random.createNativeRng(31)
  const frames = []
  for (let tick = 1; tick <= Math.max(1, budget / 10); tick += 1) {
    const moves = []
    const result = area.stepNativeKnockbackArea(state, rng, { position: id => positions[id] ?? null,
      move: mode === 'snapshot' ? undefined : (id, delta) => {
        moves.push({ id, delta })
        positions[id] = { x: Math.fround(positions[id].x + delta.x), y: Math.fround(positions[id].y + delta.y) }
        // This is the pure kernel's supplied world feedback, not a collision approximation.
        // The actual Boneyard adapter has its own solver-level replay.
        return id === 'first' ? ['second', 'second'] : id === 'second' ? ['third', 'first'] : []
      } })
    frames.push({ expected: bits({ ...result, moves, positions }) })
    state = { origin: initial.origin, remainingDistance: result.remainingDistance, targetIds: result.targetIds }
    rng = result.rng
  }
  cases.push({ budget, mode, initial, initialPositions, frames })
}
const staffCases = []
for (const budget of [0, 5, 50, 150]) {
  const initial = { ageTicks: 0, arcDegrees: 120, id: 1, kind: 'player-staff-knockback', origin: { x: 0, y: 0 },
    ownerId: 'owner', remainingDistance: budget, targetIds: ['missing', 'first', 'coincident'], worldKey: 'boneyard:area' }
  const positions = { first: { x: 30, y: -40 }, coincident: { x: 0, y: 0 } }
  let actor = initial
  let rng = random.createNativeRng(31)
  const frames = []
  while (actor !== null) {
    const result = staff.stepNativeStaffKnockback(actor, positions, rng)
    frames.push({ expected: bits(result) })
    actor = result.actor
    rng = result.rng
  }
  staffCases.push({ budget, initial, positions, frames })
}
writeFixture('current-knockback-area-goldens.json', 'tools/website-oracle/current-knockback-area-goldens.mjs',
  'current native-knockback-area.ts and native-player-staff-action.ts::stepNativeStaffKnockback',
  { sourceRevision, cases, staffCases })
