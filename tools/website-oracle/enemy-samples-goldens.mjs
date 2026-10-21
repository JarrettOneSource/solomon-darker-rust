#!/usr/bin/env node
// The actual pinned snapshot interpolation, plus endpoint projection over retained real actors.
import { readFileSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import assert from 'node:assert/strict'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, readFixture, websiteDir, writeFixture } from './oracle.mjs'
const sourceRevision=execFileSync('git',['rev-parse','HEAD'],{cwd:websiteDir,encoding:'utf8'}).trim()
const appendMovementViews = process.env.SOLOMON_APPEND_ENEMY_MOVEMENT_VIEWS === '1'
const retained = appendMovementViews ? readFixture(resolve(
  process.env.SOLOMON_RETAINED_ENEMY_FIXTURES??fixturesDir, 'enemy-samples-goldens.json',
)) : null
if (retained) assert.equal(retained.sourceRevision, sourceRevision)

const samples = await loadKernel('client/boneyard-enemy-samples.ts')
const interpolator = new samples.BoneyardEnemySampleInterpolator()
const projection = await loadKernel('host/project-boneyard-enemies.ts')
const puppet = await loadKernel('core-kernels/native-puppet-hit.ts')
const attack = await loadKernel('renderer/native-enemy-attack-effect.ts')
const fields = [
  'state', 'action', 'deathEpoch', 'actionProgress', 'alpha', 'bodyGaitPhase', 'bodyPose',
  'coffinPose', 'coffinRotationRadians', 'coffinScaleX', 'coffinState', 'deathTick',
  'demonFrontExtremityOffset', 'demonFrontRotationRadians', 'demonRearExtremityOffset',
  'demonRearRotationRadians', 'demonShadowOffset', 'effects', 'gaitPose', 'hitFlash',
  'impBodyRotationRadians', 'impEffectAlpha', 'impEffectFrame', 'pikeTargetOffset',
  'shadowLateralOffset', 'spider', 'stridePhaseDeg', 'verticalOffset',
  'zombieAngularOffsetDeg', 'zombieFrontArmPose', 'zombieRearArmPose',
  'zombieBodyRotationRadians', 'zombieArmSocketRotationRadians',
  'zombieFrontArmRotationRadians', 'zombieRearArmRotationRadians', 'zombieHeadRotationRadians',
]
function animation(value) {
  const result = Object.fromEntries(fields.map(field => [field, value[field]]))
  result.effects = value.effects.map(effect => ({
    id: effect.id, role: effect.role, alpha: effect.alpha, offset: effect.offset,
    rotationRadians: effect.rotationRadians, scale: effect.scale,
  }))
  return result
}
function empty(enemies = []) {
  return { enemies, deathEffects: [], enemyEvents: [], enemyWorldFeedback: {},
    enemyProjectileEffects: [], enemyProjectiles: [], mageLightningPulses: [], maggots: [],
    spiderSilks: [], spiderRemains: [], silkFragments: [], webbedPlayers: {} }
}
const construction = JSON.parse(readFileSync(resolve(fixturesDir, 'enemy-construction-goldens.json')))
const ai = readFixture('enemy-ai-goldens.json.zlib')
const views = JSON.parse(readFileSync(resolve(fixturesDir, 'enemy-presentation-goldens.json')))
const realActors = new Map()
for (const cohort of construction.cases) {
  for (const actor of cohort.actors) realActors.set(`construction:${cohort.name}:10:${actor.id}`, actor)
}
for (const cohort of ai.cases) {
  for (const sample of cohort.samples) {
    for (const actor of sample.actors) realActors.set(`ai:${cohort.name}:${sample.tick}:${actor.id}`, actor)
  }
}
const endpoints = retained?.endpoints??[]
const families = new Map()
for (const record of views.actors) {
  if (retained && ai.movementViewInputs[record.name] !== 'nativeMovementView') continue
  const key = `${record.group}:${record.name}:${record.tick}:${record.id}`
  if (retained) assert.ok(!endpoints.some(row => row.key === key && row.hitOverride === record.hitOverride))
  const actor = structuredClone(realActors.get(key))
  if (!actor) throw new Error(`missing retained actor ${key}`)
  if (record.hitOverride) actor.hitFeedback = puppet.receiveNativePuppetHit(record.tick)
  const players = {}
  if (actor.targetPlayerId !== null && record.context.spiderTarget !== null) {
    players[actor.targetPlayerId] = record.context.spiderTarget
  }
  if (actor.brain.family === 'skeleton' && actor.brain.pike !== null) {
    players[actor.brain.pike.playerId] = {position: record.context.pikeTargetPosition}
  }
  const visual = projection.projectBoneyardEnemies({actors: [actor]}, record.tick, {
    players, lightAt: () => record.context.spiderExposure,
  })[0]
  endpoints.push({key, hitOverride: record.hitOverride, expected: animation(visual.animation)})
  if (!families.has(visual.enemyToken)) families.set(visual.enemyToken, visual)
}
const cases = []
function record(name, first, second) {
  for (const blend of [0, 0.125, 0.5, 0.999, 1]) {
    const expected = interpolator.interpolate(empty([first]), empty([second]), blend, 12.5)
    cases.push({name, token: first.enemyToken, blend,
      first: animation(first.animation), second: animation(second.animation),
      expected: animation(expected.enemies[0].animation)})
  }
}
for (const [token, visual] of retained?[]:families) {
  const first = structuredClone(visual)
  const second = structuredClone(visual)
  Object.assign(first.animation, { state: 'idle', action: null, deathEpoch: 0,
    bodyGaitPhase: 3.8, gaitPose: 7.8, shadowLateralOffset: 3.8,
    impEffectFrame: 9, coffinRotationRadians: 6.1, stridePhaseDeg: 350,
    verticalOffset: -20, zombieFrontArmPose: 0, zombieRearArmPose: 2,
    pikeTargetOffset: {x: 5, y: 10},
    spider: {bodyHeadingDeg: 350, outlineAlpha: 0.25, outlineTint: 0x112233},
    effects: [{id: 7, role: 'magic-shield', alpha: 0.2, offset: {x: 1, y: -30}, rotationRadians: 350 * Math.PI / 180, scale: 1.5}],
  })
  Object.assign(second.animation, { state: 'idle', action: null, deathEpoch: 0,
    bodyGaitPhase: 0.2, gaitPose: 0.2, shadowLateralOffset: 0.2,
    impEffectFrame: 1, coffinRotationRadians: 0.1, stridePhaseDeg: 10,
    verticalOffset: -10, zombieFrontArmPose: 2, zombieRearArmPose: 0,
    pikeTargetOffset: {x: 15, y: 20},
    spider: {bodyHeadingDeg: 10, outlineAlpha: 0.75, outlineTint: 0xaabbcc},
    effects: [{id: 7, role: 'magic-shield', alpha: 0.8, offset: {x: 3, y: -40}, rotationRadians: 10 * Math.PI / 180, scale: 2.5},
      {id: 8, role: 'magic-shield', alpha: 1, offset: {x: 0, y: 0}, rotationRadians: 0, scale: 1}],
  })
  record(`${token}:cycles-and-articulation`, first, second)
  for (const [boundary, change] of [
    ['state', {state: 'death'}], ['action', {action: 'skeleton-pike'}],
    ['death-epoch', {deathEpoch: 1}],
    ['nullable', {pikeTargetOffset: null, spider: null, impEffectFrame: -1}],
    ['effect-role', {effects: [{...second.animation.effects[0], role: 'changed-role'}]}],
  ]) {
    const changed = structuredClone(second)
    Object.assign(changed.animation, change)
    record(`${token}:${boundary}`, first, changed)
  }
}
const members = []
const point = (x, y) => ({x, y})
const registration = id => ({managerLane: 'transient', registrationOrdinal: id})
const older = empty()
const newer = empty()
older.enemyEvents = [{id: 1, sourcePosition: point(1, 2)}]
newer.enemyEvents = [{id: 2, sourcePosition: point(3, 4)}]
older.enemyWorldFeedback = {magnitude: 1}
newer.enemyWorldFeedback = {magnitude: 3}
older.deathEffects = [1, 2].map(id => ({id, ageTicks: 3, alpha: 0.9, height: -10, position: point(10, 20), rotationRadians: 350 * Math.PI / 180, scale: 1, scaleY: 2, entry: 1, painterRegistration: null}))
newer.deathEffects = [2, 3].map(id => ({id, ageTicks: 8, alpha: 0.4, height: 10, position: point(20, 40), rotationRadians: 10 * Math.PI / 180, scale: 2, scaleY: 3, entry: 2, painterRegistration: null}))
for (const [index, kind] of ['arrow', 'guided-missile', 'demon-bomb', 'firebolt', 'poison-pool'].entries()) {
  older.enemyProjectiles.push({id: index + 1, kind, lightRegistration: null, painterRegistration: registration(index + 1), ageTicks: 3, headingDeg: 350, position: point(10, 20), speed: 3, verticalOffset: -20, visualPhaseDeg: 710, visualScale: 0.4})
  newer.enemyProjectiles.push({id: index + 1, kind, lightRegistration: null, painterRegistration: registration(index + 1), ageTicks: 8, headingDeg: 10, position: point(20, 40), speed: 5, verticalOffset: -10, visualPhaseDeg: 10, visualScale: 0.8})
}
older.enemyProjectileEffects = [{id: 1, lightRegistration: null, painterRegistration: registration(1), ageTicks: 3, alpha: 1, position: point(10, 20), rotationRadians: 350 * Math.PI / 180, scale: 1}]
newer.enemyProjectileEffects = [{id: 1, lightRegistration: null, painterRegistration: registration(1), ageTicks: 8, alpha: 0.5, position: point(20, 40), rotationRadians: 10 * Math.PI / 180, scale: 2}]
older.maggots = [{id: 1, lightRegistration: registration(1), alpha: 1, currentHealth: 10, deathTick: 3, emergencePhase: 4.8, emergenceTick: 3, headingDeg: 350, hitFlash: 1, pose: 1.8, position: point(10, 20), verticalOffset: -20, visualScale: 1, state: 'emerging'}]
newer.maggots = [{id: 1, lightRegistration: registration(1), alpha: 0.5, currentHealth: 5, deathTick: 8, emergencePhase: 0.2, emergenceTick: 8, headingDeg: 10, hitFlash: 0, pose: 0.2, position: point(20, 40), verticalOffset: -10, visualScale: 2, state: 'death'}]
older.spiderSilks = [2, 1].map(id => ({id, state: {ageTicks: 3, phase: 1, height: 10, alpha: 1, waveScale: 0.5, position: point(10, 20), drift: point(1, 2), ended: false}}))
newer.spiderSilks = [1, 3].map(id => ({id, state: {ageTicks: 8, phase: 4, height: 5, alpha: 0.5, waveScale: 1, position: point(20, 40), drift: point(3, 4), ended: true}}))
older.silkFragments = [{id: 1, state: {start: point(1, 2), middle: point(3, 4), end: point(5, 6), opacity: 1}}]
newer.silkFragments = [{id: 1, state: {start: point(2, 3), middle: point(4, 5), end: point(6, 7), opacity: 0.5}}]
older.spiderRemains = [{id: 1, state: {life: 20, position: point(10, 20), frame: 0, decal: {scale: 1, alpha: 1, position: point(4, 5)}}}]
newer.spiderRemains = [{id: 1, state: {life: 15, position: point(20, 40), frame: 1, decal: {scale: 2, alpha: 0.5, position: point(6, 7)}}}]
older.webbedPlayers = {hero: {severity: 1}}
newer.webbedPlayers = {hero: {severity: 2}}
function pulse(id, tick) {
  return {id, tick, source: point(0, 0), midpoint: point(1, 1), endpoint: point(2, 2),
    contact: {kind: 'world', position: point(2, 2)}, lightRegistration: {managerLane: 'transient', registrationOrdinal: id}, painterRegistrations: []}
}
older.mageLightningPulses = [pulse(1, 9), pulse(2, 10), pulse(4, 12)]
newer.mageLightningPulses = [pulse(2, 10), pulse(3, 11), pulse(5, 15)]
for (const blend of [0, 0.125, 0.5, 0.999, 1]) {
  for (const tick of [10, 12.5, 15]) {
    members.push({blend, tick, older, newer,
      expected: interpolator.interpolate(older, newer, blend, tick)})
  }
}
const attacks = []
for (const eventId of [1, 317, 0xffffffff]) {
  for (const ageTicks of [0, 0.125, 0.5, 1.5, 12.9, 13, 15.9, 16, 21.9, 22]) {
    for (const kind of ['imp-landing', 'imp-contact', 'demon-bomb-muzzle']) {
      const expected = kind === 'imp-landing'
        ? attack.nativeImpLandingFlareSample(eventId, ageTicks)
        : attack.nativeEnemyRawFireBurstSample(kind, eventId, ageTicks)
      attacks.push({kind, eventId, ageTicks, expected})
    }
  }
}
writeFixture('enemy-samples-goldens.json', 'tools/website-oracle/enemy-samples-goldens.mjs',
  'Current Website client/boneyard-enemy-samples.ts and host/project-boneyard-enemies.ts',
  {sourceRevision, endpoints, cases:retained?.cases??cases, members:retained?.members??members, attacks:retained?.attacks??attacks})
console.log(`${endpoints.length} real endpoint projections; ${retained?.cases.length??cases.length} retained animation brackets; ${retained?.members.length??members.length} complete member brackets`)

console.log(`${attacks.length} fractional auxiliary samples`)
