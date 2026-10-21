// Public current Source shutdown and heartbeat callbacks, with continuously read sockets.
import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const require = createRequire(resolve(websiteDir, 'frontend/package.json'))
const { WebSocket } = require('ws')
const { startGameSessionSupervisor } = await loadKernel('host/game-session-supervisor.ts')
const { encodeGameMessage } = await loadKernel('protocol/game-protocol.ts')
const { GAME_PROTOCOL_VERSION } = await loadKernel('protocol/game-protocol-contract.ts')
const { GameWelcomeReceiver } = await loadKernel('protocol/game-welcome-transfer.ts')
const { GameSaveCheckpointReceiver } = await loadKernel('protocol/game-save-checkpoint-transfer.ts')
const secret = 'runtime-parity-public-entry-test-secret', origin = 'https://rust-parity.invalid'
const content = { manifestSha256: '0'.repeat(64), mods: [] }
const entries = [], aliases = new Map(), sockets = []
const host = await startGameSessionSupervisor({ adminSecret: secret, allowedOrigins: [origin],
  revision: 'a'.repeat(40), heartbeatIntervalMs: 50, runtimeEvents: entry => entries.push(entry) })
async function until(predicate) {
  const end = Date.now() + 8000
  while (!predicate()) {
    if (Date.now() > end) throw new Error('Source operator shutdown observation timed out')
    await new Promise(resolve => setTimeout(resolve, 5))
  }
}
async function join(path, name, autoPong = true) {
  const response = await fetch(host.address.url + path, { method: 'POST',
    headers: { authorization: `Bearer ${secret}`, 'content-type': 'application/json' },
    body: JSON.stringify({ content }) })
  if (!response.ok) throw new Error(`Source admission returned ${response.status}`)
  const endpoint = await response.json()
  const client = await connect(endpoint, { type: 'client-hello', protocolVersion: GAME_PROTOCOL_VERSION, credential: endpoint.credential,
    enhancedEffects: true, character: { displayName: name, element: 'ether', discipline: 'arcane' },
    profile: { accountUsername: null, highestWave: null, totalPlaytimeMs: null },
    onlinePreferences: { activityMessages: true, globalChat: true, submitRuns: true }, cheatsEnabled: false }, autoPong)
  const id = client.messages.find(message => message.type === 'server-welcome').playerId
  aliases.set(id, `player:${name}`)
  if (path === '/admin/sessions') {
    const row = entries.findLast(entry => entry.event === 'session.provisioned')
    aliases.set(row.details.sessionId, 'private:one')
  }
  return client
}
async function connect(endpoint, hello, autoPong = true) {
  const socket = new WebSocket(host.address.url.replace('http:', 'ws:') + endpoint.path, { origin, autoPong })
  sockets.push(socket)
  const welcome = new GameWelcomeReceiver(), save = new GameSaveCheckpointReceiver(), messages = []
  const send = message => socket.send(encodeGameMessage(message))
  socket.on('message', raw => {
    const message = welcome.receivePayload(raw.toString(), payload => socket.send(payload))
    if (!message) return
    if (message.type === 'server-snapshot' && socket.readyState === WebSocket.OPEN)
      send({ type: 'client-snapshot-ack', requireKeyframe: false, sequence: message.sequence })
    if (message.type === 'server-save-checkpoint-chunk') {
      save.acceptChunk(message)
      send({ type: 'client-save-checkpoint-chunk-ack', sequence: message.sequence, nextOffset: message.offset + message.data.length })
    }
    messages.push(message)
  })
  socket.on('error', () => {})
  await new Promise((resolve, reject) => { socket.once('open', resolve); socket.once('error', reject) })
  send(hello)
  await until(() => messages.some(message => message.type === 'server-welcome'))
  return { socket, send, messages }
}

function normalize(value, key = '') {
  if (key === 'durationMs') return { nonnegative: Number.isFinite(value) && value >= 0 }
  if (key === 'serverTick') return { nonnegativeInteger: Number.isInteger(value) && value >= 0 }
  if (typeof value === 'string') return aliases.get(value) ?? value
  if (Array.isArray(value)) return value.map(item => normalize(item))
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([name, item]) => [name, normalize(item, name)]))
  return value
}
try {
  await join('/admin/hub/tickets', 'Heartbeat', false)
  await until(() => entries.some(entry => entry.event === 'player.disconnected' && entry.details.displayName === 'Heartbeat'))
  const heartbeat = entries.find(entry => entry.event === 'player.disconnected' && entry.details.displayName === 'Heartbeat')
  await join('/admin/hub/tickets', 'Shutdown Hub')
  const college = await join('/admin/sessions', 'Shutdown Private')
  college.send({ type: 'client-start-match', boneyardId: 'default-random' })
  await until(() => college.messages.some(message => message.type === 'server-boneyard-loaded'))
  const loaded = college.messages.find(message => message.type === 'server-boneyard-loaded').boneyard
  aliases.set(loaded.runId, 'run:private')
  const matches = await (await fetch(host.address.url + '/admin/matches', { headers: { authorization: `Bearer ${secret}` } })).json()
  const match = matches.items.find(row => row.players.includes('Shutdown Private'))
  const response = await fetch(host.address.url + '/admin/observers', { method: 'POST',
    headers: { authorization: `Bearer ${secret}`, 'content-type': 'application/json' },
    body: JSON.stringify({ matchId: match.id, observer: { userId: 71, username: 'Shutdown auditor' } }) })
  if (response.status !== 201) throw new Error(`Source observer admission returned ${response.status}`)
  const endpoint = await response.json()
  await connect(endpoint, { type: 'client-observer-hello', protocolVersion: GAME_PROTOCOL_VERSION, credential: endpoint.credential })
  aliases.set(entries.findLast(entry => entry.event === 'observer.connected').details.observerId, 'observer:one')
  const offset = entries.length
  await host.close()
  await until(() => entries.slice(offset).filter(entry => ['player.disconnected', 'observer.disconnected'].includes(entry.event)).length === 3)
  const project = ({ component, event, message, details }) => ({ component, event, message, details: normalize(details) })
  writeFixture('hosted-shutdown-goldens.json', 'hosted-shutdown-goldens.mjs',
    'current public supervisor close callbacks plus a real non-ponging proxy client; opaque IDs and wall observations normalized',
    { heartbeat: project(heartbeat), shutdown: entries.slice(offset).filter(entry => ['player.disconnected', 'observer.disconnected', 'session.closed'].includes(entry.event)).map(project) })
} finally {
  for (const socket of sockets) socket.close()
  await host.close()
}
