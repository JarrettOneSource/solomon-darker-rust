// Ground-item lifecycle, Goodies, shared painter order and completion cleanup.
import { loadKernel, writeFixture } from './oracle.mjs'

const store = await loadKernel('core-server/boneyard-loot-store.ts')
const loot = await loadKernel('core-kernels/native-loot.ts')
const items = await loadKernel('core-kernels/native-loot-items.ts')
const { createNativeWorldManagerOrder } = await loadKernel('core-kernels/native-world-manager-order.ts')
const { rollBoneyardLootSeed } = await loadKernel('core-server/boneyard-enemy-loot-seed.ts')
const { nearestBoneyardGoodie } = await loadKernel('core-kernels/boneyard-goodie-interaction.ts')

const participant = (playerId, position, perks = []) => ({ playerId, position,
  advancedUnlocks: new Array(8).fill(false), alive: true, connected: true,
  headingIndex: 0, level: 19, modifiers: loot.nativeLootModifiers(perks), ownedRecipeIndexes: [] })
const far = [participant('far', { x: 10000, y: 10000 })]
const spec = (kind, position, overrides = {}) => ({ activationDelayTicks: 0, id: 0,
  kind, nativeTypeId: { gold: 2012, sack: 2013, bonus: 2038, orb: 2011 }[kind],
  phase: 0, position, source: 'enemy', ...overrides })
const potion = () => items.potionItem(items.createNativeLootItemIds(1), 0)
const cases = []

function run(name, seed, goodies, operations) {
  const order = createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 10, transient: 20 } })
  let current = store.createBoneyardLootStore(seed, goodies)
  const initial = current
  const records = []
  for (const operation of operations) {
    let output = {}
    if (operation.kind === 'spawn') {
      const result = store.spawnBoneyardLootSpecs(current, operation.specs, operation.tick, order.register)
      current = result.store
      output.rejectedCount = result.rejectedCount
    } else if (operation.kind === 'step') {
      const placement = operation.blocked
        ? { canPlace: ({ x, y }) => x < 95 || x > 105 || y < 95 || y > 105 }
        : loot.NATIVE_LOOT_OPEN_PLACEMENT
      const result = store.stepBoneyardLootStore(current, { participants: operation.participants, placement,
        tick: operation.tick, registerWorldPainter: order.register })
      current = result.store
      output = { events: result.events, pickups: result.pickups }
    } else if (operation.kind === 'activate') {
      current = store.activateBoneyardGoodie(current, operation.eid, operation.playerId)
    } else if (operation.kind === 'key-needed') {
      const result = store.boneyardGoodieKeyNeeded(current, operation.goodieId, operation.playerId, operation.tick)
      current = result.store
      output.event = result.event
    } else if (operation.kind === 'retire') {
      current = store.retireBoneyardGoodiesOutsideBounds(current, operation.bounds)
    } else if (operation.kind === 'remove-last-word') {
      output = store.nativeHagathaLastWordLoot(current)
      current = store.removeBoneyardLootActors(current, output.actorIds)
    } else if (operation.kind === 'custom') {
      const result = store.spawnBoneyardCustomLootItems(current, operation.items, operation.position, operation.tick, order.register)
      current = result.store
      output.rejectedCount = result.rejectedCount
    } else if (operation.kind === 'materialize') {
      const result = store.materializeBoneyardEnemyLoot(current, { ...operation.input,
        placement: loot.NATIVE_LOOT_OPEN_PLACEMENT }, order.register)
      current = result.store
      output.rejectedCount = result.rejectedCount
    } else if (operation.kind === 'seed') {
      const result = rollBoneyardLootSeed(current, operation.bound)
      current = result.store
      output.seed = result.seed
    }
    records.push({ operation, output, expected: current,
      nextRegistrationOrdinal: order.state().nextRegistrationOrdinal })
  }
  cases.push({ name, seed, goodies, initial, records })
}

