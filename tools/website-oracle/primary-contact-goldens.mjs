#!/usr/bin/env node
// Contact transactions use real enemy constructors. Existing family fixtures own
// constructor/death art; these samples protect spell ordering, draws and ownership.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'
import { weldActorRow } from './weld-state-rows.mjs'

const stores = await loadKernel('core-server/boneyard-enemy-store.ts')
const construction = await loadKernel('core-server/enemies/construction.ts')
const workModule = await loadKernel('core-server/enemies/work.ts')
const combat = await loadKernel('core-server/boneyard-spell-combat.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const managers = await loadKernel('core-kernels/native-world-manager-order.ts')
const schema = await loadKernel('core-kernels/boneyard-wave-schema.ts')
const progression = await loadKernel('core-kernels/player-progression.ts')
const primary = await loadKernel('core-kernels/native-primary-skill-profile.ts')
const enemyProjectiles = await loadKernel('core-server/enemies/projectile-emission.ts')
const silk = await loadKernel('core-kernels/native-silk.ts')

const airWaterHost = await loadKernel('core-server/air-water-player-visual-system.ts')
const weld = await loadKernel('core-kernels/native-weld-primary-runtime.ts')
const weldHail = await loadKernel('core-kernels/native-weld-hail-contact.ts')
const weldSteam = await loadKernel('core-kernels/native-weld-steam.ts')
const weldedJourneys = JSON.parse(readFileSync(resolve(fixturesDir, 'weld-step-goldens.json'), 'utf8'))

const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const frostOnly = process.env.SOLOMON_CONTACT_SET === 'frost'
const worldKey = 'boneyard:contact-oracle'
const origin = { x: 150, y: 250 }
const registration = (managerLane, registrationOrdinal) => ({ managerLane, registrationOrdinal })
const point = value => value === null || value === undefined ? null : [value.x, value.y]
const reg = value => value ? [value.managerLane, value.registrationOrdinal] : null

function projectile(kind, extra = {}) {
  const common = {
    ageTicks: 1, charge: 1, damage: kind === 'earth' ? 10 : 4,
    direction: { x: 1, y: 0 }, flightTicks: 1, id: 4,
    lightRegistration: registration('actor', 4), ownerId: 'hero', phase: 'flight',
    position: { ...origin }, velocity: { x: 3, y: 0 }, worldKey,
  }
  if (kind === 'ether') {
    return { ...common, kind, damageRetention: 0.75, headingDegrees: 90,
      piercesRemaining: 0, reacquiresTarget: false, speed: 3, targetId: null,
      turnInput: 2, turnAccumulator: Math.fround(1.1), underpowered: false,
      visualScale: 1, ...extra }
  }
  if (kind === 'fire') {
    return { ...common, kind, burnDamage: 0, emberDamage: 0, emberFragments: 0,
      explodeDamage: 0, explodeRadius: 0, privateSeed: 7,
      spentEmber: { kind: 'none' }, underpowered: false, ...extra }
  }
  return { ...common, kind, assemblyCharge: 1, hitTargetIds: [], maximumCharge: 1,
    orientation: [1, 0, 0, 0, 1, 0, 0, 0, 1], remainingDamage: 10,
    shellCharge: 1, toughness: 1, ...extra }
}

function profile(kind, extra = {}) {
  const book = progression.createPlayerSkillBook({ element: kind, discipline: 'arcane', displayName: 'Contact oracle' })
  return { ...primary.nativePrimarySkillProfile(book, progression.playerStatBook(), { damage: 1, manaCost: 1 }), ...extra }
}

function channel(kind, extra = {}) {
  return { damage: 0.25, direction: { x: 1, y: 0 }, endpoint: { x: 355, y: 250 },
    id: 11, kind, manaCost: kind === 'air' ? 0.12 : 0.125, origin: { ...origin },
    ownerId: 'hero', primarySkill: profile(kind), queryOrigin: { ...origin },
    terrainContact: false, underpowered: false, worldKey, ...extra }
}

function transient(kind, extra = {}) {
  const common = { ageTicks: 0, id: 11, kind, ownerId: 'hero', worldKey,
    lightRegistration: null, painterRegistrations: [] }
  if (kind === 'air') {
    return { ...common, birthTick: 10, enhancedEffects: true, direction: { x: 1, y: 0 },
      endpoint: { x: 355, y: 250 }, hurricaneCharge: 0, midpoint: { x: 252.5, y: 250 },
      origin: { ...origin }, targetId: 'enemy:1', underpowered: false, variant: 0,
      lightRegistration: registration('transient', 11),
      painterRegistrations: [registration('actor', 11), registration('actor', 12), registration('actor', 13)], ...extra }
  }
  if (kind === 'water') {
    return { ...common, direction: { x: 1, y: 0 }, origin: { ...origin },
      obstructionDistance: null, obstructionPoint: null, speed: 4, underpowered: false, variant: 0,
      painterRegistrations: [registration('transient', 11)], ...extra }
  }
  if (kind === 'ether-blast') {
    return { ...common, birthTick: 10, charges: 2, origin: { ...origin },
      presentationRng: random.createNativeRng(9),
      lightRegistration: registration('transient', 11),
      painterRegistrations: [registration('transient', 11)], ...extra }
  }
  if (kind === 'fire-ember') {
    return { ...common, burnDamage: 0.125, contactCadence: 3, contactDue: true,
      damage: 1.5, height: 0, horizontalVelocity: { x: 1, y: 0 }, life: 2, phase: 0,
      position: { ...origin }, spentEmber: { kind: 'none' }, verticalVelocity: 0,
      painterRegistrations: [registration('transient', 11)], ...extra }
  }
  return { ...common, burnDamage: 0.125, damage: 1.5, footprintDimension: 60,
    origin: { ...origin }, presentation: 'fire', soundPitch: 1, visualScale: 1,
    lightRegistration: registration('actor', 11),
    painterRegistrations: [registration('actor', 11)], ...extra }
}

const enemy = (x, y = 250, extra = {}) => ({ token: 'SKELETON', position: { x, y }, ...extra })
const scenery = { active: true, actorFlags: 4, attachment: { x: 0, y: 0 }, bodyRadius: 15,
  cellBindingOrder: 0, id: 'scenery:1', kind: 'scenery', nativePriority: 0,
  pendingRemove: false, position: { ...origin }, queryLane: 'grid', registrationOrder: 0 }
const cases = [
  { name: 'ether-post-move-first-cell', enemies: [enemy(168), enemy(151)], projectiles: [projectile('ether')] },
  { name: 'ether-locked-age199-ignores-scenery', enemies: [enemy(400)], scenery: [scenery],
    projectiles: [projectile('ether', { ageTicks: 199, targetId: 'enemy:1' })] },
  { name: 'ether-age200-scenery', enemies: [enemy(400)], scenery: [scenery],
    projectiles: [projectile('ether', { ageTicks: 200, targetId: 'enemy:1' })] },
  { name: 'ether-pierce-retarget', enemies: [enemy(155), enemy(200), enemy(210)],
    projectiles: [projectile('ether', { piercesRemaining: 2, reacquiresTarget: true })] },
  { name: 'earth-pool-depletion', enemies: [enemy(170, 250, { health: 5 }), enemy(190, 250, { health: 5 })],
    projectiles: [projectile('earth', { remainingDamage: 6 })] },
  { name: 'earth-pool-depletion-coarse', enhancedEffects: false,
    enemies: [enemy(170, 250, { health: 5 }), enemy(190, 250, { health: 5 })],
    projectiles: [projectile('earth', { remainingDamage: 6 })] },
  { name: 'earth-toughness-and-prior-hit', enemies: [enemy(170), enemy(190)],
    projectiles: [projectile('earth', { toughness: 5, hitTargetIds: ['enemy:1'] })] },
  { name: 'ether-blast-precedes-projectile', enemies: [enemy(155), enemy(170)],
    projectiles: [projectile('ether')], transients: [transient('ether-blast')] },
  { name: 'fire-shield-no-overflow', enemies: [enemy(155, 250, { shield: 3 }), enemy(170)], multiplier: 2,
    projectiles: [projectile('fire', { burnDamage: 0.1 })] },
  { name: 'fire-detonation-native-draws', enemies: [enemy(155), enemy(172)], multiplier: 2,
    projectiles: [projectile('fire', { burnDamage: 0.1, emberDamage: 0.75, emberFragments: 3, explodeDamage: 1, explodeRadius: 70 })] },
  { name: 'fire-delayed-ember-consumed-once', enemies: [enemy(155)], transients: [transient('fire-ember')],
    contacts: [{ amount: 1.5, burnDamage: 0.125, kind: 'fire-ember', ownerId: 'hero', position: origin, radius: 7, spellId: 11, worldKey }] },
  { name: 'fire-explosion-square-then-actors', enemies: [enemy(178, 278), enemy(180, 250), enemy(155)],
    transients: [transient('fire-explosion')], multiplier: 2,
    contacts: [{ amount: 0.125, burnDamage: 0.1, kind: 'fire-patch', ownerId: 'hero', position: origin, radius: 50, spellId: 12, worldKey },
      { amount: 0.5, kind: 'fire-good-imp', ownerId: 'hero', spellId: 13, targetId: 'enemy:2', worldKey }] },
  { name: 'air-chain-stun', enemies: [enemy(200), enemy(260), enemy(310)], transients: [transient('air')],
    emissions: [channel('air', { primarySkill: profile('air', { arcCount: 2, stunMovementFactor: 0.5 }) })] },
  { name: 'air-disintegrate-high-tail', tick: 40, enemies: [enemy(200, 250, { health: 0.5 })], transients: [transient('air')],
    emissions: [channel('air', { primarySkill: profile('air', { disintegrateChance: 100 }) })] },
  { name: 'air-underpowered-no-chain-stun', enemies: [enemy(200), enemy(260)], transients: [transient('air')],
    emissions: [channel('air', { underpowered: true, primarySkill: profile('air', { arcCount: 2, stunMovementFactor: 0.5, disintegrateChance: 100 }) })] },
  { name: 'water-cone-los-aura', enemies: [enemy(200), enemy(350), enemy(355), enemy(250, 285), enemy(120)], blockedX: 350,
    emissions: [channel('water', { primarySkill: profile('water', { auraRadiusScale: 100 / 120, auraMovementFactor: 0.7 }) })] },
  { name: 'water-hail-push-and-block', enemies: [enemy(200), enemy(225), enemy(280, 270)], movementClampX: 281,
    emissions: [channel('water', { primarySkill: profile('water', { pushbackFactor: 1, hailThreshold: 3000, hailDamageMinimum: 0.25, hailDamageMaximum: 1.5 }) })] },
  { name: 'water-underpowered', enemies: [enemy(200), enemy(280, 290)],
    emissions: [channel('water', { underpowered: true, primarySkill: profile('water', { pushbackFactor: 1, hailThreshold: 3000, auraRadiusScale: 100 / 120 }) })] },
  { name: 'water-silk-and-arrow-force', enemies: [enemy(500)], forces: true,
    emissions: [channel('water', { primarySkill: profile('water', { pushbackFactor: 1 }) })] },
  { name: 'water-underpowered-excludes-forces', enemies: [enemy(500)], forces: true,
    emissions: [channel('water', { underpowered: true, primarySkill: profile('water', { pushbackFactor: 1 }) })] },
]


// Take actual actors from the complete cast journeys, then exercise contact
// boundaries against real enemy constructors. Family fixtures own all birth math.
const copy = value => JSON.parse(JSON.stringify(value))
const sourceScreen = buildId => weldedJourneys.screens.find(screen => screen.buildId === buildId)
const contactActor = actor => ({ ...copy(actor), worldKey })
const weldProfile = buildId => weldedJourneys.cases.find(sample => sample.profile.buildId === buildId).profile
for (const buildId of [1000, 1001, 1002, 1009]) {
  const actor = contactActor(sourceScreen(buildId).spells.projectiles[0])
  actor.position = { ...origin }
  actor.targetId = 'enemy:1'
  cases.push({ name: `weld-${buildId}-accepted-contact`, enemies: [enemy(155), enemy(172)],
    projectiles: [actor], multiplier: 1.5 })
  if (buildId !== 1009) cases.push({ name: `weld-${buildId}-scenery-contact`, enemies: [enemy(400)],
    scenery: [scenery], projectiles: [{ ...copy(actor), targetId: null }] })
}
for (const buildId of [1003, 1004, 1005]) {
  const skill = weldProfile(buildId)
  const emission = channel('water', { kind: 'weld', primarySkill: skill, damage: 0.25 })
  const transients = buildId === 1003 ? [weld.createNativeWeldChannelActor({
    buildId, direction: { x: 1, y: 0 }, endpoint: { x: 355, y: 250 },
    id: 11, midpoint: { x: 252.5, y: 250 }, origin, ownerId: 'hero',
    targetId: 'enemy:1', tick: 10, underpowered: false, vector: skill.vector.values, worldKey,
  })] : []
  cases.push({ name: `weld-${buildId}-channel-status-chain-and-push`, tick: 40,
    enemies: [enemy(200, 250, { blizzardPushAccumulator: 0.8, blizzardPushLastTick: 39 }), enemy(260), enemy(310)],
    transients, emissions: [emission], scenery: [scenery], forces: buildId !== 1003,
    movementClampX: 265 })
  cases.push({ name: `weld-${buildId}-underpowered-channel`, enemies: [enemy(200), enemy(260)],
    transients: copy(transients), emissions: [{ ...emission, underpowered: true }] })
}
for (const buildId of [1006, 1008]) {
  const held = contactActor(sourceScreen(buildId).spells.transients.find(actor => actor.kind === 'weld-persistent'))
  const released = weld.releaseNativeWeldPersistentActor({ actor: held, firstChildId: 50,
    rng: random.createNativeRng(71), tick: 100 }).actors[0]
  let actor = released
  if (buildId === 1006) actor = { ...actor, origin: { ...origin }, remainingDamage: 6 }
  else actor = weld.stepNativeWeldWorldActor(actor, random.createNativeRng(71)).actor
  let position = actor.origin
  if (buildId === 1008) {
    const step = weld.nativeWeldHailstoneFlightContactSubsteps(actor)[0]
    const offset = Object.values(step.releaseOffsets).sort((a,b) => Math.hypot(a.x,a.y) - Math.hypot(b.x,b.y))[0]
    position = { x: step.origin.x + offset.x, y: step.origin.y + offset.y }
  }
  cases.push({ name: `weld-${buildId}-released-damage-pools`, tick: 110,
    enemies: [enemy(position.x + 1, position.y, { health: 5 }), enemy(position.x + 12, position.y, { health: 5 })],
    transients: [actor] })
  cases.push({ name: `weld-${buildId}-damage-pool-exhaustion`, tick: 110,
    enemies: [enemy(position.x + 1, position.y, { health: 1000 })],
    transients: [{ ...copy(actor), ...(buildId === 1006 ? { remainingDamage: 0.5 } : {}) }] })
  if (buildId === 1008) {
    const knockback = weldHail.createNativeWeldHailKnockback({ actor, id: 30, targetId: 'enemy:1', tick: 110 })
    cases.push({ name: 'weld-hail-existing-knockback-moves-target', tick: 111,
      enemies: [enemy(position.x + 1, position.y, {})], transients: [knockback] })
  }
}
let meteor = contactActor(sourceScreen(1007).spells.transients.find(actor => actor.kind === 'weld-meteor'))
let meteorRng = random.createNativeRng(75)
while (!meteor.impactDue) { const step = weld.stepNativeWeldWorldActor(meteor, meteorRng); meteor = step.actor; meteorRng = step.rng }
cases.push({ name: 'weld-1007-native-impact-then-fire', tick: 130,
  enemies: [enemy(meteor.position.x, meteor.position.y)], transients: [meteor] })
do { const step = weld.stepNativeWeldWorldActor(meteor, meteorRng); meteor = step.actor; meteorRng = step.rng } while (!meteor.pulseDue)
cases.push({ name: 'weld-1007-pulse-after-impact', tick: 140,
  enemies: [enemy(meteor.position.x, meteor.position.y)], transients: [meteor] })
let steam = contactActor(sourceScreen(1005).spells.transients.find(actor => actor.kind === 'weld-steam' && actor.contactEnabled && actor.life > 1))
while (!steam.contactDue) steam = weldSteam.stepNativeWeldSteamActor(steam)
cases.push({ name: 'weld-steam-owned-steamed-effect', tick: 60,
  enemies: [enemy(steam.position.x, steam.position.y + 15)], transients: [steam] })
// Current Steamed submits a terminal payload through the secondary receiver.
// Its real admission/ordering cases live in current-secondary-special-death-goldens.

function hurricanes(charges) {
  return airWaterHost.synchronizeAirWaterPlayerVisualActors({nextId:11,projectiles:[],transients:[]},
    charges.map((charge,index)=>({hurricaneCharge:charge,hurricaneContactCharge:charge,
      hurricaneDamageMinimum:2,hurricaneDamageMaximum:4,ownerId:index===0?'hero':'other',
      position:{x:150+index*50,y:250+index*50},worldKey})),10,random.createNativeRng(7)).spells.transients
}
for (const charge of [0.25,1]) cases.push({name:`hurricane-charge-${charge}-ordered-damage`,tick:11,
  enemies:[enemy(200,250,{hurricaneCooldown:0}),enemy(300)],transients:hurricanes([charge])})
cases.push({name:'hurricane-existing-cooldown-still-orbits',tick:11,
  enemies:[enemy(200,250,{hurricaneCooldown:100})],transients:hurricanes([1])})
cases.push({name:'hurricane-combined-owner-forces',tick:11,
  enemies:[enemy(200,250,{hurricaneCooldown:0})],transients:hurricanes([0.25,1])})
cases.push({name:'hurricane-native-serial-movement-gate',tick:12,
  enemies:[enemy(200,250,{hurricaneCooldown:0})],transients:hurricanes([1])})

function spellRow(spell) {
  if (spell.kind === 'weld') return weldActorRow(spell)
  return { id: spell.id, kind: spell.kind, charge: spell.charge, damage: spell.damage,
    position: point(spell.position), velocity: point(spell.velocity),
    detail: spell.kind === 'earth' ? [spell.remainingDamage, spell.hitTargetIds, spell.shellCharge]
      : spell.kind === 'ether' ? [spell.piercesRemaining, spell.targetId, spell.headingDegrees, spell.direction.x, spell.direction.y]
      : [] }
}

function effectRow(effect) {
  if (effect.kind.startsWith('weld-')) return weldActorRow(effect)
  const detail = {}
  for (const key of ['birthTick', 'charge', 'charges', 'damage', 'burnDamage', 'footprintDimension', 'visualScale', 'lifetimeTicks', 'variant', 'targetId', 'headingDegrees', 'contactCharge', 'damageMinimum', 'damageMaximum', 'enhancedEffects', 'phaseDegrees']) {
    if (key in effect) detail[key] = effect[key]
  }
  for (const key of ['origin', 'position', 'direction', 'endpoint', 'midpoint']) {
    if (key in effect) detail[key] = point(effect[key])
  }
  return { id: effect.id, kind: effect.kind, age: effect.ageTicks,
    light: reg(effect.lightRegistration), registrations: (effect.painterRegistrations ?? []).map(reg), detail }
}

const currentFlame = process.env.SOLOMON_CONTACT_FOCUS === 'current-flame'
if (currentFlame) {
  assert.equal(execFileSync('git', ['-C', websiteDir, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
    'fa08ae249ff5563570bdb2a653e2ef9358ce7b83')
  const skill = weldProfile(1003)
  const main = { cameraDisplacement: { x: .5, y: -1 }, midpoint: { x: 175, y: 240 }, targetId: 'enemy:1',
    nativeGeometry: { start: origin, control: { x: 175, y: 250 }, end: { x: 200, y: 250 }, offset: { x: 0, y: -20 } },
    preclipEndpoint: { x: 200, y: 250 }, creatorRegistration: registration('actor', 48) }
  const flame = (extra = {}) => channel('water', { kind: 'weld', primarySkill: skill,
    endpoint: { x: 200, y: 230 }, flameLashPresentation: main, ...extra })
  const neighbors = [enemy(200, 250, { health: .1 }), ...[-20, 0, 20].flatMap(dx => [-20, 0, 20]
    .filter(dy => dx !== 0 || dy !== 0).map(dy => enemy(200 + dx, 250 + dy, { health: 100 })))]
  const explosive = structuredClone(skill)
  explosive.vector.values[4] = .2; explosive.vector.values[5] = 20
  explosive.vector.values[6] = 2; explosive.vector.values[7] = 8
  cases.splice(0, cases.length,
    { name: 'current-flame-no-receiver', enemies: [enemy(900)], emissions: [flame({ flameLashPresentation: { ...main, targetId: null } })] },
    { name: 'current-flame-accepted-chain', enemies: [enemy(200), enemy(260), enemy(310)], emissions: [flame()] },
    { name: 'current-flame-weak-no-chain', enemies: [enemy(200), enemy(260)], emissions: [flame({ underpowered: true,
      flameLashPresentation: { ...main, cameraDisplacement: null } })] },
    { name: 'current-flame-coarse-factory', enhancedEffects: false, enemies: [enemy(200), enemy(260)], emissions: [flame()] },
    { name: 'current-flame-terminal-before-factory', enemies: neighbors, emissions: [flame({ primarySkill: explosive, damage: 100 })] },
    { name: 'current-flame-shield-receiver', enemies: [enemy(200, 250, { shield: 10 }), enemy(260)], emissions: [flame({ primarySkill: explosive, damage: 100 })] },
    { name: 'current-flame-cocoon-is-not-terminal', enemies: [{ token: 'COCOON', position: { x: 200, y: 250 }, health: .1 }],
      emissions: [flame({ primarySkill: explosive, damage: 100 })] },
    { name: 'current-fire-radial-two-damage-lanes', enemies: [enemy(190), enemy(190, 290), enemy(155)],
      transients: [transient('fire-explosion', { lightRegistration: registration('transient', 11), painterRegistrations: [registration('transient', 11)] })] },
    { name: 'current-fire-spider-context-flags', enemies: [{ token: 'SPIDER', position: { x: 155, y: 250 } }],
      transients: [transient('fire-explosion', { lightRegistration: registration('transient', 11), painterRegistrations: [registration('transient', 11)] })] },
    { name: 'current-steam-rectangle-damage-lanes', enemies: [enemy(190), enemy(175), enemy(155)],
      transients: [transient('fire-explosion', { presentation: 'steam', lightRegistration: registration('transient', 11), painterRegistrations: [registration('transient', 11)] })] },
  )
}

// Dedicated current witnesses use actual retained Frost Missile factory inputs.
// Terrain contacts are the real tick's outbox, not a surviving projectile row.
if (frostOnly) {
  cases.length = 0
  for (const enhancedEffects of [false, true]) for (const widen of [0, 1]) {
    for (const chillWind of [0, 2]) for (const terrain of [false, true]) {
      const actor = contactActor(sourceScreen(1001).spells.projectiles[0])
      actor.position = { ...origin }; actor.targetId = 'enemy:1'
      actor.vector[5] = chillWind; actor.vector[6] = widen
      cases.push({ name: `frost-${enhancedEffects}-${widen}-${chillWind}-${terrain}`,
        enhancedEffects, enemies: [enemy(155), enemy(172), enemy(400)],
        projectiles: terrain ? [] : [actor], frostWorldContacts: terrain ? [actor] : [],
      })
    }
  }
}
for (const sample of cases) {
  const seed = `primary-contact:${sample.name}`
  const order = managers.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 100, transient: 70 } })
  const context = { tick: 10, players: {}, resolveMovement: request => request.requestedPosition, registerWorldPainter: order.register }
  const work = workModule.createEnemyWork(stores.createBoneyardEnemyStore(seed, 50), context, true)
  const intents = sample.enemies.map((spec, index) => ({ enemyToken: spec.token, flags: [], id: index + 1,
    locationPolicy: 'anywhere', nativeTypeId: schema.BONEYARD_WAVE_ENEMY_TYPES[spec.token],
    position: spec.position, spawnTick: 10, waveOrdinal: 31 }))
  work.actors.push(...construction.materializeSpawnIntents(work, context, intents))
  for (let index = 0; index < sample.enemies.length; index += 1) {
    const spec = sample.enemies[index]
    const actor = work.actors[index]
    if (spec.hurricaneCooldown !== undefined) actor.hurricaneContactCooldown = spec.hurricaneCooldown
    if (spec.blizzardPushAccumulator !== undefined) { actor.blizzardPushAccumulator = spec.blizzardPushAccumulator; actor.blizzardPushLastTick = spec.blizzardPushLastTick }
    if (spec.health !== undefined) actor.currentHealth = spec.health
    if (spec.shield !== undefined) { actor.shieldHealth = spec.shield; actor.shieldMaximumHealth = spec.shield }
  }
  if (sample.forces) {
    const arrow = enemyProjectiles.spawnProjectile(work, work.actors[0], 10, 'arrow', 8,
      { position: { x: 200, y: 250 }, headingDeg: 90, speed: 6, minimumSpeed: 6, lifetimeTicks: 500 })
    work.projectiles = [{ ...arrow, chillTumbleAccumulator: 0.9 }]
    const creation = silk.createNativeSilk({ x: 210, y: 250 },
      { position: { x: 500, y: 250 }, velocityPerTick: { x: 0, y: 0 } }, 45, random.createNativeRng(31))
    let state = creation.state
    for (let step = 0; step < 20; step += 1) state = silk.stepNativeSilk(state)
    work.silks = [{ id: 44, ownerActorId: 1, spawnTick: 10,
      state: { ...state, forceAccumulator: 0.9 }, nativeRegistrationOrder: 60,
      nativeCellBindingOrder: 60, painterRegistration: order.register('actor') }]
  }
  const store = workModule.finishEnemyStore(work, -1)
  const spells = { nextId: 100, projectiles: sample.projectiles ?? [], transients: sample.transients ?? [] }
  const notices = []
  const observer = {
    onReward: ({ enemy, playerId }) => notices.push(['reward', enemy.enemyToken, enemy.maximumHealth, playerId]),
    attributionObserver: { onEnemyHealthDamage: ({ actorId, amount, maximumHealth, playerId }) => notices.push(['health', actorId, amount, maximumHealth, playerId]) },
  }
  const movements = []
  const result = combat.resolveBoneyardSpellCombat(store, spells, sample.emissions ?? [], sample.tick ?? 10,
    worldKey, random.createNativeRng(17), (start, end) => end.x === sample.blockedX ? 0.5 : null,
    order.register, () => sample.multiplier ?? 1, sample.scenery ?? [], observer,
    sample.contacts ?? [], (id, start, requested, radius) => {
      movements.push([id, point(start), point(requested), radius])
      return { x: Math.min(requested.x, sample.movementClampX ?? Infinity), y: requested.y }
    }, () => 1600, () => 0.8, sample.frostWorldContacts ?? [], sample.enhancedEffects ?? true)
  sample.expected = {
    ...(currentFlame ? { currentDetails: { steeringRng: result.enemies.steeringRngState,
      spiders: result.enemies.actors.filter(actor => actor.brain.family === 'spider')
        .map(actor => [actor.id, actor.brain.actionState, actor.brain.strafeSign, actor.brain.strafeTicksRemaining]) } } : {}),
    ...(frostOnly ? { events: result.events.map(event => ({ type:event.type, actorId:event.actorId,
      eventId:event.eventId, tick:event.tick, sound:event.sound??null, pitch:event.pitch??null,
      gainScale:event.gainScale??null, position:event.sourcePosition??null })),
      nextEventId:result.enemies.nextEventId,
      reactions:result.enemies.actors.map(actor=>[actor.hitFeedback.timer,actor.hitReactionTimer]) } : {}),
    actors: result.enemies.actors.map(actor => [actor.id, actor.currentHealth, actor.lifeState,
      point(actor.position), actor.shieldHealth, actor.lethalMagicDamage, actor.nativeCellBindingOrder, actor.blizzardPushAccumulator, actor.blizzardPushLastTick, actor.hurricaneContactCooldown]),
    spells: { nextId: result.spells.nextId, projectiles: result.spells.projectiles.map(spellRow), transients: result.spells.transients.map(effectRow) },
    hits: result.hits, burns: result.burns, etherBurns: result.etherBurns, targetEffects: result.targetEffects,
    rng: result.rng, enemyRng: result.enemies.rngState, notices, movements,
    painterOrder: order.state().nextRegistrationOrdinal,
    forces: {
      arrows: result.enemies.projectiles.map(p => [p.id, p.chillTumbleAccumulator]),
      silks: result.enemies.silks.map(p => [p.id, p.state.forceAccumulator]),
      fragments: result.enemies.silkFragments.map(p => [p.id, point(p.state.start), point(p.state.middle),
        point(p.state.end), point(p.state.velocity), p.state.opacity, p.state.opacityLossPerTick]),
      projectileEffects: result.enemies.projectileEffects.map(p => [p.id, p.kind, point(p.position)]),
    },
  }
}

writeFixture(currentFlame ? 'flame-contact-goldens.json' : frostOnly ? 'current-frost-contact-goldens.json' : 'primary-contact-goldens.json', 'tools/website-oracle/primary-contact-goldens.mjs',
  'Website/frontend/src/game/core-server/boneyard-spell-combat.ts and spell-combat/*.ts', { sourceRevision, cases })
console.log(`primary contact: ${cases.length} source transactions`)
