import { readFileSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const staff = await loadKernel('core-kernels/native-player-staff-action.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const stats = JSON.parse(readFileSync(resolve(fixturesDir, 'player-skill-runtime-goldens.json')))
const derived = stats.cases[0].derived
const seeds = [0, 1, 2, 3, 7, 42, 77, 123, 999, 2026, 2468, 0x12345678]
const actions = []
const byOutcome = new Map()
for (const seed of seeds) for (const lane of ['primary', 'secondary']) {
  for (const timing of [1, 1.75]) for (const chance of [0, 100]) {
    const input = { derived: { ...derived, staffEquipped: true,
      staffActionTimingFactor: timing, flailingChancePercent: chance },
      headingDegrees: -427.125 + seed % 17, id: 19, lane,
      origin: { x: 123.25, y: -54.5 }, ownerId: 'player', worldKey: 'boneyard:staff' }
    const birth = staff.createNativePlayerStaffAction(input, random.createNativeRng(seed))
    byOutcome.set(birth.action.outcome, birth.action)
    const steps = []
    let action = birth.action
    for (let tick = 1; action; tick++) {
      const ownerPosition = { x: input.origin.x + tick * 0.125, y: input.origin.y - tick * 0.375 }
      const result = staff.stepNativePlayerStaffAction(action, ownerPosition)
      steps.push({ ownerPosition, result, pose: staff.nativePlayerStaffActionPose(result.sample) })
      action = result.action
    }
    actions.push({ seed, lane, timing, chance, input, birth, steps })
  }
}
if (byOutcome.size !== 5) throw new Error('Staff source seed set missed a proc outcome')

const geometry = []
const origin = { x: 0.125, y: -0.25 }
const coordinates = [-105, -100, -70, -60, -50, -45, -40, -30, -0.1, 0, 0.1, 30, 40, 45, 50, 60, 70, 100, 105]
const targets = []
for (const x of coordinates) for (const y of coordinates) for (const collisionRadius of [0, 25]) {
  targets.push({ id: `target:${targets.length}`, position: { x: origin.x + x, y: origin.y + y }, collisionRadius })
}
const ids = values => values.map(value => value.id)
for (const heading of [-720, -50, -0.1, 0, 0.1, 49.999, 50, 50.001, 90, 179.999, 180, 270, 359.99, 360, 721]) {
  const player = { collisionRadius: 15, headingDegrees: heading, position: origin }
  const damage = []
  for (const [outcome, initial] of byOutcome) {
    const action = { ...initial, headingDegrees: heading, origin }
    damage.push({ action, admitted: ids(staff.nativeStaffDamageTargets(action, targets)),
      knockback: ids(staff.nativeStaffKnockbackTargets(action, targets)) })
  }
  geometry.push({ player, admission: staff.nativeStaffAdmissionTarget(player, targets)?.id ?? null,
    physical: ids(staff.nativeStaffPhysicalContactTargets(player, targets)), damage })
}

const registrationQueries = []
const orderedTargets = [
  { id: 'same-cell-first', position: { x: 15, y: -40 }, collisionRadius: 0 },
  { id: 'same-cell-second', position: { x: 15, y: -45 }, collisionRadius: 0 },
  { id: 'negative-cell', position: { x: -35, y: -50 }, collisionRadius: 25 },
  { id: 'positive-cell', position: { x: 70, y: -50 }, collisionRadius: 0 },
  { id: 'behind', position: { x: 0, y: 80 }, collisionRadius: 0 },
  { id: 'origin', position: { x: 0, y: 0 }, collisionRadius: 0 },
  { id: 'radius-boundary', position: { x: 100, y: 0 }, collisionRadius: 25 },
]
for (const outcome of ['knockback', 'critical-hit', 'whirl']) {
  const action = { ...byOutcome.get(outcome), origin: { x: 0, y: 0 }, headingDegrees: 0 }
  for (const registrationOrders of [null, [90, 10, 40, 30, 20, 60, 50], [7, 7, null, 0, 2, null, 1]]) {
    const queryTargets = orderedTargets.map((target, index) => registrationOrders?.[index] == null
      ? target : { ...target, registrationOrder: registrationOrders[index] })
    registrationQueries.push({ action, targets: queryTargets,
      targetIds: ids(staff.nativeStaffKnockbackTargets(action, queryTargets)) })
  }
}

const physical = []
const physicalTargets = [
  { id: 'same', position: { x: 123.25, y: -54.5 }, collisionRadius: 15, pike: true },
  { id: 'pike', position: { x: 126.25, y: -58.5 }, collisionRadius: 10, pike: true },
  { id: 'plain', position: { x: 93.75, y: -65.75 }, collisionRadius: 0, pike: false },
]
const normal = byOutcome.get('normal')
for (const seed of seeds) for (const element of ['air', 'earth', 'ether', 'fire', 'water']) {
  for (const chance of [0, 50, 200]) {
    physical.push({ seed, element, chance,
      result: staff.resolveNativeStaffPhysicalContacts(normal, physicalTargets, element, chance, random.createNativeRng(seed)) })
  }
}

const presentations = []
for (const action of byOutcome.values()) for (const count of [0, 1, 3]) for (const seed of [0, 77, 2026]) {
  const targetIds = ['enemy:1', 'enemy:2', 'scenery:3'].slice(0, count)
  const result = staff.createNativeStaffContactPresentation(100, action, targetIds,
    { x: 144.125, y: -80.25 }, 0x12abef, random.createNativeRng(seed), [0.95, 1.075], [1])
  const vfxJourneys = []
  const indexes = [...new Set([0, Math.floor(result.vfx.length / 2), result.vfx.length - 1])]
  for (const index of indexes) {
    let actor = result.vfx[index]
    if (!actor) continue
    const samples = [actor]
    while (actor) { actor = staff.stepNativePlayerStaffVfx(actor); samples.push(actor) }
    vfxJourneys.push({ index, samples })
  }
  const eventJourney = [result.event]
  let event = result.event
  while (event) { event = staff.stepNativeStaffContactEvent(event); eventJourney.push(event) }
  presentations.push({ action, seed, targetIds, result, vfxJourneys, eventJourney })
}

const knockbacks = []
const positions = { same: { ...normal.origin }, north: { x: 129.25, y: -85.5 }, diagonal: { x: -3.5, y: 9.25 } }
const targetIds = ['same', 'north', 'missing', 'diagonal']
for (const action of byOutcome.values()) for (const factor of [0, 0.25, 1, 1.375]) for (const seed of [0, 77]) {
  const birth = staff.createNativeStaffKnockback(300, action, targetIds, factor)
  let actor = birth
  let rng = random.createNativeRng(seed)
  const samples = []
  while (actor) {
    const result = staff.stepNativeStaffKnockback(actor, positions, rng)
    samples.push(result)
    actor = result.actor
    rng = result.rng
  }
  knockbacks.push({ action, factor, seed, birth, samples })
}

const contactKnockbacks = []
for (const targetExists of [false, true]) {
  const birth = staff.createNativeStaffContactKnockback(400, normal, 'pike', { x: 1.25, y: -5.5 })
  let actor = birth
  const samples = []
  while (actor) { const result = staff.stepNativeStaffContactKnockback(actor, targetExists); samples.push(result); actor = result.actor }
  contactKnockbacks.push({ targetExists, birth, samples })
}
const pikeBreakBirth = staff.createNativeStaffPikeBreakVfx(500, normal, physicalTargets[1], random.createNativeRng(77), -812.5)
const pikeBreak = [pikeBreakBirth]
let pike = pikeBreakBirth
while (pike) { pike = staff.stepNativeStaffPikeBreakVfx(pike); pikeBreak.push(pike) }
const damage = []
for (const amount of [0, 0.125, 17.3, 999]) for (const count of [1, 2, 3, 8]) for (const radial of [false, true]) {
  damage.push({ amount, count, radial, result: staff.nativeStaffContactDamagePerTarget(amount, count, radial) })
}

writeFixture('staff-action-goldens.json', 'tools/website-oracle/staff-action-goldens.mjs',
  'Website/frontend/src/game/core-kernels/native-player-staff-action.ts', {
    sourceRevision: execFileSync('git', ['-C', websiteDir, 'rev-parse', 'HEAD'], {encoding:'utf8'}).trim(),
    actions, geometryTargets: targets, geometry, registrationQueries, physicalAction: normal, physicalTargets, physical,
    presentations, positions, knockbackTargetIds: targetIds, knockbacks, contactKnockbacks, pikeBreak, damage,
  })