for (const seed of ['ground-one', 'ground-two']) {
  const specs = [spec('gold', { x: 0, y: 0 }, { amount: 7, tier: 2, phase: 13, scatterSeed: 731 }),
    spec('gold', { x: 20, y: 0 }, { amount: 25, tier: 3, activationDelayTicks: 4 }),
    spec('sack', { x: 200, y: 0 }, { item: potion() }),
    spec('sack', { x: 240, y: 0 }, { item: items.miscItem(items.createNativeLootItemIds(100), 1) }),
    spec('orb', { x: 400, y: 0 }, { orbKind: 'health', value: Math.fround(0.3), phase: 45 }),
    spec('orb', { x: 440, y: 0 }, { orbKind: 'mana', value: Math.fround(0.01) }),
    ...[0, 1, 2].map((bonusKind) => spec('bonus', { x: 600 + 40 * bonusKind, y: 0 }, { bonusKind }))]
  const operations = [{ kind: 'spawn', specs, tick: 1 }]
  for (const tick of [1, 4, 16, 17, 25, 40, 899, 900, 901, 950, 1050, 1199, 1200, 1201, 1250, 1320]) {
    operations.push({ kind: 'step', tick, participants: far })
  }
  operations.push({ kind: 'remove-last-word' })
  run(`decay-${seed}`, seed, [], operations)
}

for (const perks of [[], [3, 4, 9, 23]]) {
  const operations = [{ kind: 'spawn', tick: 1, specs: [
    spec('gold', { x: 0, y: 0 }, { amount: 9 }),
    spec('sack', { x: 150, y: 0 }, { item: potion(), activationDelayTicks: 3 }),
    spec('orb', { x: 300, y: 0 }, { orbKind: 'mana', value: 0.5 }),
    spec('bonus', { x: 450, y: 0 }, { bonusKind: 1 }),
  ] }]
  for (const [tick, x] of [[1, 37.5], [2, 0], [3, 150], [4, 150], [20, 350], [45, 300], [50, 450], [100, 0]]) {
    const participants = [participant('dead', { x, y: 0 }), participant('first', { x, y: 0 }, perks),
      participant('second', { x: x + 10, y: 0 }, perks)]
    participants[0].alive = false
    if (tick === 2) participants[1].connected = false
    operations.push({ kind: 'step', tick, participants })
  }
  run(`pickups-${perks.length}`, `pickup-${perks.length}`, [], operations)
}

for (let selector = 0; selector < 18; selector++) {
  const goodie = { eid: 'chest', position: { x: 100, y: 100 }, rewardSeed: selector,
    sceneryRegistrationOrdinal: 31, subtype: selector % 2 }
  const operations = [
    { kind: 'key-needed', goodieId: 1, playerId: 'far', tick: 0 },
    { kind: 'key-needed', goodieId: 1, playerId: 'far', tick: 200 },
    { kind: 'key-needed', goodieId: 1, playerId: 'far', tick: 201 },
    { kind: 'activate', eid: 'chest', playerId: 'far' },
  ]
  for (const tick of [1, 99, 100, 150, 199, 200, 249, 250, 300, 500]) {
    operations.push({ kind: 'step', tick, participants: far, blocked: selector % 2 === 1 })
  }
  operations.push({ kind: 'activate', eid: 'chest', playerId: 'far' }, { kind: 'remove-last-word' },
    { kind: 'retire', bounds: { x: 0, y: 0, w: 100, h: 100 } },
    { kind: 'retire', bounds: { x: 0, y: 0, w: 99, h: 99 } })
  run(`goodie-${selector}`, `goodie-${selector}`, [goodie], operations)
}
run('default-goodie-seeds-and-custom', 'mixed-goodies', [
  { eid: 'first', position: { x: 10, y: 20 } }, { eid: 'second', position: { x: 30, y: 40 }, rewardSeed: 17 },
], [{ kind: 'custom', tick: 5, position: { x: 200, y: 300 }, items: [potion(),
  { equipmentType: null, iconRecords: [70], id: 99, kind: 'sack', name: 'Sack', nativeSubtype: 0,
    nativeTypeId: 7008, quantity: 1, rarity: null, recipeIndex: null, contents: [potion()] }] },
  { kind: 'step', tick: 6, participants: [participant('local', { x: 200, y: 300 })] },
  { kind: 'step', tick: 100, participants: far }])

