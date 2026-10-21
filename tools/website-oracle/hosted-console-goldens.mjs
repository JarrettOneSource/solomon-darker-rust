// Actual supervisor sockets, persistent Console VM and deployed policy worker.
import { createRequire } from 'node:module'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const require = createRequire(resolve(websiteDir, 'frontend/package.json'))
const { WebSocket } = require('ws')
const { startGameSessionSupervisor } = await loadKernel('host/game-session-supervisor.ts')
const { MlBotPolicyInferenceWorker } = await loadKernel('host/ml-bot-host-controller.ts')
const { encodeGameMessage } = await loadKernel('protocol/game-protocol.ts')
const { GAME_PROTOCOL_VERSION } = await loadKernel('protocol/game-protocol-contract.ts')
const { GameWelcomeReceiver } = await loadKernel('protocol/game-welcome-transfer.ts')
const policy = await MlBotPolicyInferenceWorker.create(readFileSync(resolve(
  websiteDir, 'frontend/server-assets/ml-bot-policy-v7-selected.sdml',
)))
const secret = 'runtime-parity-public-entry-test-secret'
const origin = 'https://rust-parity.invalid'
const content = {manifestSha256:'0'.repeat(64),mods:[]}
const host = await startGameSessionSupervisor({
  adminSecret:secret,allowedOrigins:[origin],revision:'a'.repeat(40),mlBotPolicy:policy,
  luaWasmPath:require.resolve('wasmoon/dist/glue.wasm'),
})
const sockets = []
async function request(method,path,body) {
  const response = await fetch(host.address.url+path, {
    method,headers:{authorization:`Bearer ${secret}`,'content-type':'application/json'},
    ...(method === 'GET' ? {} : {body:JSON.stringify(body)}),
  })
  const result = await response.json()
  if (!response.ok) throw new Error(`${path}: ${JSON.stringify(result)}`)
  return result
}
async function join(path,name,developerAccess = false) {
  const endpoint = await request('POST',path,{content,developerAccess})
  const socket = new WebSocket(host.address.url.replace('http:','ws:')+endpoint.path,{origin})
  sockets.push(socket)
  const messages = []
  const welcomeReceiver = new GameWelcomeReceiver()
  let waiting
  const send = message => socket.send(encodeGameMessage(message))
  socket.on('message',data => {
    const message = welcomeReceiver.receivePayload(data.toString(), payload => socket.send(payload))
    if (!message) return
    if (message.type === 'server-snapshot' && socket.readyState === WebSocket.OPEN) {
      send({type:'client-snapshot-ack',requireKeyframe:false,sequence:message.sequence})
    }
    messages.push(message)
    if (waiting) {
      const index = messages.findIndex(waiting.accept)
      if (index >= 0) {
        const done = waiting
        waiting = undefined
        clearTimeout(done.timer)
        done.resolve(messages.splice(index,1)[0])
      }
    }
  })
  socket.on('error',()=>{})
  await new Promise((resolve,reject) => {socket.once('open',resolve);socket.once('error',reject)})
  const next = accept => {
    const index = messages.findIndex(accept)
    if (index >= 0) return Promise.resolve(messages.splice(index,1)[0])
    return new Promise((resolve,reject) => {
      waiting = {accept,resolve,timer:setTimeout(()=>reject(new Error(
        `Source Console timeout: ${messages.map(row=>row.type).join(',')}`,
      )),8000)}
    })
  }
  send({type:'client-hello',protocolVersion:GAME_PROTOCOL_VERSION,credential:endpoint.credential,enhancedEffects:true,
    character:{displayName:name,element:'ether',discipline:'arcane'},
    profile:{accountUsername:null,highestWave:null,totalPlaytimeMs:null},
    onlinePreferences:{activityMessages:true,globalChat:true,submitRuns:true},cheatsEnabled:false})
  const welcome = await next(message=>message.type==='server-welcome')
  return {socket,send,next,welcome}
}
async function execute(client,requestId,code) {
  client.send({type:'client-lua-execute',requestId,code})
  const result = await client.next(message=>message.type==='server-lua-result' && message.requestId===requestId)
  return {error:result.error,ok:result.ok,output:result.output,values:result.values}
}
const rows = []
try {
  const guest = await join('/admin/hub/tickets','Guest')
  rows.push({case:'global-ordinary-lua-rejected',result:await execute(guest,1,'return 17')})
  const privateOwner = await join('/admin/sessions','Private owner')
  rows.push({case:'private-authority-with-cheats-off',result:await execute(privateOwner,1,'return 17, "actual Lua"')})
  rows.push({case:'persistent-console-globals',result:await execute(privateOwner,2,'counter = (counter or 0) + 1; return counter')})
  rows.push({case:'persistent-console-globals-next-request',result:await execute(privateOwner,3,'counter = counter + 1; return counter')})
  const developer = await join('/admin/hub/tickets','Developer',true)
  const summoned = await execute(developer,1,'return sd.bots.summon({element="water",discipline="mind"})')
  if (!summoned.ok) throw new Error(JSON.stringify(summoned))
  const bot = summoned.values[0]
  rows.push({case:'actual-bot-reservation',name:bot.display_name,opaqueId:/^bot-[\w-]{16}$/.test(bot.player_id)})
  const bots = await developer.next(message=>message.type==='server-party-state'
    && message.state.hubPlayers.some(row=>row.playerId===bot.player_id))
  rows.push({case:'admitted-bot-roster',names:bots.state.hubPlayers.map(row=>row.displayName).sort()})
  const health = await request('GET','/health')
  if (health.protocol !== `solomon-dark/${GAME_PROTOCOL_VERSION}`)
    throw new Error(`Unexpected Source health protocol: ${health.protocol}`)
  delete health.protocol
  rows.push({case:'live-bot-health',health})
  developer.send({type:'client-party-invite',targetPlayerId:bot.player_id})
  const before = performance.now()
  await developer.next(message=>message.type==='server-party-state'
    && message.state.party.memberPlayerIds.length===2)
  const presence = await request('GET','/admin/presence')
  const actual = presence.items.find(row=>row.bot)
  rows.push({case:'delayed-bot-invitation',waitedAtLeastThreeSeconds:performance.now()-before>=2900,
    bot:actual.bot,developer:actual.developer,displayName:actual.displayName,
    partyLeader:actual.partyLeader,partySize:actual.partySize,activity:actual.activity})
  const secondDeveloper = await join('/admin/hub/tickets','Other developer',true)
  rows.push({case:'one-console-owner-per-authority',result:await execute(secondDeveloper,1,'return 23')})
  developer.socket.close()
  await new Promise(resolve=>developer.socket.once('close',resolve))
  const closedAt = performance.now()
  while ((await request('GET','/admin/presence')).items.some(row=>row.displayName==='Developer')) {
    if (performance.now()-closedAt>8000) throw new Error('Source owner close was not observed')
    await new Promise(resolve=>setTimeout(resolve,10))
  }
  rows.push({case:'new-console-owner-after-disconnect',result:await execute(secondDeveloper,2,'return counter == nil, 23')})
  guest.socket.close()
  secondDeveloper.socket.close()
  let after
  const emptiedAt = performance.now()
  do {
    after = await request('GET','/admin/presence')
    if (performance.now()-emptiedAt>8000) throw new Error('Source shared humans did not disconnect')
    if (after.items.some(row=>row.session==='global-hub' && !row.bot)) {
      await new Promise(resolve=>setTimeout(resolve,10))
    }
  } while (after.items.some(row=>row.session==='global-hub' && !row.bot))
  rows.push({case:'last-shared-human-removes-bots',bots:after.items.filter(row=>row.bot).length})
  writeFixture('hosted-console-goldens.json','tools/website-oracle/hosted-console-goldens.mjs',
    'host/game-session-supervisor.ts; game-host.ts Console and learned-bot lifecycle',{rows})
  console.log(`${rows.length} actual Source Console/bot observations`)
} finally {
  for (const socket of sockets) socket.close()
  await host.close()
  await policy.close()
}
