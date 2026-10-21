#!/usr/bin/env node
// The real secondary receiver and terminal dispatcher run before the next contact.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const stores = await loadKernel('core-server/boneyard-enemy-store.ts')
const construction = await loadKernel('core-server/enemies/construction.ts')
const work = await loadKernel('core-server/enemies/work.ts')
const secondary = await loadKernel('core-server/native-secondary-world.ts')
const death = await loadKernel('core-server/spell-combat/special-death.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const managers = await loadKernel('core-kernels/native-world-manager-order.ts')
const schema = await loadKernel('core-kernels/boneyard-wave-schema.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const bits = value => {
  if (typeof value === 'number') { const bytes = Buffer.alloc(8); bytes.writeDoubleBE(value); return `f64:${bytes.toString('hex')}` }
  if (Array.isArray(value)) return value.map(bits)
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, bits(item)]))
  return value
}
const registration = value => value ? [value.managerLane, value.registrationOrdinal] : null
const point = value => [value.x, value.y]
const payload = { kind: 'steam', emberDamage: .25, emberFragments: 3,
  explodeDamage: 2, explodeRadius: 55, privateSeed: 4137 }
const cases = [
  { name: 'surviving-contact', health: 10 },
  { name: 'terminal-before-next-contact', health: .1, neighborHealth: .1 },
  { name: 'terminal-neighbor-survives', health: .1, neighborHealth: 10 },
  { name: 'zero-damage-does-not-dispatch', health: .1, amount: 0 },
  { name: 'shield-is-not-terminal', health: .1, shield: 20 },
  { name: 'armor-break-is-not-terminal', health: .1, flags: ['FLAG_ARMOR'] },
  { name: 'cocoon-release-is-not-terminal', health: .1, token: 'COCOON' },
  { name: 'terminal-without-payload', health: .1, noPayload: true },
  { name: 'terminal-zero-radius', health: .1, radius: 0 },
  { name: 'terminal-reduced-effects', health: .1, enhancedEffects: false },
]
for (const sample of cases) {
  const seed = `secondary-special:${sample.name}`
  const order = managers.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 100, transient: 70 } })
  const context = { tick: 10, players: {}, resolveMovement: request => request.requestedPosition, registerWorldPainter: order.register }
  const initial = work.createEnemyWork(stores.createBoneyardEnemyStore(seed, 50), context, true)
  const specs = [
    { token: sample.token ?? 'SKELETON', flags: sample.flags ?? [], position: { x: 150, y: 250 } },
    { token: 'SKELETON', flags: [], position: { x: 160, y: 250 } },
  ]
  initial.actors.push(...construction.materializeSpawnIntents(initial, context, specs.map((spec, index) => ({
    enemyToken: spec.token, flags: spec.flags, id: index + 1, locationPolicy: 'anywhere',
    nativeTypeId: schema.BONEYARD_WAVE_ENEMY_TYPES[spec.token], position: spec.position, spawnTick: 10, waveOrdinal: 31,
  }))))
  initial.actors[0].currentHealth = sample.health
  initial.actors[1].currentHealth = sample.neighborHealth ?? 10
  if (sample.shield) { initial.actors[0].shieldHealth = sample.shield; initial.actors[0].shieldMaximumHealth = sample.shield }
  const store = work.finishEnemyStore(initial, -1)
  const contacts = [1, 2].map((id, index) => ({ amount: index ? .125 : sample.amount ?? .25,
    hitStrength: index ? 0 : .625, suppressHitReaction: true, suppressHurtSound: index === 1,
    kind: 'fire', ownerId: 'hero', sourceActorId: 77 + index, targetId: id,
    ...(!index && !sample.noPayload ? { specialDeathPayload: { ...payload, explodeRadius: sample.radius ?? payload.explodeRadius } } : {}),
  }))
  const notices = []
  const observer = { onReward: ({ enemy, playerId }) => notices.push(['reward', enemy.enemyToken, playerId]),
    attributionObserver: { onEnemyHealthDamage: ({ actorId, amount, playerId }) => notices.push(['health', actorId, amount, playerId]) } }
  let rng = random.createNativeRng(17)
  let nextSpellId = 100
  const transients = []
  const dispatches = []
  const resolved = secondary.resolveBoneyardNativeSecondaryCombat(store, { damage: contacts,
    dampenedCasterTargetIds: [], dispelledShieldTargetIds: [], targetHeadingChanges: [], removedProjectileIds: [] },
  10, observer, () => 1, order.register, [], sample.enhancedEffects ?? true, (enemies, contact, victim) => {
    dispatches.push([contact.sourceActorId, victim.id, contact.specialDeathPayload.privateSeed, point(victim.position)])
    const result = death.resolveBoneyardSpecialDeathPayload({ payload: contact.specialDeathPayload, victim,
      sourceEnemies: enemies, nextSpellId, rng, ownerId: contact.ownerId, tick: 10,
      worldKey: 'boneyard:secondary-special', enhancedEffects: sample.enhancedEffects ?? true,
      registerWorldPainter: order.register, lethalObserver: observer })
    rng = result.rng; nextSpellId = result.spells.nextId
    transients.push(...result.spells.transients)
    return { enemies: result.enemies, events: result.events }
  })
  sample.seed = seed
  sample.contacts = contacts
  sample.expected = bits({
    actors: resolved.enemies.actors.map(actor => [actor.id, actor.currentHealth, actor.lifeState,
      actor.shieldHealth, actor.hitFeedback, actor.hitReactionTimer]),
    dispatches, notices, rng, enemyRng: resolved.enemies.rngState, nextSpellId,
    transients: transients.map(actor => ({ id: actor.id, kind: actor.kind, age: actor.ageTicks,
      ownerId: actor.ownerId, worldKey: actor.worldKey, light: registration(actor.lightRegistration),
      painters: actor.painterRegistrations.map(registration),
      ...(actor.kind === 'fire-explosion' ? { origin: point(actor.origin), damage: actor.damage,
        footprintDimension: actor.footprintDimension, visualScale: actor.visualScale, soundPitch: actor.soundPitch,
        presentation: actor.presentation } : { position: point(actor.position), velocity: point(actor.velocity),
        life: actor.life, scale: actor.scale, contactEnabled: actor.contactEnabled, contactDue: actor.contactDue,
        contactDamage: actor.contactDamage, phase: actor.phase, variant: actor.variant }),
    })),
    painter: order.state().nextRegistrationOrdinal,
  })
}
writeFixture('current-secondary-special-death-goldens.json', 'tools/website-oracle/current-secondary-special-death-goldens.mjs',
  'current native-secondary-world.ts applyContact + spell-combat/special-death.ts with actual enemy constructors',
  { sourceRevision, cases })
