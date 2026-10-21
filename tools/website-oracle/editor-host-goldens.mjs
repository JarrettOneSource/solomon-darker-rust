// Observable disposable-editor behavior through the unchanged current supervisor.
import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { readFileSync } from 'node:fs'
import { loadKernel, websiteDir, fixturesDir, writeFixture } from './oracle.mjs'
const require = createRequire(resolve(websiteDir, 'frontend/package.json'))
const { WebSocket } = require('ws')
const { startGameSessionSupervisor } = await loadKernel('host/game-session-supervisor.ts')
const { encodeGameMessage } = await loadKernel('protocol/game-protocol.ts')
const { GAME_PROTOCOL_VERSION, GAME_PROTOCOL_NAME } = await loadKernel('protocol/game-protocol-contract.ts')
const { GameWelcomeReceiver } = await loadKernel('protocol/game-welcome-transfer.ts')
const native = JSON.parse(readFileSync(resolve(fixturesDir, 'editor-test-goldens.json'), 'utf8'))
const secret = 'runtime-parity-public-entry-test-secret', origin = 'https://rust-parity.invalid'
const host = await startGameSessionSupervisor({ adminSecret: secret, allowedOrigins: [origin], revision: 'a'.repeat(40) })
const sockets = [], rows = []
const post = async (path, body, authorized = true) => {
  const response = await fetch(host.address.url + path, { method: 'POST', headers: {
    'content-type': 'application/json', ...(authorized ? { authorization: `Bearer ${secret}` } : {}),
  }, body: JSON.stringify(body) })
  return { status: response.status, body: await response.json() }
}
const get = async path => (await fetch(host.address.url + path, { headers: { authorization: `Bearer ${secret}` } })).json()
const wait = async predicate => {
  const deadline = Date.now() + 8000
  while (!predicate()) { if (Date.now() > deadline) throw new Error('Editor Source observation timed out'); await new Promise(resolve => setTimeout(resolve, 5)) }
}
const editorRequest = { name: '  Authored map  ', bytesBase64: native.binaries.spawn }
const provision = () => post('/admin/editor-test', editorRequest)
async function connect(endpoint, extra = {}) {
  const socket = new WebSocket(host.address.url.replace('http:', 'ws:') + endpoint.path, { origin })
  sockets.push(socket)
  const messages = [], receiver = new GameWelcomeReceiver()
  const send = message => socket.send(encodeGameMessage(message))
  socket.on('message', raw => {
    const message = receiver.receivePayload(raw.toString(), payload => socket.send(payload))
    if (!message) return
    messages.push(message)
    if (message.type === 'server-snapshot' && socket.readyState === WebSocket.OPEN)
      send({ type: 'client-snapshot-ack', sequence: message.sequence, requireKeyframe: false })
  })
  socket.on('error', () => {})
  await new Promise((resolve, reject) => { socket.once('open', resolve); socket.once('error', reject) })
  send({ type: 'client-hello', protocolVersion: GAME_PROTOCOL_VERSION, credential: endpoint.credential,
    character: { discipline: 'arcane', element: 'ether', displayName: 'Editor' }, enhancedEffects: true,
    profile: { accountUsername: null, highestWave: null, totalPlaytimeMs: null },
    onlinePreferences: { activityMessages: true, globalChat: true, submitRuns: true }, cheatsEnabled: false, ...extra })
  return { socket, messages, send, next: async accept => { await wait(() => messages.some(accept)); return messages.find(accept) } }
}
async function close(client) {
  if (client.socket.readyState === WebSocket.CLOSED) return
  const closed = new Promise(resolve => client.socket.once('close', resolve))
  client.socket.close(); await closed
}
try {
  rows.push({ case: 'unauthorized', ...await post('/admin/editor-test', {}, false) })
  for (const [name, body] of [['null', null], ['missing', {}], ['encoding', { name: 'Broken', bytesBase64: '!!!!' }]])
    rows.push({ case: `reject-${name}`, ...await post('/admin/editor-test', body), sessions: host.sessionCount() })
  const endpoint = await provision()
  rows.push({ case: 'endpoint', status: endpoint.status, kind: endpoint.body.sessionKind, protocolRecognized: endpoint.body.protocol === GAME_PROTOCOL_NAME })
  const client = await connect(endpoint.body)
  const welcome = await client.next(message => message.type === 'server-welcome')
  rows.push({ case: 'welcome', kind: welcome.sessionKind, choices: welcome.boneyards })
  for (const [name, extra] of [['save', { save: 'not a save', saveIntent: 'resume' }],
    ['resume', { resumeToken: 'existing-session-resume-token' }], ['intro', { beginCollegeIntro: true }]]) {
    const endpoint = await provision(), rejected = await connect(endpoint.body, extra)
    const message = await rejected.next(message => message.type === 'server-disconnect')
    rows.push({ case: `reject-${name}`, code: message.code, reason: message.reason })
    await close(rejected)
  }
  for (const id of ['default-random', 'stock-tutorial', 'unknown-editor-map']) {
    const endpoint = await provision(), rejected = await connect(endpoint.body)
    await rejected.next(message => message.type === 'server-welcome')
    rejected.send({ type: 'client-start-match', boneyardId: id })
    const message = await rejected.next(message => message.type === 'server-disconnect')
    rows.push({ case: `reject-match-${id}`, code: message.code, reason: message.reason })
    await close(rejected)
  }
  client.send({ type: 'client-start-tutorial' })
  client.send({ type: 'client-save-before-leave', requestId: 1 })
  const firstSave = await client.next(message => message.type === 'server-save-before-leave' && message.requestId === 1)
  rows.push({ case: 'hub-save-suppressed', sequence: firstSave.checkpointSequence })
  client.send({ type: 'client-start-match', boneyardId: 'editor-test' })
  const loaded = (await client.next(message => message.type === 'server-boneyard-loaded')).boneyard
  const grace = await client.next(message => message.type === 'server-gameplay-resume-grace' && message.grace)
  client.send({ type: 'client-ready-boneyard', runId: loaded.runId })
  client.send({ type: 'client-resume-grace-ready', sequence: grace.grace.sequence })
  const initial = (await client.next(message => message.type === 'server-snapshot' && message.frame.world.kind === 'boneyard')).frame.tick
  await client.next(message => message.type === 'server-snapshot' && message.frame.tick >= initial + 100)
  rows.push({ case: 'authored-world', choice: loaded.choice.id, name: loaded.choice.name, spawn: loaded.scene.spawn,
    geometrySha256: loaded.geometrySha256, advancedAtLeast100Ticks: true })
  client.send({ type: 'client-save-before-leave', requestId: 2 })
  const secondSave = await client.next(message => message.type === 'server-save-before-leave' && message.requestId === 2)
  rows.push({ case: 'run-save-suppressed', sequence: secondSave.checkpointSequence,
    checkpointPublished: client.messages.some(message => message.type.startsWith('server-save-checkpoint')) })
  for (const path of ['/admin/presence', '/admin/hub/parties', '/admin/matches'])
    rows.push({ case: path, body: await get(path) })
  const normalEndpoint = await post('/admin/sessions', { content: { manifestSha256: '0'.repeat(64), mods: [] } })
  const normal = await connect(normalEndpoint.body)
  const normalWelcome = await normal.next(message => message.type === 'server-welcome')
  await normal.next(message => message.type === 'server-save-checkpoint' || message.type === 'server-save-checkpoint-chunk')
  rows.push({ case: 'ordinary-college-unchanged', kind: normalWelcome.sessionKind,
    randomChoice: normalWelcome.boneyards.some(choice => choice.id === 'default-random'), checkpointPublished: true })
  await close(normal)
  // Failed Hello never creates a wizard, so these three owners remain unclaimed
  // until the ordinary ticket timeout; closing an admitted preview reaps it.
  await wait(() => host.sessionCount() === 4)
  rows.push({ case: 'pre-wizard-rejections-retain-unclaimed-owners', sessions: host.sessionCount() })
  rows.push({ case: 'deployment-omits-disposable-wizard', ...await post('/admin/deployments/restart', { targetRevision: 'b'.repeat(40) }) })
  writeFixture('editor-host-goldens.json', 'editor-host-goldens.mjs',
    'current game-session-supervisor.ts + game-host.ts public authenticated editor-test HTTP and live sockets', { rows })
} finally { for (const socket of sockets) socket.close(); await host.close() }
