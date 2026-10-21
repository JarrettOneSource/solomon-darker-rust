// Shared read-only projection of accepted public Game state.
import { loadKernel } from './oracle.mjs'
import { playerStateSnapshot } from './player-state-snapshot.mjs'
import { snapshot as playerCombatSnapshot } from './player-entity-fixture.mjs'
const game = await loadKernel('core-server/game-simulation.ts')

const point = value => [value.x, value.y]
const registration = value => value == null ? null : [value.managerLane, value.registrationOrdinal]

export function sessionTickSnapshot(state, playerId = 'local') {
  const store = state.playerEntities
  const index = store.identities.findIndex(identity => identity.playerId === playerId)
  const character = game.getPlayerCharacter(state, playerId)
  const progression = store.progressions[index]
  const world = state.world
  const enemies = world.kind === 'boneyard' ? world.enemies : null
  return {
    tick: state.tick,
    player: playerStateSnapshot(state, index),
    combat: playerCombatSnapshot(store, index).combat,
    potionClocks: [progression.damageX4TicksRemaining, progression.mindChugTicksRemaining,
      progression.poisonImmunityTicksRemaining, progression.hagathaRuntime],
    runtime: playerCombatSnapshot(store, index).runtime,
    character,
    lighting: store.lightings[index],
    combatRng: state.combatRng,
    secondary: state.secondaryAbilities,
    primary: {
      nextId: state.primarySpells.nextId,
      projectiles: state.primarySpells.projectiles.map(actor => [actor.id, actor.kind,
        actor.ageTicks, point(actor.position), point(actor.velocity), actor.damage,
        actor.phase, actor.worldKey, registration(actor.lightRegistration)]),
      transients: state.primarySpells.transients.map(actor => [actor.id, actor.kind,
        actor.ageTicks, actor.worldKey, registration(actor.lightRegistration),
        actor.painterRegistrations.map(registration)]),
    },
    barrier: state.levelUpBarrier,
    run: state.run,
    order: state.worldManagerOrder,
    world: world.kind === 'hub' ? {
      kind: 'hub', participant: world.participants[playerId],
      collisionRng: world.collisionRngState,
      students: world.studentPopulation.students.map(student => [student.id, point(student.position),
        student.tick, student.rngState, student.painterRegistration.registrationOrdinal]),
    } : {
      kind: 'boneyard', runId: world.runId, lantern: world.lanternPosition,
      encounter: world.encounter === null ? null : [world.encounter.phase,
        world.encounter.runEventId, world.encounter.voiceTicksRemaining, world.encounter.rngState],
      waves: world.waves === null ? null : [world.waves.phase, world.waves.waveOrdinal,
        world.waves.lowPopulationTicks, world.waves.spawnCountdown, world.waves.rngState],
      tutorial: world.tutorial,
      feedback: world.enemyWorldFeedback,
      score: world.hallOfFameRuns[playerId],
      ouchDeadline: world.playerOuchDeadlineTick,
      actors: enemies.actors.map(actor => [actor.id, actor.config.enemyToken, point(actor.position),
        actor.headingDeg, actor.currentHealth, actor.lifeState, actor.lightRegistration.registrationOrdinal]),
      projectiles: enemies.projectiles.map(actor => [actor.id, actor.kind, point(actor.position),
        actor.ageTicks, registration(actor.painterRegistration)]),
      events: world.enemyEvents.map(event => [event.eventId, event.type, event.actorId,
        event.deflectPitch ?? null, event.targetPlayerId ?? null]),
      counters: [enemies.nextActorId, enemies.nextEventId, enemies.nextProjectileId,
        enemies.nextDeathEffectId, enemies.nextNativeCellBindingOrder, enemies.nextNativeRegistrationOrder],
      enemyRng: [enemies.rngState, enemies.steeringRngState, enemies.locomotionRngState, enemies.headFacingRngState],
      loot: { rng: world.loot.sharedRng, nextActorId: world.loot.nextActorId,
        nextItemId: world.loot.nextItemId,
        actors: world.loot.actors.map(actor => [actor.id, actor.kind, point(actor.position),
          actor.ageTicks, actor.amount, actor.item?.nativeTypeId ?? null]),
        events: world.lootEvents.map(event => [event.eventId, event.type, event.actorId, event.playerId ?? null, event.text ?? null, event.tick]),
      },
      bindings: enemies.targetCellBindings,
    },
  }
}


export function multiplayerSessionSnapshot(state) {
  const ids = state.playerEntities.identities.map(identity => identity.playerId)
  const shared = sessionTickSnapshot(state, ids[0])
  const participants = Object.fromEntries(ids.map(id => {
    const row = sessionTickSnapshot(state, id)
    return [id, Object.fromEntries(['player', 'combat', 'potionClocks', 'runtime', 'character', 'lighting']
      .map(key => [key, row[key]]).concat([[state.world.kind === 'hub' ? 'hub' : 'score',
        row.world.participant ?? row.world.score]]))]
  }))
  for (const key of ['player', 'combat', 'potionClocks', 'runtime', 'character', 'lighting']) delete shared[key]
  delete shared.world.participant
  delete shared.world.score
  return { ids, participants, shared }
}
