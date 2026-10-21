import { loadKernel, writeFixture } from './oracle.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const skills = await loadKernel('core-kernels/player-skill-runtime.ts')
const primary = await loadKernel('core-kernels/native-primary-skill-profile.ts')
const spellKernel = await loadKernel('core-kernels/primary-spells.ts')
const targeting = await loadKernel('core-kernels/primary-spell-targeting.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const managers = await loadKernel('core-kernels/native-world-manager-order.ts')
const construction = await loadKernel('core-server/boneyard-world-construction.ts')
const placement = await loadKernel('core-server/boneyard-world-placement.ts')
const collision = await loadKernel('core-server/boneyard-collision.ts')
const enemyConstruction = await loadKernel('core-server/enemies/construction.ts')
const enemyWork = await loadKernel('core-server/enemies/work.ts')
const worldTargets = await loadKernel('core-server/boneyard-world-targets.ts')
const combat = await loadKernel('core-server/boneyard-spell-combat.ts')
const secondary = await loadKernel('core-kernels/native-secondary-abilities.ts')
const secondaryWorld = await loadKernel('core-server/native-secondary-world.ts')

const scene = {
  name: 'Primary adapter oracle', environmentMode: 0,
  bounds: { x: 0, y: 0, w: 1000, h: 800 }, spawn: { x: 150, y: 250, facingDeg: 90 },
  objects: [{ eid: 'tree', typeId: 2001, pos: { x: 450, y: 250 }, variant: 0 },
    { eid: 'grave', typeId: 2029, pos: { x: 650, y: 250 }, variant: 0 },
    { eid: 'monument', typeId: 2009, pos: { x: 700, y: 600 }, variant: 0 },
    { eid: 'building', typeId: 2040, pos: { x: 900, y: 600 }, variant: 0 },
    { eid: 'decoration', typeId: 9999, pos: { x: 1, y: 1 } }],
  fences: [{ eid: 'first', typeId: 2002, points: [{ x: 100, y: 700 }, { x: 200, y: 700 }], style: 0 },
    { eid: 'shared', typeId: 2002, points: [{ x: 200, y: 700 }, { x: 300, y: 700 }], style: 2 },
    { eid: 'wall', typeId: 2002, points: [{ x: 300, y: 700 }, { x: 400, y: 700 }], style: 3 }],
  roads: [], sprites: [], terrain: [], solomonDig: null,
}
const loaded = { choice: { id: 'oracle', name: scene.name, source: 'custom' }, scene,
  runId: 'adapter', seed: 'primary-adapter', sourceSha256: 'oracle', geometrySha256: 'oracle' }
const point = value => value ? [value.x, value.y] : null
const reg = value => value ? [value.managerLane, value.registrationOrdinal] : null

const scenery = placement.createBoneyardSceneryTargets(scene).primarySceneryTargets
const cleanupBounds = { x: 600, y: 200, w: 400, h: 500 }
const sceneryRow = target => [target.id, point(target.position), target.bodyRadius, target.kind,
  target.registrationOrder, target.cellBindingOrder, target.nativePriority]
const viewRays = [
  { start: { x: 150, y: 250 }, direction: { x: 1, y: 0 }, view: scene.bounds },
  { start: { x: 150, y: 250 }, direction: { x: -0.6, y: 0.8 }, view: scene.bounds },
  { start: { x: 150, y: 250 }, direction: { x: 0, y: 0 }, view: scene.bounds },
  { start: { x: 150.25, y: 250.125 }, direction: { x: Math.fround(0.7), y: Math.fround(-0.3) },
    view: targeting.nativePrimaryViewBounds({ bounds: scene.bounds, focus: { x: 500, y: 400 },
      padding: 16, scale: 1.35, viewportWidth: 1600, viewportHeight: 900 }) },
].map(input => ({ ...input, endpoint: point(targeting.nativePrimaryViewRayEndpoint(input)) }))

const cases = []
const journeys = ['air', 'earth', 'ether', 'fire', 'water'].map(element => ({ element, name: element }))
journeys.push({ element: 'air', name: 'air-prismatic-boss', boss: true })
for (const journey of journeys) {
  const { element } = journey
  const config = { displayName: 'Oracle', element, discipline: 'arcane' }
  const initial = game.createGameSimulation({ hero: config }, { gameRngSeed: 7 })
  const store = initial.playerEntities
  if (journey.boss) store.economies[0].ownedPerkSelectors = [22]
  const book = store.skillBooks[0]
  const runtime = store.skillRuntimes[0]
  const derived = skills.playerSkillDerivedStats(runtime, book, store.statBooks[0], store.progressions[0], store.economies[0])
  const profile = primary.nativePrimarySkillProfile(book, store.statBooks[0], {
    damage: derived.offensiveDamageFactor, equipment: runtime.equipmentModifiers,
    globalFlatDamage: derived.offensiveDamageFlat, globalManaReduction: derived.offensiveManaCostReduction,
    manaCost: derived.offensiveManaCostFactor,
  })
  let player = { ...entities.playerCharacterAt(store, 'hero'), position: { x: 150, y: 250 }, headingIndex: 6 }
  let world = construction.createBoneyardWorld(loaded)
  const castCollision = collision.withBoneyardGateCollision(world.collision, world.gateLeaves)
  let spells = spellKernel.createPrimarySpellSimulation()
  let abilities = secondary.createNativeSecondarySimulation(0)
  if (journey.boss) abilities = secondary.applyNativeSecondaryTargetEffect(abilities, 'boneyard:adapter', 1, { prismaticTicks: 25 })
  let rng = random.createNativeRng(17)
  let mana = store.progressions[0].currentMana
  const order = managers.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 100, transient: 70 } })
  const birthContext = { tick: 0, players: {}, resolveMovement: request => request.requestedPosition, registerWorldPainter: order.register }
  const birth = enemyWork.createEnemyWork(world.enemies, birthContext, true)
  birth.actors.push(...enemyConstruction.materializeSpawnIntents(birth, birthContext,
    [250, 320].map((x, index) => ({ enemyToken: journey.boss && index === 0 ? 'DEMONSKULL' : 'SKELETON', flags: [], id: index + 1,
      nativeTypeId: journey.boss && index === 0 ? 1008 : 1001,
      position: { x, y: 250 }, locationPolicy: 'anywhere', spawnTick: 0, waveOrdinal: 31 }))))
  world = { ...world, enemies: enemyWork.finishEnemyStore(birth, -1) }
  const frames = []
  for (let tick = 0; tick <= 170; tick += 1) {
    const cast = spellKernel.stepPrimarySpells({
      canPlaceProjectile: (_spell, position, radius) => collision.canPlaceBoneyardBody(position, world.bounds, castCollision, radius),
      canTraverseProjectile: (spell, start, end, radius = 0, mask = 0) => radius > 0
        ? collision.firstBoneyardPathBlockProgress(start, end, world.bounds, castCollision, radius,
          spell.kind === 'earth' ? new Set(['scenery:grave']) : undefined) === null
        : collision.firstBoneyardLineObstruction(start, end, world.bounds, castCollision, undefined, mask) === null,
      castAuthority: { hero: { alive: true, availableMana: mana, castProgressFactor: derived.castProgressFactor,
        eligible: tick < 160, planeActive: false, primarySkill: profile, weaponKind: 'staff' } },
      inputs: { hero: { aim: { x: 500, y: 250 }, cast: { primary: tick < 150, quickbar: null },
        movement: { x: 0, y: 0 }, viewportWidth: 1600, viewportHeight: 900 } },
      players: { hero: player }, previousPlayers: { hero: player }, registerWorldPainter: order.register,
      rng, spells, tick, viewScale: 1.35,
      spellObstructionPoint: (_owner, start, end, excluded, mask) => collision.firstBoneyardLineObstruction(start, end, world.bounds, castCollision, excluded, mask),
      spellRangeEndpoint: (_owner, start, direction, padding) => targeting.nativePrimaryViewRayEndpoint({ start, direction,
        view: targeting.nativePrimaryViewBounds({ bounds: world.bounds, focus: player.position, padding,
          scale: 1.35, viewportWidth: 1600, viewportHeight: 900 }) }),
      spellTargets: () => worldTargets.boneyardPrimarySpellTargets(world), worldKeyForPlayer: () => 'boneyard:adapter',
    })
    player = cast.players.hero
    mana = Math.max(0, mana - cast.manaSpent.hero)
    const contact = combat.resolveBoneyardSpellCombat(world.enemies, cast.spells, cast.channelEmissions,
      tick, 'boneyard:adapter', cast.rng,
      (start, end, radius) => collision.firstBoneyardPathBlockProgress(start, end, world.bounds, castCollision, radius),
      order.register, (id, kind) => journey.boss && id === 1 ? (kind === 'air' ? 6 : 3) : 1,
      worldTargets.boneyardWorldSceneryTargets(world), undefined,
      cast.fireActorContacts, (_id, start, requested, radius) => collision.resolveBoneyardMovement(start, requested, world.bounds, castCollision, radius),
      [], () => 1600, () => 1)
    world = { ...world, enemies: contact.enemies }
    spells = contact.spells
    rng = contact.rng
    const previousId = abilities.nextActorId
    for (const burn of contact.burns) {
      const target = secondaryWorld.boneyardNativeSecondaryTarget(world.enemies, burn.targetId)
      if (target) abilities = secondary.applyNativeSecondaryFireBurn(abilities, {
        damage: burn.damage, ownerId: burn.ownerId, rank: Math.max(1, book.effectiveRanks[22] ?? 0), skillId: 22, target, worldKey: 'boneyard:adapter' })
    }
    for (const burn of contact.etherBurns) {
      const target = secondaryWorld.boneyardNativeSecondaryTarget(world.enemies, burn.targetId)
      if (target) abilities = secondary.applyNativeSecondaryEtherBurn(abilities, {
        ownerId: burn.ownerId, rank: Math.max(1, book.effectiveRanks[14] ?? 0), target, worldKey: 'boneyard:adapter' })
    }
    if (abilities.nextActorId !== previousId) abilities = secondary.enrollNativeSecondaryPainterOwners(abilities, order.register)
    for (const effect of contact.targetEffects) abilities = secondary.applyNativeSecondaryTargetEffect(abilities, effect.worldKey, effect.targetId, effect.patch)
    if (![0, 1, 14, 18, 59, 95, 149, 150, 160, 170].includes(tick)) continue
    frames.push({ tick, mana, spent: cast.manaSpent.hero, underflow: cast.manaUnderflowPlayerIds.length > 0,
      heading: player.headingIndex, cast: [player.primaryCast.actionTick, player.primaryCast.castSequence,
        player.primaryCast.channelActive, player.primaryCast.emissionSequence, player.primaryCast.targetId],
      actors: world.enemies.actors.map(actor => [actor.id, actor.currentHealth, actor.lifeState, point(actor.position)]),
      spells: { nextId: spells.nextId,
        projectiles: spells.projectiles.map(p => [p.id, p.kind, p.ageTicks, p.phase, point(p.position), p.charge, p.damage]),
        transients: spells.transients.map(p => [p.id, p.kind, p.ageTicks]) },
      secondary: abilities.actors.map(actor => [actor.id, actor.kind, actor.damage, actor.targetId, reg(actor.lightRegistration), actor.painterRegistrations.map(reg)]),
      targetEffects: abilities.targetEffects.map(effect => [effect.targetId, effect.coldSlowFactor, effect.coldSlowTicks]),
      secondaryTargets: world.enemies.actors.map(actor => {
        const target = secondaryWorld.boneyardNativeSecondaryTarget(world.enemies, actor.id)
        return target ? [target.family, target.id, reg(target.lightRegistration), target.nativeFlags,
          point(target.position), target.radius, target.scale, target.shieldHealth] : null
      }),
      radialTargets: [0, 100, 200].map(radius => secondaryWorld.boneyardNativeSecondaryTargets(
        world.enemies, player.position, radius).map(target => target.id)),
      rng, order: order.state().nextRegistrationOrdinal,
    })
  }
  cases.push({ ...journey, frames })
}

writeFixture('boneyard-primary-goldens.json', 'tools/website-oracle/boneyard-primary-goldens.mjs',
  'Website/frontend/src/game/core-server/game-simulation.ts primary phases and native callbacks', {
    scene, cleanupBounds, scenery: scenery.map(sceneryRow),
    cleanedScenery: placement.retainInsideBounds(scenery, cleanupBounds, target => target.id.startsWith('fencepost:')).map(sceneryRow),
    viewRays, cases,
  })
