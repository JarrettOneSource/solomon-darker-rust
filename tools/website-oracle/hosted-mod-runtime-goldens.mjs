// Exercise the unchanged public supervisor, real definition VM and prepared owner.
import { createRequire } from 'node:module'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const require = createRequire(resolve(websiteDir, 'frontend/package.json'))
const { WebSocket } = require('ws')
const { startGameSessionSupervisor } = await loadKernel('host/game-session-supervisor.ts')
const { encodeGameMessage, decodeServerGameMessage } = await loadKernel('protocol/game-protocol.ts')
const source = JSON.parse(readFileSync(new URL(
  '../../crates/sim/tests/fixtures/website/mod-host-runtime-goldens.json', import.meta.url,
)))
const payload = source.payload
payload.mods[0].entryScript = payload.mods[0].entryScript.replace('return sd.mod({', `
local terminal = sd.rules.on("action.shop.purchase", sd.effect.damage({target = "player-1", amount = 10000}))
return sd.mod({`).replace('rules = {spawn_content, probe, remember_purchase}',
  'rules = {spawn_content, probe, remember_purchase, terminal}')
const secret = 'runtime-parity-public-entry-test-secret'
const origin = 'https://rust-parity.invalid'
const host = await startGameSessionSupervisor({
  adminSecret: secret,
  allowedOrigins: [origin],
  revision: 'a'.repeat(40),
  luaWasmPath: require.resolve('wasmoon/dist/glue.wasm'),
})
let socket
try {
  const response = await fetch(`${host.address.url}/admin/sessions`, {
    method: 'POST',
    headers: { authorization: `Bearer ${secret}`, 'content-type': 'application/json' },
    body: JSON.stringify({ content: payload }),
  })
  const endpoint = await response.json()
  if (response.status !== 201) throw new Error(JSON.stringify(endpoint))
  socket = new WebSocket(host.address.url.replace('http:', 'ws:') + endpoint.path, { origin })
  const messages = []
  let waiting
  function send(message) { socket.send(encodeGameMessage(message)) }
  socket.on('message', data => {
    const message = decodeServerGameMessage(data.toString())
    if (message.type === 'server-snapshot') {
      send({ type: 'client-snapshot-ack', requireKeyframe: false, sequence: message.sequence })
    }
    messages.push(message)
    if (waiting) {
      const index = messages.findIndex(waiting.accept)
      if (index >= 0) {
        const completed = waiting
        waiting = undefined
        clearTimeout(completed.timer)
        completed.resolve(messages.splice(index, 1)[0])
      }
    }
  })
  socket.on('error', () => {})
  await new Promise((resolve, reject) => {
    socket.once('open', resolve)
    socket.once('error', reject)
  })
  function next(accept) {
    const index = messages.findIndex(accept)
    if (index >= 0) return Promise.resolve(messages.splice(index, 1)[0])
    return new Promise((resolve, reject) => {
      waiting = { accept, resolve, timer: setTimeout(() => reject(new Error(
        `Source prepared host timeout: ${messages.map(message => message.type).join(', ')}`,
      )), 8000) }
    })
  }
  send({
    type: 'client-hello', protocolVersion: 129, credential: endpoint.credential,
    character: { displayName: 'Authored Runtime', element: 'ether', discipline: 'arcane' },
    profile: { accountUsername: null, highestWave: null, totalPlaytimeMs: null },
    onlinePreferences: { activityMessages: true, globalChat: true, submitRuns: true },
    cheatsEnabled: false,
  })
  const welcome = await next(message => message.type === 'server-welcome')
  const initial = await next(message => message.type === 'server-mod-content')
  const rows = [{
    case: 'prepared-private-welcome',
    catalogIds: welcome.modCatalog.map(item => item.content.contentId),
    contentCount: initial.content.length,
    powerups: initial.powerups,
  }]
  send({ type: 'client-start-match', boneyardId: 'default-random' })
  const loaded = (await next(message => message.type === 'server-boneyard-loaded')).boneyard
  const grace = await next(message => message.type === 'server-gameplay-resume-grace' && message.grace)
  send({ type: 'client-ready-boneyard', runId: loaded.runId })
  send({ type: 'client-resume-grace-ready', sequence: grace.grace.sequence })
  const runtime = await next(message => message.type === 'server-mod-runtime'
    && message.projection.powerup_actors.length === 1)
  // This public socket test has a real wall clock. Retain its observed birth tick
  // below, while comparing its ordering against the later saved authoritative tick.
  // Exact controlled-tick values are covered by mod-host-game-goldens.
  const projectedPowerups = runtime.projection.powerup_actors.map(({ spawned_tick, ...row }) => row)
  rows.push({ case: 'actual-run-start-rule', powerups: projectedPowerups })
  send({ type: 'client-save-before-leave', requestId: 1 })
  const saved = await next(message => message.type === 'server-save-checkpoint'
    && JSON.parse(message.save).continuation?.summary.worldKind === 'boneyard'
    && Object.values(JSON.parse(message.save).modState ?? {}).some(state => state.powerups.length === 1))
  const document = JSON.parse(saved.save)
  const savedTick = document.continuation.summary.savedAtTick
  const modState = JSON.parse(JSON.stringify(document.modState))
  for (const state of Object.values(modState)) {
    for (const powerup of state.powerups) {
      if (powerup.spawned_tick > savedTick) throw new Error('Source saved a future powerup')
      delete powerup.spawned_tick
    }
  }
  rows.push({ case: 'actual-prepared-save', modState,
    birthNotAfterSave: runtime.projection.powerup_actors.every(powerup => powerup.spawned_tick <= savedTick) })
  send({ type: 'client-mod-action', action: 'shop-buy', arguments: { row: 0 },
    requestId: 1, target: runtime.projection.shops[0].content_id })
  await next(message => message.type === 'server-save-checkpoint'
    && message.reason === 'game-over' && JSON.parse(message.save).continuation === null)
  send({ type: 'client-save-before-leave', requestId: 2 })
  const receipt = await next(message => message.type === 'server-save-before-leave' && message.requestId === 2)
  const terminal = await next(message => message.type === 'server-save-checkpoint'
    && message.sequence === receipt.checkpointSequence)
  rows.push({ case: 'explicit-terminal-leave', nonzero: receipt.checkpointSequence > 0,
    reason: terminal.reason, profileOnly: JSON.parse(terminal.save).continuation === null })
  writeFixture('hosted-mod-runtime-goldens.json', 'hosted-mod-runtime-goldens.mjs',
    'host/game-session-supervisor.ts and game-host.ts prepared tick/publication/save',
    { payload, observed: { powerups: runtime.projection.powerup_actors, savedTick }, rows })
} finally {
  socket?.close()
  await host.close()
}
