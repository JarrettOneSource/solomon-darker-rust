// Controlled constructor inputs for whole-save round trips, never a live authority.
// All retained motion and IDs below come from the unmodified Source factories.
import { loadKernel } from './oracle.mjs'
import { allLearnedBook, authority, context, factors } from './secondary-inputs.mjs'

const secondary = await loadKernel('core-kernels/native-secondary-abilities.ts')
const deaths = await loadKernel('core-kernels/native-death-animations.ts')
const equipment = await loadKernel('core-kernels/player-equipment-appearance.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const manager = await loadKernel('core-kernels/native-world-manager-order.ts')

export function withRetainedSecondaryDeathActors(game, ownerId = game.playerEntities.identities[0].playerId) {
  if (game.world.kind !== 'boneyard') throw new Error('Retained death input needs a Boneyard')
  const index = game.playerEntities.identities.findIndex(row => row.playerId === ownerId)
  if (index < 0) throw new Error('Retained death input needs its actual owner')
  const character = entities.playerCharacterAt(game.playerEntities, ownerId)
  const lighting = game.playerEntities.lightings[index]
  const worldKey = `boneyard:${game.world.runId}`
  const order = manager.createNativeWorldManagerOrder(game.worldManagerOrder)
  const caster = { ...authority(allLearnedBook(), factors(false)), alive: true, character,
    enhancedEffects: game.enhancedEffects, worldKey }
  const ctx = tick => ({ ...context(caster, tick), enhancedEffects: game.enhancedEffects,
    players: { [ownerId]: caster }, registerWorldPainter: order.register })

  // Isolate the fixture's constructor epoch from pre-existing spell actors. The
  // Golem must complete its real 400 assembly updates before its damage receiver admits death.
  let state = { ...game.secondaryAbilities, actors: [], events: [], players: {} }
  state = secondary.activateNativeSecondaryBeltSkill(state, ownerId, 45, 0, ctx(game.tick)).state
  const golemId = state.actors.find(actor => actor.kind === 'golem').id
  for (let tick = 1; tick <= 400; tick += 1) {
    state = secondary.stepNativeSecondaryAbilities(state, ctx(tick)).state
  }
  state = { ...state, events: [] }
  const damaged = secondary.applyNativeSecondaryGolemDamage(state, golemId, {
    primaryDamage: 1_000_000, secondaryDamage: 0, reflectablePhysicalSourceInRange: false,
  }, game.tick, game.enhancedEffects, order.register)
  if (!damaged.killed) throw new Error('Source did not admit the prepared Golem death')
  state = damaged.state
  const player = state.players[ownerId]
  state = { ...state, players: { ...state.players, [ownerId]: { ...player,
    globalCooldownTicks: 0, castAction: null, castSpinTicksRemaining: 0,
  } } }
  state = secondary.activateNativeSecondaryBeltSkill(state, ownerId, 74, 1, ctx(game.tick)).state
  const drain = state.actors.find(actor => actor.kind === 'ether-drain')
  state = secondary.captureNativeEtherDrainImage(state, drain.id,
    { atlas: 'Demon', bodyYOffset: 0, entry: 80, tint: 0xffffff }, game.tick)

  const appearance = equipment.playerDeathEquipmentAppearance(character.config.element,
    game.playerEntities.economies[index].equipment)
  const damagedOwner = entities.damagePlayerEntity(game.playerEntities, ownerId, 1_000_000, game.tick)
  const ownerCombat = entities.stepPlayerEntityCombatTick(damagedOwner, state.rng, new Set(), {}, game.tick)
  if (ownerCombat.store.progressions[index].lifeState !== 'dying') {
    throw new Error('Retained death input owner must admit the lethal damage receiver')
  }
  const weaponRegistration = order.register('actor')
  const playerEntities = entities.setPlayerDeathWeaponPainterRegistration(ownerCombat.store, ownerId, weaponRegistration)
  const weapon = deaths.createNativeDeathWeaponActor({ deathEpoch: playerEntities.progressions[index].deathEpoch,
    headingIndex: character.headingIndex, id: game.world.nextDeathWeaponId, ownerId,
    painterRegistration: weaponRegistration, position: character.position,
    tick: game.tick, weapon: appearance.weapon }, ownerCombat.rng)
  return { ...game, playerEntities, worldManagerOrder: order.state(), secondaryAbilities: {
    ...state, rng: weapon.rng, actors: [...game.secondaryAbilities.actors, ...state.actors],
    events: [...game.secondaryAbilities.events, ...state.events],
    players: { ...game.secondaryAbilities.players, ...state.players },
  }, world: { ...game.world, nextDeathWeaponId: game.world.nextDeathWeaponId + 1,
    deathWeapons: [...game.world.deathWeapons, weapon.actor] } }
}
