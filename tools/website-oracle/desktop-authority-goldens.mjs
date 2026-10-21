// The unchanged current desktop authority entry: shared bootstrap, actual Console,
// save-before-leave, same-process peer ownership and resetWhenEmpty.
import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const require = createRequire(resolve(websiteDir, 'frontend/package.json'))
const { WebSocket } = require('ws')
const { startGameHost } = await loadKernel('host/game-host.ts')
const { encodeGameMessage } = await loadKernel('protocol/game-protocol.ts')
const { GAME_PROTOCOL_VERSION } = await loadKernel('protocol/game-protocol-contract.ts')
const { GameWelcomeReceiver } = await loadKernel('protocol/game-welcome-transfer.ts')
const { GameSaveCheckpointReceiver } = await loadKernel('protocol/game-save-checkpoint-transfer.ts')
const { parseGameSaveDocument } = await loadKernel('save/game-save-contract.ts')
const { gameSimulationPlayerRecords } = await loadKernel('core-server/game-simulation.ts')
const origin = 'solomon-darker://app'
const credential = 'desktop-authority-public-test-credential'
const sockets = []
const rows = []
const host = await startGameHost({ authentication: { kind:'shared', credential },
  host:'127.0.0.1', port:0, allowedOrigins:[origin], resetWhenEmpty:true,
  luaWasmPath:require.resolve('wasmoon/dist/glue.wasm') })
async function waitFor(predicate) {
  const end = Date.now() + 8000
  while (!predicate()) {
    if (Date.now() > end) throw new Error('Source desktop authority observation timed out')
    await new Promise(resolve => setTimeout(resolve, 5))
  }
}
async function connect(name, candidate = credential, saved) {
  const socket = new WebSocket(host.address.url, { origin })
  sockets.push(socket)
  const messages = [], welcome = new GameWelcomeReceiver(), checkpoint = new GameSaveCheckpointReceiver()
  const send = message => socket.send(encodeGameMessage(message))
  socket.on('error', () => {})
  socket.on('message', raw => {
    let message = welcome.receivePayload(raw.toString(), payload => socket.send(payload))
    if (!message) return
    if (message.type === 'server-snapshot' && socket.readyState === WebSocket.OPEN)
      send({ type:'client-snapshot-ack', requireKeyframe:false, sequence:message.sequence })
    if (message.type === 'server-save-checkpoint-chunk') {
      const complete = checkpoint.acceptChunk(message)
      send({ type:'client-save-checkpoint-chunk-ack', sequence:message.sequence, nextOffset:message.offset + message.data.length })
      message = complete
      if (!message) return
    }
    messages.push(message)
  })
  await new Promise((resolve, reject) => { socket.once('open', resolve); socket.once('error', reject) })
  send({ type:'client-hello', protocolVersion:GAME_PROTOCOL_VERSION, credential:candidate,
    character:{ displayName:name, element:'ether', discipline:'arcane' }, enhancedEffects:true,
    profile:{accountUsername:null,highestWave:null,totalPlaytimeMs:null},
    onlinePreferences:{activityMessages:true,globalChat:true,submitRuns:true},cheatsEnabled:false,
    ...(saved ? {save:saved,saveIntent:'resume'} : {}) })
  const next = async accept => {
    await waitFor(() => messages.some(accept))
    return messages.splice(messages.findIndex(accept), 1)[0]
  }
  const close = async () => {
    if (socket.readyState === WebSocket.CLOSED) return
    const closed = new Promise(resolve => socket.once('close', resolve))
    socket.close()
    await closed
  }
  return {socket,messages,send,next,close}
}
async function execute(client, requestId, code) {
  client.send({type:'client-lua-execute',requestId,code})
  const result = await client.next(message => message.type === 'server-lua-result' && message.requestId === requestId)
  return {ok:result.ok,error:result.error,output:result.output,values:result.values}
}
function welcomeRow(message) {
  return {id:message.playerId,kind:message.sessionKind,cheats:message.cheatsEnabled,
    gold:message.snapshot.players[message.playerId].economy.gold,
    players:Object.keys(message.snapshot.players).length}
}
try {
  const invalid = await connect('Rejected', 'wrong')
  const rejected = await invalid.next(message => message.type === 'server-disconnect')
  rows.push({case:'wrong-bootstrap',code:rejected.code})
  await invalid.close()
  const owner = await connect('Owner')
  const ownerWelcome = await owner.next(message => message.type === 'server-welcome')
  rows.push({case:'owner',...welcomeRow(ownerWelcome)})
  const guest = await connect('Guest')
  rows.push({case:'same-bootstrap-admits-peer',...welcomeRow(await guest.next(message => message.type === 'server-welcome'))})
  rows.push({case:'guest-console-rejected',result:await execute(guest,1,'return 17')})
  guest.send({type:'client-cheat-mode',enabled:true})
  rows.push({case:'guest-cheat-mode-is-local',enabled:(await guest.next(message => message.type === 'server-cheat-mode')).enabled})
  rows.push({case:'owner-console',result:await execute(owner,1,'counter = 3; sd.player.set_gold(4321); return counter, sd.player.get_state().display_name')})
  owner.send({type:'client-save-before-leave',requestId:1})
  const ack = await owner.next(message => message.type === 'server-save-before-leave' && message.requestId === 1)
  const saved = await owner.next(message => message.type === 'server-save-checkpoint' && message.sequence === ack.checkpointSequence)
  const parsed = parseGameSaveDocument(saved.save)
  rows.push({case:'durable-source-save-before-leave',integrity:parsed.integrity,
    schemaVersion:parsed.sourceSchemaVersion,gold:parsed.profile.economy.gold,
    worldKind:parsed.continuation.summary.worldKind,playerId:parsed.continuation.summary.playerId})
  await owner.close()
  await waitFor(() => !gameSimulationPlayerRecords(host.state())['player-1'])
  rows.push({case:'next-peer-becomes-console-owner',result:await execute(guest,2,'return counter == nil, 23')})
  await guest.close()
  await waitFor(() => Object.keys(gameSimulationPlayerRecords(host.state())).length === 0)
  const fresh = await connect('Fresh')
  rows.push({case:'last-human-resets-owner',...welcomeRow(await fresh.next(message => message.type === 'server-welcome'))})
  await fresh.close()
  await waitFor(() => Object.keys(gameSimulationPlayerRecords(host.state())).length === 0)
  const resumed = await connect('Owner',credential,saved.save)
  rows.push({case:'source-save-resumes-on-fresh-owner',...welcomeRow(await resumed.next(message => message.type === 'server-welcome'))})
  await resumed.close()
  writeFixture('desktop-authority-goldens.json','tools/website-oracle/desktop-authority-goldens.mjs',
    ['frontend/desktop/local-game-host.mjs','host/run-game-host.ts','host/game-host.ts','save/game-save-contract.ts'],{rows})
} finally {
  for (const socket of sockets) if (socket.readyState !== WebSocket.CLOSED) socket.terminate()
  await host.close()
}
