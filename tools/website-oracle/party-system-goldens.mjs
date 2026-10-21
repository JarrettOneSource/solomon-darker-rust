// Replay the public hosted membership API; expected states come from pinned source.
import { loadKernel, writeFixture } from './oracle.mjs'

const source = await loadKernel('host/party-system.ts')
const identity = suffix => ({ id: `party-${suffix}`, joinCode: `CODE-${suffix}`, listingId: `listing-${suffix}` })
let state = source.createPartySystem()
const rows = []
function apply(action, request = {}) {
  let result = null
  let error = null
  try {
    switch (action) {
      case 'reset': state = source.createPartySystem(); break
      case 'register': state = source.registerPartyPlayer(state, request.id, identity(request.identity)); break
      case 'remove': state = source.removePartyPlayer(state, request.id); break
      case 'invite': result = source.invitePartyPlayer(state, request.id, request.target, request.maximum); break
      case 'accept': result = source.acceptPartyInvitation(state, request.id, request.invitation, request.maximum); break
      case 'deny': result = source.denyPartyInvitation(state, request.id, request.invitation); break
      case 'join': result = source.joinPartyPlayer(state, request.id, request.party, request.maximum); break
      case 'leave': result = source.leaveParty(state, request.id, identity(request.identity)); break
      case 'kick': result = source.kickPartyPlayer(state, request.id, request.target, identity(request.identity)); break
      case 'visibility': result = source.setPartyVisibility(state, request.id, request.visibility); break
      case 'effects': result = source.setPartyEnhancedEffects(state, request.id, request.enabled); break
      case 'rotate': result = source.rotatePartyJoinCode(state, request.id, request.code); break
      case 'request': result = source.requestPartyJoin(state, request.party, { id: request.requestId, requester: request.requester }, request.maximum); break
      case 'decide': result = source.decidePartyJoinRequest(state, request.id, request.requestId); break
      case 'clear': state = source.clearPartyInvitations(state, request.party); break
      case 'restore': state = source.restorePartyMembership(state, request.id, request.members, request.leader, request.visibility); break
      default: throw new Error(`Unknown party action ${action}`)
    }
    if (result !== null) state = result.state
  } catch (caught) {
    error = caught.message
  }
  rows.push({ action, request, accepted: error === null && (result?.accepted ?? true),
    reason: result?.reason ?? null, error, expected: state })
}
const invite = (id, target, maximum = 4) => apply('invite', { id, target, maximum })
const register = id => apply('register', { id, identity: id })
const requester = id => ({ accountUsername: null, displayName: `Visitor ${id}`, requesterId: id })
apply('reset')
for (const id of ['zeta', 'alpha', 'omega', 'beta', 'gamma']) register(id)
register('alpha')
invite('missing', 'alpha')
invite('alpha', 'missing')
invite('alpha', 'alpha')
invite('alpha', 'beta', 1)
invite('alpha', 'beta')
invite('alpha', 'beta')
apply('accept', { id: 'omega', invitation: 'invite-1', maximum: 4 })
apply('deny', { id: 'omega', invitation: 'invite-1' })
apply('accept', { id: 'beta', invitation: 'missing', maximum: 4 })
apply('accept', { id: 'beta', invitation: 'invite-1', maximum: 4 })
invite('beta', 'omega')
invite('alpha', 'beta')
invite('omega', 'alpha')
invite('omega', 'gamma')
apply('deny', { id: 'gamma', invitation: 'invite-2' })
apply('deny', { id: 'gamma', invitation: 'invite-2' })
apply('join', { id: 'missing', party: 'party-alpha', maximum: 4 })
apply('join', { id: 'omega', party: 'missing', maximum: 4 })
apply('join', { id: 'alpha', party: 'party-alpha', maximum: 4 })
apply('join', { id: 'alpha', party: 'party-omega', maximum: 4 })
apply('join', { id: 'omega', party: 'party-alpha', maximum: 2 })
invite('alpha', 'omega', 3)
invite('alpha', 'gamma', 3)
apply('request', { party: 'party-alpha', requestId: 'visitor', requester: requester('visitor'), maximum: 4 })
apply('join', { id: 'omega', party: 'party-alpha', maximum: 3 })
apply('visibility', { id: 'missing', visibility: 'private' })
apply('visibility', { id: 'beta', visibility: 'private' })
apply('visibility', { id: 'alpha', visibility: 'public' })
apply('visibility', { id: 'alpha', visibility: 'invite-only' })
apply('rotate', { id: 'beta', code: 'NEW-CODE' })
apply('rotate', { id: 'missing', code: 'NEW-CODE' })
apply('rotate', { id: 'alpha', code: 'CODE-alpha' })
apply('rotate', { id: 'alpha', code: 'NEW-CODE' })
apply('request', { party: 'missing', requestId: 'r0', requester: requester('v0'), maximum: 4 })
apply('request', { party: 'party-alpha', requestId: 'r0', requester: requester('v0'), maximum: 3 })
for (let index = 0; index < 17; index += 1) {
  apply('request', { party: 'party-alpha', requestId: `r${index}`, requester: requester(`v${index}`), maximum: 4 })
}
apply('request', { party: 'party-alpha', requestId: 'duplicate', requester: requester('v0'), maximum: 4 })
apply('decide', { id: 'alpha', requestId: 'missing' })
apply('decide', { id: 'beta', requestId: 'r0' })
apply('decide', { id: 'alpha', requestId: 'r0' })
apply('visibility', { id: 'alpha', visibility: 'private' })
apply('request', { party: 'party-alpha', requestId: 'private', requester: requester('private'), maximum: 4 })
apply('clear', { party: 'missing' })
apply('leave', { id: 'missing', identity: 'missing' })
apply('leave', { id: 'zeta', identity: 'zeta-new' })
apply('kick', { id: 'beta', target: 'omega', identity: 'omega-new' })
apply('kick', { id: 'alpha', target: 'alpha', identity: 'alpha-new' })
apply('kick', { id: 'alpha', target: 'missing', identity: 'missing-new' })
apply('kick', { id: 'missing', target: 'alpha', identity: 'missing-new' })
apply('kick', { id: 'alpha', target: 'beta', identity: 'beta-new' })
apply('leave', { id: 'alpha', identity: 'alpha-new' })
invite('omega', 'zeta')
apply('remove', { id: 'omega' })
apply('remove', { id: 'missing' })
apply('reset')
register('claimant')
register('other')
apply('restore', { id: 'missing', members: ['missing'], leader: 'missing', visibility: 'public' })
for (const members of [[], ['claimant', 'claimant'], ['not-claimant'], ['claimant', 'other']]) {
  apply('restore', { id: 'claimant', members, leader: 'claimant', visibility: 'invite-only' })
}
apply('restore', { id: 'claimant', members: ['claimant'], leader: 'missing', visibility: 'public' })
invite('claimant', 'other')
apply('restore', { id: 'claimant', members: ['claimant', 'offline'], leader: 'offline', visibility: 'invite-only' })
apply('remove', { id: 'offline' })
apply('clear', { party: 'party-claimant' })

apply('reset')
register('alpha')
register('beta')
invite('alpha','beta')
apply('accept',{id:'beta',invitation:'invite-1',maximum:4})
apply('effects',{id:'alpha',enabled:false})
apply('effects',{id:'alpha',enabled:false})
apply('effects',{id:'beta',enabled:true})
apply('effects',{id:'missing',enabled:true})
apply('effects',{id:'alpha',enabled:true})

writeFixture('party-system-goldens.json', 'tools/website-oracle/party-system-goldens.mjs',
  'Website/frontend/src/game/host/party-system.ts public authority API', { rows })