for (const category of ['gold', 'orb', 'item', 'potion', 'powerup']) {
  for (const onDeathProgram of [null, 'miniboss-die']) {
    const policies = { gold: 4, item: 4, orb: 4, potion: 4, powerup: 4, specificItem: 0 }
    policies[category] = 3
    const input = { advancedUnlocks: new Array(8).fill(true), actorSeed: 110,
      arena: { disableMask: 0, itemLevelMinimum: 0, itemLevelMaximum: 100, level: 30,
        mode: 0, specialSuppression: false }, inventoryHasHealthPotion: false,
      modifiers: loot.nativeLootModifiers([3, 4, 9, 23]), nearbyMaskTwoCount: 60,
      onDeathProgram, ownedRecipeIndexes: [], participantLevel: 31, participantSlot: 0,
      policies, position: { x: 100, y: 200 }, tick: 5, sceneForcesHealthPotion: false,
      worldBadguyCount: 100, worldHasHealthPotionSack: false }
    run(`enemy-${category}-${onDeathProgram}`, `enemy-${category}-${onDeathProgram}`, [], [
      { kind: 'materialize', input }, { kind: 'step', tick: 6, participants: far },
      { kind: 'step', tick: 100, participants: far }, { kind: 'remove-last-word' }])
  }
}
run('shared-enemy-seed-writers', 'loot-seed-owner', [], [
  { kind: 'seed', bound: 10_000_000 }, { kind: 'seed', bound: 1_000_000 },
  { kind: 'spawn', tick: 1, specs: [spec('gold', { x: 0, y: 0 }, { amount: 7 })] },
  { kind: 'step', tick: 1, participants: [participant('local', { x: 0, y: 0 })] },
  { kind: 'seed', bound: 1_000_000 }, { kind: 'seed', bound: 10_000_000 },
])

const capacityOrder = createNativeWorldManagerOrder()
const nearest = []
for (let headingIndex = 0; headingIndex < 24; headingIndex++) {
  const actor = { headingIndex, position: { x: 100, y: 200 } }
  const heading = headingIndex * Math.PI / 12
  const probe = { x: Math.fround(100 + Math.sin(heading) * 25), y: Math.fround(200 - Math.cos(heading) * 25) }
  const goodie = (id, x, y, extra = {}) => ({ id, eid: `nearest-${id}`, active: false, exhausted: false, phase: 0,
    position: { x, y }, ...extra })
  for (const goodies of [
    [goodie(1, probe.x + 50, probe.y)],
    [goodie(1, probe.x + 49.99, probe.y)],
    [goodie(1, probe.x - 10, probe.y), goodie(2, probe.x + 10, probe.y)],
    [goodie(1, probe.x, probe.y, { active: true }), goodie(2, probe.x, probe.y, { exhausted: true }),
      goodie(3, probe.x, probe.y, { phase: 1 }), goodie(4, probe.x, probe.y, { phase: 2 })],
  ]) nearest.push({ actor, goodies, expected: nearestBoneyardGoodie(goodies, actor)?.id ?? null })
}
let capacity = { ...store.createBoneyardLootStore('capacity'), nextActorId: 2047 }
const capacitySpecs = Array.from({ length: 2048 }, () => spec('gold', { x: 0, y: 0 }, { amount: 1 }))
const filled = store.spawnBoneyardLootSpecs(capacity, capacitySpecs, 1, capacityOrder.register)
capacity = store.removeBoneyardLootActors(filled.store, [2047, 2])
const reused = store.spawnBoneyardLootSpecs(capacity, capacitySpecs.slice(0, 3), 2, capacityOrder.register)
writeFixture('loot-store-goldens.json', 'tools/website-oracle/loot-store-goldens.mjs',
  'Website core-server/boneyard-loot-store.ts and core-kernels/boneyard-goodie-interaction.ts', { cases, nearest,
    capacity: { filledIds: filled.store.actors.map(({ id }) => id), filledNext: filled.store.nextActorId,
      filledRejected: filled.rejectedCount, reusedIds: reused.store.actors.map(({ id }) => id),
      reusedNext: reused.store.nextActorId, reusedRejected: reused.rejectedCount,
      nextRegistrationOrdinal: capacityOrder.state().nextRegistrationOrdinal } })
console.log(JSON.stringify({ cases: cases.length, snapshots: cases.reduce((sum, value) => sum + value.records.length, 0) }))
