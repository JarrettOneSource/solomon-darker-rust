// Actual broker public API, with clocks and random invitation IDs normalized at the edge.
import { loadKernel, writeFixture } from './oracle.mjs'

const { startGameSocialBroker } = await loadKernel('host/game-social-broker.ts')
const broker = startGameSocialBroker()
const connections = new Map()
const facts = new Map()
const ids = new Map()
const aliases = new Map()
const rows = []
let deliveries = []
let now = 1_000
const originalNow = Date.now
Date.now = () => now
const reference = key => `player-ref-${key.repeat(32)}`
const preferences = { activityMessages: true, globalChat: true, submitRuns: true }
function normalize(value) {
  if (typeof value === 'string' && value.startsWith('college-invite-')) {
    if (!aliases.has(value)) {
      const alias = `invitation-${aliases.size + 1}`
      aliases.set(value, alias)
      ids.set(alias, value)
    }
    return aliases.get(value)
  }
  if (Array.isArray(value)) return value.map(normalize)
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([key, entry]) => [key, normalize(entry)]))
  }
  return value
}
function profile(who) {
  const fact = facts.get(who)
  if (fact.profileNull) return null
  return {
    accountUsername: `account-${who}`, activity: fact.available ? 'hub' : 'boneyard',
    discipline: 'arcane', displayName: fact.name, element: 'fire', gold: 37,
    highestWave: 4, playerReference: fact.badReference ? reference('z') : reference(who),
    sessionKind: fact.host === 'college-a' ? 'global-hub' : 'private-college', totalPlaytimeMs: 9_000,
  }
}
function apply(action, input = {}) {
  let result = null
  let error = null
  try {
    const current = connections.get(input.connection)
    switch (action) {
      case 'register': {
        const fact = facts.get(input.who) ?? {
          host: input.host, local: input.local, available: true, name: `Wizard ${input.who}`,
        }
        facts.set(input.who, fact)
        const connection = broker.register({
          hostId: input.host, localPlayerId: input.local,
          canReceiveCollegeInvitation: () => facts.get(input.who).available,
          deliverChat: message => deliveries.push({ connection: input.connection, chat: message }),
          deliverCollegeInvitations: invitations => deliveries.push({ connection: input.connection, invitations }),
          profile: () => profile(input.who),
        }, { ...preferences }, input.reference ?? reference(input.who))
        connections.set(input.connection, connection)
        break
      }
      case 'activate': current.activate(); break
      case 'close': current.close(); break
      case 'global': result = current.publishGlobal(input.text); break
      case 'whisper': result = current.publishWhisper(reference(input.target), input.text); break
      case 'activity': current.publishActivity(input.activity); break
      case 'preferences': current.setOnlinePreferences(input.preferences); break
      case 'invite': result = current.inviteToCollege(reference(input.target), input.party, input.code); break
      case 'dismiss': current.dismissCollegeInvitation(ids.get(input.invitation) ?? input.invitation); break
      case 'revoke': current.revokeCollegeInvitations(input.party); break
      case 'refresh': current.refreshCollegeInvitationAvailability(); break
      case 'card': result = current.resolvePlayerCard(reference(input.target)); break
      case 'facts': Object.assign(facts.get(input.who), input.patch); break
      case 'prune': now = input.now; broker.prune(now); break
      case 'shutdown': broker.close(); break
      default: throw new Error(`Unknown broker operation ${action}`)
    }
  } catch (caught) { error = caught.message }
  rows.push({ action, input, result: normalize(result), error, deliveries: normalize(deliveries) })
  deliveries = []
}
const act = (action, connection, input = {}) => apply(action, { connection, ...input })
try {
  for (const [who, host, local] of [
    ['a', 'college-a', 'player-1'], ['b', 'college-a', 'player-2'],
    ['c', 'college-b', 'player-1'], ['d', 'college-b', 'player-2'],
  ]) apply('register', { connection: who, who, host, local })
  for (const id of ['a', 'b', 'c']) act('activate', id)
  act('activate', 'a')
  act('global', 'a', { text: 'Across both Colleges' })
  act('whisper', 'a', { target: 'c', text: 'Different local player-1' })
  act('whisper', 'a', { target: 'a', text: 'self' })
  act('whisper', 'a', { target: 'd', text: 'inactive' })
  act('activity', 'a', { activity: 'entered-college' })
  act('preferences', 'b', { preferences: { ...preferences, activityMessages: false } })
  act('activity', 'a', { activity: 'searching-solomon' })
  act('preferences', 'c', { preferences: { ...preferences, globalChat: false } })
  act('global', 'a', { text: 'Opted out recipient is absent' })
  act('global', 'c', { text: 'Opted out sender' })
  act('card', 'a', { target: 'c' })
  act('invite', 'a', { target: 'a', party: 'party-a', code: 'AAAA-BBBB' })
  act('invite', 'a', { target: 'd', party: 'party-a', code: 'AAAA-BBBB' })
  act('invite', 'a', { target: 'c', party: 'party-a', code: 'AAAA-BBBB' })
  act('preferences', 'c', { preferences })
  apply('facts', { who: 'c', patch: { available: false } })
  act('invite', 'a', { target: 'c', party: 'party-a', code: 'AAAA-BBBB' })
  apply('facts', { who: 'c', patch: { available: true } })
  act('invite', 'a', { target: 'c', party: 'party-a', code: 'AAAA-BBBB' })
  act('invite', 'b', { target: 'c', party: 'party-a', code: 'AAAA-BBBB' })
  for (let index = 1; index < 9; index += 1) {
    act('invite', 'a', { target: 'c', party: `party-${index}`, code: `CODE-${index}` })
  }
  act('dismiss', 'a', { invitation: 'invitation-1' })
  act('dismiss', 'c', { invitation: 'invitation-3' })
  act('revoke', 'b', { party: 'party-a' })
  act('invite', 'b', { target: 'c', party: 'party-b', code: 'CCCC-DDDD' })
  apply('prune', { now: 600_999 })
  apply('prune', { now: 601_000 })
  act('invite', 'a', { target: 'c', party: 'party-a', code: 'EEEE-FFFF' })
  apply('facts', { who: 'c', patch: { available: false } })
  act('refresh', 'c')
  apply('facts', { who: 'c', patch: { available: true } })
  act('refresh', 'c')
  act('invite', 'a', { target: 'c', party: 'party-a', code: 'EEEE-FFFF' })
  act('preferences', 'a', { preferences: { ...preferences, globalChat: false } })
  act('preferences', 'a', { preferences })
  act('invite', 'b', { target: 'a', party: 'party-b', code: 'CCCC-DDDD' })
  apply('register', { connection: 'a2', who: 'a', host: 'college-a', local: 'player-1' })
  act('close', 'a')
  act('activate', 'a2')
  act('global', 'a', { text: 'stale generation' })
  act('global', 'a2', { text: 'replacement generation' })
  apply('facts', { who: 'a', patch: { name: 'Renamed Wizard' } })
  act('card', 'c', { target: 'a' })
  apply('facts', { who: 'a', patch: { profileNull: true } })
  act('card', 'c', { target: 'a' })
  act('global', 'a2', { text: 'Missing profile falls back to local identity' })
  apply('facts', { who: 'a', patch: { profileNull: false, badReference: true } })
  act('card', 'c', { target: 'a' })
  apply('facts', { who: 'a', patch: { badReference: false } })
  apply('register', { connection: 'bad', who: 'x', host: 'college-b', local: 'x', reference: 'invalid' })
  apply('register', { connection: 'taken', who: 'x', host: 'college-b', local: 'x', reference: reference('a') })
  act('close', 'b')
  act('close', 'b')
  act('whisper', 'c', { target: 'b', text: 'closed' })
  act('activate', 'd')
  act('invite', 'c', { target: 'd', party: 'party-c', code: 'HHHH-JJJJ' })
  act('close', 'c')
  apply('shutdown')
  act('global', 'a2', { text: 'closed broker' })
  apply('register', { connection: 'late', who: 'z', host: 'college-a', local: 'late' })
} finally { Date.now = originalNow; broker.close() }
writeFixture('social-broker-goldens.json', 'tools/website-oracle/social-broker-goldens.mjs',
  'Website/frontend/src/game/host/game-social-broker.ts complete public connection API', { rows })
