// Real current Website supervisor/WS admission, party mutations and close callbacks.
// Only OS-issued identities and wall-clock observations are normalized afterwards.
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
const secret = 'runtime-parity-public-entry-test-secret'
const origin = 'https://rust-parity.invalid'
const entries = [], sockets = [], aliases = new Map()
const content = {manifestSha256:'0'.repeat(64),mods:[]}
const host = await startGameSessionSupervisor({adminSecret:secret,allowedOrigins:[origin],
  revision:'a'.repeat(40),runtimeEvents:entry=>entries.push(entry)})
const delay = () => new Promise(resolve=>setTimeout(resolve,5))
async function until(predicate) {
  const end=Date.now()+8000
  while(!predicate()) {if(Date.now()>end)throw new Error('Source runtime event was not observed');await delay()}
}
async function join(path,name) {
  const response=await fetch(host.address.url+path,{method:'POST',headers:{authorization:`Bearer ${secret}`,
    'content-type':'application/json'},body:JSON.stringify({content})})
  if(!response.ok)throw new Error(`${path}: ${response.status}`)
  const endpoint=await response.json()
  const socket=new WebSocket(host.address.url.replace('http:','ws:')+endpoint.path,{origin})
  sockets.push(socket)
  const welcome=new GameWelcomeReceiver(), save=new GameSaveCheckpointReceiver(), messages=[]
  const send=message=>socket.send(encodeGameMessage(message))
  socket.on('message',raw=>{
    const message=welcome.receivePayload(raw.toString(),payload=>socket.send(payload))
    if(!message)return
    if(message.type==='server-snapshot'&&socket.readyState===WebSocket.OPEN)
      send({type:'client-snapshot-ack',requireKeyframe:false,sequence:message.sequence})
    if(message.type==='server-save-checkpoint-chunk') {
      save.acceptChunk(message)
      send({type:'client-save-checkpoint-chunk-ack',sequence:message.sequence,nextOffset:message.offset+message.data.length})
    }
    messages.push(message)
  })
  socket.on('error',()=>{})
  await new Promise((resolve,reject)=>{socket.once('open',resolve);socket.once('error',reject)})
  send({type:'client-hello',protocolVersion:GAME_PROTOCOL_VERSION,credential:endpoint.credential,
    enhancedEffects:true,character:{displayName:name,element:'ether',discipline:'arcane'},
    profile:{accountUsername:null,highestWave:null,totalPlaytimeMs:null},
    onlinePreferences:{activityMessages:true,globalChat:true,submitRuns:true},cheatsEnabled:false})
  await until(()=>messages.some(message=>message.type==='server-welcome'))
  const id=messages.find(message=>message.type==='server-welcome').playerId
  aliases.set(id,`player:${name}`)
  await until(()=>messages.some(message=>message.type==='server-party-state'))
  const party=messages.find(message=>message.type==='server-party-state').state.party
  aliases.set(party.id,`party:${name}`)
  if(path==='/admin/sessions') {
    const provisioned=entries.findLast(entry=>entry.event==='session.provisioned')
    aliases.set(provisioned.details.sessionId,'private:one')
  }
  return {id,socket,send,messages}
}
function normalize(value,key='') {
  if(key==='durationMs')return {nonnegative:Number.isFinite(value)&&value>=0}
  if(key==='serverTick')return {nonnegativeInteger:Number.isInteger(value)&&value>=0}
  if(typeof value==='string')return aliases.get(value)??value
  if(Array.isArray(value))return value.map(item=>normalize(item))
  if(value&&typeof value==='object')return Object.fromEntries(Object.entries(value).map(([name,item])=>[name,normalize(item,name)]))
  return value
}
try {
  const alpha=await join('/admin/hub/tickets','Alpha')
  const beta=await join('/admin/hub/tickets','Beta')
  const college=await join('/admin/sessions','Private')
  alpha.send({type:'client-party-invite',targetPlayerId:beta.id})
  await until(()=>beta.messages.some(message=>message.type==='server-party-state'&&message.state.invitations.length))
  const invitation=beta.messages.findLast(message=>message.type==='server-party-state'&&message.state.invitations.length).state.invitations[0]
  aliases.set(invitation.id,'invitation:one')
  beta.send({type:'client-party-accept',invitationId:invitation.id})
  await until(()=>entries.some(entry=>entry.event==='party.invitation_accepted'))
  alpha.send({type:'client-party-settings',visibility:'invite-only'})
  await until(()=>entries.some(entry=>entry.event==='party.visibility_changed'))
  alpha.send({type:'client-party-rotate-code'})
  await until(()=>entries.some(entry=>entry.event==='party.join_code_rotated'))
  beta.send({type:'client-party-leave'})
  await until(()=>entries.some(entry=>entry.event==='party.member_left'))
  for(const client of [college,beta,alpha]) {
    const count=entries.filter(entry=>entry.event==='player.disconnected').length
    client.socket.close(1000,'fixture close')
    await until(()=>entries.filter(entry=>entry.event==='player.disconnected').length>count)
  }
  await until(()=>entries.some(entry=>entry.event==='session.closed'))
  const rows=entries.map(({component,event,message,details,occurredAtUtc})=>({component,event,message,
    timestampIsSourceUtc:/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/.test(occurredAtUtc),details:normalize(details)}))
  writeFixture('hosted-runtime-events-goldens.json','hosted-runtime-events-goldens.mjs',
    'current host/game-session-supervisor.ts + game-host.ts public sockets; opaque IDs/wall clocks normalized',{rows})
} finally {
  for(const socket of sockets)socket.close()
  await host.close()
}
