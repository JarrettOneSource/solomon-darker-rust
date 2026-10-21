// Actual public shared-world operations: no translated host policy or mutated clocks.
import { loadKernel, writeFixture } from './oracle.mjs'
import { multiplayerSessionSnapshot } from './session-tick-rows.mjs'
import { memorialSnapshot } from './hub-memorial-rows.mjs'

const shared = await loadKernel('host/shared-game-worlds.ts')
const game = await loadKernel('core-server/game-simulation.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const character = element => ({ displayName: 'Oracle', element, discipline: 'arcane' })
const identity = suffix => ({ id: `party-${suffix}`, joinCode: `CODE-${suffix}`, listingId: `listing-${suffix}` })
const loaded = runId => ({
  ...NATIVE_GENERATED_BONEYARDS[0],
  choice: { id: 'default-random', name: 'Random Boneyard', source: 'default' },
  runId, seed: '0123456789abcdef',
})
function snapshot(state) {
  const ids = { entityIds: state.playerEntities.entityIds, nextEntityId: state.playerEntities.nextEntityId }
  if (state.playerEntities.identities.length > 0) return { ...multiplayerSessionSnapshot(state), ...ids }
  return { ids: [], tick: state.tick, gameRng: state.gameRng, combatRng: state.combatRng,
    order: state.worldManagerOrder, run: state.run, traderSeed: state.world.traderAnimationSeed, ...ids }
}
function partySnapshot(parties) {
  return { revision: parties.revision, nextInvitationOrdinal: parties.nextInvitationOrdinal,
    groups: parties.parties.map(p => [p.id, p.joinCode, p.listingId, p.leaderPlayerId, p.memberPlayerIds, p.visibility]),
    invitations: parties.invitations.map(p => [p.id, p.inviterPlayerId, p.invitedPlayerId, p.partyId]),
    requests: parties.joinRequests.map(p => [p.id, p.partyId, p.requester]) }
}
function snapshotWorlds() {
  return { hub: snapshot(worlds.hub), parties: partySnapshot(worlds.parties), memorial: memorialSnapshot(worlds.hub.world.memorial),
    runs: worlds.runs.map(run => ({ partyId: run.partyId, runId: run.loadedBoneyard.runId,
      snapshot: snapshot(run.state) })) }
}
let worlds = shared.createSharedGameWorlds(0)
let detached = null
const rows = []
function record(action, request = {}, result = null, extra = null) {
  rows.push({ action, request, accepted: result?.accepted ?? true, reason: result?.reason ?? null,
    expected: snapshotWorlds(), extra })
}
function apply(action, request = {}) {
  let result = null
  let extra = null
  switch (action) {
    case 'add': worlds = shared.addSharedHubPlayer(worlds, request.id, character(request.element), identity(request.id)); break
    case 'invite': result = shared.inviteSharedPartyPlayer(worlds, request.id, request.target, 4); break
    case 'accept': result = shared.acceptSharedPartyInvitation(worlds, request.id, request.invitation, 4); break
    case 'start': result = shared.startSharedPartyRun(worlds, request.id, loaded(request.runId)); break
    case 'step':
      for (let tick = 0; tick < request.count; tick += 1) {
        worlds = shared.stepSharedGameWorlds(worlds, {}, new Set(request.paused))
      }
      break
    case 'save-state': {
      const saved = shared.sharedPartySaveStateForPlayer(worlds, request.id)
      extra = saved === null ? null : snapshot(saved)
      break
    }
    case 'loaded': extra = shared.sharedLoadedBoneyardForPlayer(worlds, request.id)?.runId ?? null; break
    case 'detach': {
      detached = game.detachGameSimulationPlayer(shared.sharedGameStateForPlayer(worlds, request.id), request.id)
      worlds = shared.detachSharedGamePlayer(worlds, request.id)
      break
    }
    case 'rejoin': result = shared.rejoinSharedPartyRunPlayer(worlds, detached, request.id,
      request.partyId, identity(`${request.id}-rejoined`), request.maximum, null); break
    case 'partition': {
      try {
        const selected = game.partitionGameSimulationPlayers(worlds.hub, request.ids)
        extra = { selected: snapshot(selected.selected), remaining: snapshot(selected.remaining) }
      } catch (error) { extra = { error: error.message } }
      break
    }
    case 'damage': {
      const current = shared.sharedGameStateForPlayer(worlds, request.id)
      const result = entities.damagePlayerEntityWithResult(current.playerEntities, request.id, request.amount, current.tick, true, false, 1)
      worlds = shared.replaceSharedGameStateForPlayer(worlds, request.id, { ...current, playerEntities: result.store })
      break
    }
    case 'continue': result = shared.continueSharedPartyGameOver(worlds, request.id, request.runId, request.eventId); break
    case 'confirm': result = shared.confirmSharedPartyLoadout(worlds, request.id, character(request.element)); break
    case 'remove': worlds = shared.removeSharedGamePlayer(worlds, request.id); break
    case 'restore': {
      let state = game.createGameSimulation({ [request.id]: character(request.element) }, { gameRngSeed: 73 })
      const player = entities.playerCharacterAt(state.playerEntities, request.id)
      state = { ...state, playerEntities: entities.replacePlayerCharacter(state.playerEntities, request.id,
        { ...player, position: { x: 1234, y: 567 }, velocity: { x: 4, y: -3 } }) }
      worlds = shared.restoreSharedGamePlayer(worlds, state, null, request.id, identity(request.id))
      break
    }
    default: throw new Error(`Unknown shared-world action ${action}`)
  }
  if (result?.accepted) worlds = result.state
  record(action, request, result, extra)
}
record('create')
for (const [id, element] of [['zeta', 'fire'], ['alpha', 'water'], ['omega', 'ether'], ['beta', 'earth'], ['gamma', 'air']]) {
  apply('add', { id, element })
}
apply('invite', { id: 'zeta', target: 'alpha' })
apply('accept', { id: 'alpha', invitation: 'invite-1' })
apply('invite', { id: 'omega', target: 'beta' })
apply('accept', { id: 'beta', invitation: 'invite-2' })
apply('step', { count: 3, paused: [] })
apply('save-state', { id: 'zeta' })
apply('save-state', { id: 'missing' })
for (const ids of [['alpha', 'zeta'], [], ['zeta', 'zeta'], ['missing']]) apply('partition', { ids })
apply('start', { id: 'alpha', runId: 'not-leader' })
apply('start', { id: 'zeta', runId: 'party-run-a' })
apply('start', { id: 'zeta', runId: 'duplicate' })
apply('start', { id: 'omega', runId: 'party-run-b' })
apply('save-state', { id: 'zeta' })
apply('loaded', { id: 'zeta' })
apply('loaded', { id: 'gamma' })
apply('detach', { id: 'alpha' })
apply('rejoin', { id: 'alpha', partyId: 'party-omega', maximum: 4 })
apply('rejoin', { id: 'alpha', partyId: 'party-zeta', maximum: 4 })
apply('detach', { id: 'alpha' })
apply('remove', { id: 'alpha' })
apply('rejoin', { id: 'alpha', partyId: 'party-zeta', maximum: 1 })
apply('rejoin', { id: 'alpha', partyId: 'party-zeta', maximum: 4 })
apply('step', { count: 4, paused: ['party-zeta'] })
apply('add', { id: 'delta', element: 'fire' })
apply('invite', { id: 'zeta', target: 'delta' })
apply('step', { count: 3, paused: ['party-omega'] })
apply('damage', { id: 'zeta', amount: 10000 })
apply('damage', { id: 'alpha', amount: 10000 })
apply('step', { count: 1, paused: ['party-omega'] })
// Wait for the actual death presentation, then the source Game Over input gate.
while (shared.sharedGameStateForPlayer(worlds, 'zeta').run.phase === 'active') {
  apply('step', { count: 100, paused: ['party-omega'] })
}
apply('continue', { id: 'zeta', runId: 'party-run-a', eventId: 9999 })
apply('step', { count: 500, paused: ['party-omega'] })
const eventId = shared.sharedGameStateForPlayer(worlds, 'zeta').run.gameOverEventId
apply('continue', { id: 'zeta', runId: 'party-run-a', eventId })
apply('step', { count: 20, paused: ['party-omega'] })
apply('confirm', { id: 'zeta', element: 'fire' })
apply('confirm', { id: 'alpha', element: 'water' })
apply('step', { count: 2, paused: ['party-omega'] })
apply('restore', { id: 'traveler', element: 'air' })
apply('remove', { id: 'beta' })
apply('remove', { id: 'omega' })
for (const id of [...worlds.hub.playerEntities.identities.map(row => row.playerId)]) apply('remove', { id })
apply('step', { count: 1, paused: [] })
writeFixture('shared-worlds-goldens.json', 'tools/website-oracle/shared-worlds-goldens.mjs',
  'Website@fa08ae249ff5563570bdb2a653e2ef9358ce7b83/frontend/src/game/host/shared-game-worlds.ts public composition and core-server/game-simulation.ts partition', { loaded: loaded('fixture'), rows })
