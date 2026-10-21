// Exercise the unchanged semantic event projector, including its retained-actor
// fallback and subscription branches. Input projections retain only fields it reads.
import { loadKernel, writeFixture } from './oracle.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const { deriveWebLuaEvents, applyWebLuaCommands } = await loadKernel('host/lua/web-lua-game-api.ts')
const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const config = { displayName: 'Oracle', element: 'ether', discipline: 'arcane' }
const hub = game.createGameSimulation({ zeta: config, alpha: config })
const loaded = { ...NATIVE_GENERATED_BONEYARDS[0],
  choice: { id: 'default-random', name: 'Random Boneyard', source: 'default' },
  seed: '0123456789abcdef', runId: 'derived-event-run' }
const active = game.enterBoneyardWorld(hub, loaded)
const variant = id => id.split('-').map(word => word[0].toUpperCase() + word.slice(1)).join('')

function facts(state) {
  const world = state.world.kind !== 'boneyard' ? null : {
    waves: state.world.waves ? { event_id: state.world.waves.waveEventId,
      ordinal: state.world.waves.waveOrdinal, phase: variant(state.world.waves.phase) } : null,
    last_enemy_event_id: state.world.enemyEvents.at(-1)?.eventId ?? 0,
    enemy_events: state.world.enemyEvents
      .filter(event => event.type === 'enemy-spawned' || event.type === 'enemy-death')
      .map(event => ({ actor_id: event.actorId, event_id: event.eventId,
        kind: event.type === 'enemy-spawned' ? 'Spawned' : 'Death',
        target_player_id: event.targetPlayerId ?? null, tick: event.tick,
        source_position: event.sourcePosition ?? null })),
    actors: state.world.enemies.actors.map(actor => ({ id: actor.id,
      native_type_id: actor.config.nativeTypeId, position: actor.position })),
  }
  return { run_id: state.run.runId, phase: variant(state.run.phase), tick: state.tick, world,
    players: state.playerEntities.identities.map(({ playerId: id }) => ({
      id, gold: game.getPlayerEconomy(state, id).gold,
      level: game.getPlayerProgression(state, id).level,
      experience: game.getPlayerProgression(state, id).experience,
    })) }
}

const cases = []
function record(name, previous, current, includes = null) {
  cases.push({ name, previous: facts(previous), current: facts(current), includes,
    expected: deriveWebLuaEvents(previous, current, id => includes === null || includes.includes(id)) })
}
record('hub-idle', hub, hub)
record('actual-run-entry', hub, active)
record('run-entry-unsubscribed', hub, active, [])
const changed = applyWebLuaCommands(hub, [
  { type: 'set-gold', playerId: 'zeta', value: 321 },
  { type: 'grant-experience', playerId: 'zeta', amount: 900 },
  { type: 'set-gold', playerId: 'alpha', value: 111 },
  { type: 'grant-experience', playerId: 'alpha', amount: 160 },
]).state
record('actual-player-order', hub, changed)
record('only-level-subscription', hub, changed, ['level.up'])
record('only-gold-subscription', hub, changed, ['gold.changed'])
record('ordinary-run-exit', active, hub)
record('terminal-run-exit', { ...active, run: { ...active.run, phase: 'game-over' } }, hub)
record('loadout-run-exit', active, { ...hub, run: { ...hub.run, phase: 'loadout' } })

const actor = (id, nativeTypeId, x, y) => ({ id, config: { nativeTypeId }, position: { x, y } })
const before = { ...active, world: { ...active.world,
  waves: { ...active.world.waves, phase: 'wave-threshold', waveEventId: 5, waveOrdinal: 3 },
  enemies: { ...active.world.enemies, actors: [actor(41, 2001, 5, 6)] },
  enemyEvents: [{ type: 'attack-marker', eventId: 5 }],
} }
const after = { ...active, tick: active.tick + 1, world: { ...active.world,
  waves: { ...active.world.waves, phase: 'wave-lull-delay', waveEventId: 6, waveOrdinal: 4 },
  enemies: { ...active.world.enemies, actors: [actor(42, 2003, 15, 16)] },
  enemyEvents: [
    { type: 'enemy-spawned', eventId: 4, actorId: 0, tick: 0 },
    { type: 'enemy-death', eventId: 6, actorId: 41, tick: 100 },
    { type: 'enemy-spawned', eventId: 7, actorId: 42, tick: 101, targetPlayerId: 'alpha' },
    { type: 'enemy-death', eventId: 8, actorId: 43, tick: 102, sourcePosition: { x: 1.25, y: -7 } },
    { type: 'enemy-spawned', eventId: 9, actorId: 44, tick: 103 },
    { type: 'attack-marker', eventId: 10 },
  ],
} }
record('wave-and-retained-enemy-edges', before, after)
for (const includes of [['enemy.spawned'], ['enemy.death'], ['wave.started'], ['wave.completed'], []]) {
  record(`subscriptions:${includes.join(',')}`, before, after, includes)
}
record('first-wave-from-null', { ...before, world: { ...before.world, waves: null } }, after)
record('unchanged-event-ids', after, after)
writeFixture('derived-mod-events-goldens.json', 'derived-mod-events-goldens.mjs',
  'host/lua/web-lua-game-api.ts::deriveWebLuaEvents', { cases })
