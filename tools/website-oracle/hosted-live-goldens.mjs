import { pathToFileURL } from 'node:url'
import { createRequire } from 'node:module'
import { writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { createHash } from 'node:crypto'
import { websiteDir, fixturesDir } from './oracle.mjs'
const base=resolve(websiteDir,'frontend')
const require=createRequire(`${base}/package.json`)
const {WebSocket}=require('ws')
const {startGameSessionSupervisor}=await import(pathToFileURL(`${base}/src/game/host/game-session-supervisor.ts`))
const {encodeGameMessage,decodeServerGameMessage}=await import(pathToFileURL(`${base}/src/game/protocol/game-protocol.ts`))
const secret='runtime-parity-public-entry-test-secret'
const origin='https://rust-parity.invalid'
const content={manifestSha256:'0'.repeat(64),mods:[]}
const start=revision=>startGameSessionSupervisor({adminSecret:secret,allowedOrigins:[origin],revision,luaWasmPath:require.resolve('wasmoon/dist/glue.wasm')})
let host=await start('a'.repeat(40))
const sockets=[]
const rows=[]
const post=async(path,body,authorized=true)=>{
  const response=await fetch(host.address.url+path,{method:'POST',headers:{'content-type':'application/json',...(authorized?{authorization:`Bearer ${secret}`}:{})},body:JSON.stringify(body)})
  return {status:response.status,body:await response.json()}
}
const get=async(path)=>{
 const response=await fetch(host.address.url+path,{headers:{authorization:`Bearer ${secret}`}})
 return {status:response.status,body:await response.json()}
}
async function connect(endpoint){
 const socket=new WebSocket(host.address.url.replace('http:','ws:')+endpoint.path,{origin})
 sockets.push(socket)
 const messages=[]
 let waiting=null
 socket.on('message',data=>{
  const message=decodeServerGameMessage(data.toString())
  if(message.type==='server-snapshot')socket.send(encodeGameMessage({type:'client-snapshot-ack',requireKeyframe:false,sequence:message.sequence}))
  messages.push(message)
  if(waiting){const index=messages.findIndex(waiting.accept);if(index>=0){const done=waiting;waiting=null;clearTimeout(done.timer);done.resolve(messages.splice(index,1)[0])}}
 })
 socket.on('error',()=>{})
 await new Promise((resolve,reject)=>{socket.once('open',resolve);socket.once('error',reject)})
 const next=accept=>{const index=messages.findIndex(accept);if(index>=0)return Promise.resolve(messages.splice(index,1)[0]);return new Promise((resolve,reject)=>{waiting={accept,resolve,timer:setTimeout(()=>reject(new Error('source message timeout: '+messages.map(message=>message.type+(message.reason?': '+message.reason:'')).join(','))),5000)}})}
 return {socket,next,send:message=>socket.send(encodeGameMessage(message))}
}
async function join(endpoint,name,extra={}){
 const client=await connect(endpoint)
 const {socket,next}=client
 socket.send(encodeGameMessage({type:'client-hello',protocolVersion:129,credential:endpoint.credential,
  character:{discipline:'arcane',element:'ether',displayName:name},profile:{accountUsername:null,highestWave:null,totalPlaytimeMs:null},
  onlinePreferences:{activityMessages:true,globalChat:true,submitRuns:true},cheatsEnabled:false,...extra}))
 const welcome=await next(message=>message.type==='server-welcome')
 return {...client,welcome}
}
try{
 rows.push({case:'unauthorized',...(await post('/admin/sessions',{content},false))})
 rows.push({case:'invalid-account',...(await post('/admin/sessions',{content,leaderboardUserId:0}))})
 const one=await post('/admin/sessions',{content})
 const two=await post('/admin/sessions',{content})
 rows.push({case:'private-provision',status:[one.status,two.status],distinctPaths:one.body.path!==two.body.path,distinctCredentials:one.body.credential!==two.body.credential,kind:one.body.sessionKind})
 const a=await join(one.body,'Alpha')
 const b=await join(two.body,'Beta')
 rows.push({case:'private-owners',playerIds:[a.welcome.playerId,b.welcome.playerId],kinds:[a.welcome.sessionKind,b.welcome.sessionKind]})
 a.send({type:'client-chat',channel:'global',text:'hello Colleges'})
 const chat=await b.next(message=>message.type==='server-chat'&&message.text==='hello Colleges')
 rows.push({case:'cross-college-chat',channel:chat.channel,sender:chat.sender.displayName,referenceIsOpaque:chat.sender.playerId.startsWith('player-ref-'),senderReferenceMatches:chat.sender.playerId===chat.sender.playerReference})
 const h1=await join((await post('/admin/hub/tickets',{content})).body,'Gamma')
 let h2=await join((await post('/admin/hub/tickets',{content})).body,'Delta')
 h1.send({type:'client-party-invite',targetPlayerId:h2.welcome.playerId})
 const invite=await h2.next(message=>message.type==='server-party-state'&&message.state.invitations.length===1)
 h2.send({type:'client-party-accept',invitationId:invite.state.invitations[0].id})
 const merged=await h1.next(message=>message.type==='server-party-state'&&message.state.party.memberPlayerIds.length===2)
 rows.push({case:'global-party-merge',leaderRetained:merged.state.party.leaderPlayerId===h1.welcome.playerId,memberNames:merged.state.partyRoster.map(row=>row.displayName),hubNames:merged.state.hubPlayers.map(row=>row.displayName).sort()})
 const resolved=await post('/admin/join/resolve',{code:merged.state.party.joinCode})
 rows.push({case:'code-resolution',status:resolved.status,target:resolved.body.target,intentOpaque:/^[\w-]{32}$/.test(resolved.body.intentId)})
 const health=await get('/health')
 rows.push({case:'health',body:{...health.body,protocol:undefined}})
 a.send({type:'client-start-match',boneyardId:'default-random'})
 const loaded=(await a.next(message=>message.type==='server-boneyard-loaded')).boneyard
 rows.push({case:'actual-loaded-world',runAndSeedDistinct:loaded.runId!==loaded.seed,
   choice:loaded.choice.id,source:loaded.choice.source,named:loaded.scene.name.length>0,
   sourceDigest:loaded.sourceSha256.length===64,geometryDigest:loaded.geometrySha256.length===64,
   authoredObjects:loaded.scene.objects.length>0,authoredSprites:loaded.scene.sprites.length>0})
 const matches=await get('/admin/matches')
 const match=matches.body.items.find(item=>item.players.includes('Alpha'))
 rows.push({case:'observer-directory',status:matches.status,players:match.players,partyLeader:match.partyLeader,
  playerCount:match.playerCount,session:match.session,visibility:match.visibility,boneyardNamed:match.boneyardName.length>0,waveNumber:match.waveNumber})
 const endpoint=await post('/admin/observers',{matchId:match.id,observer:{userId:71,username:'Auditor'}})
 const observer=await connect(endpoint.body)
 observer.send({type:'client-observer-hello',protocolVersion:129,credential:endpoint.body.credential})
 const observed=await observer.next(message=>message.type==='server-welcome')
 rows.push({case:'observer-hello',status:endpoint.status,observer:observed.observer,developer:observed.developerAccess,
  existingView:observed.playerId===a.welcome.playerId,session:observed.sessionKind})
 observer.send({type:'client-party-leave'})
 const rejection=await observer.next(message=>message.type==='server-disconnect')
 rows.push({case:'observer-readonly',code:rejection.code,reason:rejection.reason,humanPlayers:(await get('/health')).body.privatePlayers})
 const presence=await get('/admin/presence')
 rows.push({case:'presence',items:presence.body.items.map(({displayName,activity,partySize,developer,session})=>({displayName,activity,partySize,developer,session})).sort((a,b)=>a.displayName.localeCompare(b.displayName))})

 const mod={boneyards:[],contentSha256:'a'.repeat(64),entryScript:null,files:[],id:'hosted.content',name:'Hosted Content',priority:1.0,slug:'hosted',version:'1.0.0'}
 const packageContent=mods=>({manifestSha256:'f'.repeat(64),mods})
 const missingEntry={...mod}
 delete missingEntry.entryScript
 rows.push({case:'package-missing-nullable-field',...(await post('/admin/sessions',{content:packageContent([missingEntry])}))})
 rows.push({case:'package-lua-rejected',...(await post('/admin/sessions',{content:packageContent([{...mod,entryScript:'sd.kit.itme{key="bad"}'}])}))})
 const image=Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jM1sAAAAASUVORK5CYII=','base64')
 const imageFile={byteLength:image.length,bytesBase64:image.toString('base64'),contentType:'image/png',kind:'image',path:'art/icon.png',sha256:createHash('sha256').update(image).digest('hex')}
 const authored={...mod,files:[imageFile],entryScript:'local icon=sd.art.sprite("art/icon.png")\nsd.status{key="ward"}\nsd.potion{key="tonic",name="Tonic",duration="1s",art={icon=icon},on_use=sd.effect.status{target="user",status=sd.ref("status","ward")}}'}
 const admitted=await post('/admin/sessions',{content:packageContent([authored]),leaderboardUserId:1.0})
 if(admitted.status!==201)throw new Error('Source authored admission: '+JSON.stringify(admitted))
 const authoredClient=await join(admitted.body,'Authored')
 rows.push({case:'package-lua-catalog',status:admitted.status,content:authoredClient.welcome.content,catalog:authoredClient.welcome.modCatalog,assets:authoredClient.welcome.modAssets})

 h1.send({type:'client-start-match',boneyardId:'default-random'})
 const partyWorld=(await h2.next(message=>message.type==='server-boneyard-loaded')).boneyard
 const partySave=(await h2.next(message=>message.type==='server-save-checkpoint'&&JSON.parse(message.save).continuation?.summary.worldKind==='boneyard')).save
 const partyToken=JSON.parse(partySave).continuation.summary.partyRejoinToken
 const rejoinRequest={content,token:partyToken,save:partySave}
 rows.push({case:'rejoin-connected',...(await post('/admin/rejoin',rejoinRequest))})
 rows.push({case:'rejoin-normalized-document',...(await post('/admin/rejoin',{...rejoinRequest,save:partySave+' '}))})
 const changedSave=JSON.parse(partySave)
 changedSave.continuation.summary.savedAtTick+=1
 rows.push({case:'rejoin-invalid-document',...(await post('/admin/rejoin',{...rejoinRequest,save:JSON.stringify(changedSave)}))})
 rows.push({case:'rejoin-wrong-account',...(await post('/admin/rejoin',{...rejoinRequest,leaderboardUserId:17}))})
 rows.push({case:'rejoin-legacy-ended',...(await post('/admin/rejoin',{...rejoinRequest,token:'r'.repeat(43)}))})
 const departed=h2.welcome.playerId
 h2.socket.close()
 await h1.next(message=>message.type==='server-party-state'&&message.state.partyRoster.some(row=>row.playerId===departed&&!row.connected))
 const reserved=await post('/admin/rejoin',rejoinRequest)
 rows.push({case:'rejoin-reserved',status:reserved.status,session:reserved.body.sessionKind,sameWorld:reserved.body.path==='/game-hub'})
 rows.push({case:'rejoin-already-reserved',...(await post('/admin/rejoin',rejoinRequest))})
 h2=await join(reserved.body,'Delta',{save:partySave,saveIntent:'resume'})
 rows.push({case:'rejoin-owner',samePlayer:h2.welcome.playerId===departed,sameRun:h2.welcome.snapshot.run.runId===partyWorld.runId,
   partyNames:(await h2.next(message=>message.type==='server-party-state')).state.partyRoster.map(row=>row.displayName)})

 const targetRevision='b'.repeat(40)
 const restarting=post('/admin/deployments/restart',{targetRevision})
 const clients=[a,b,h1,h2,authoredClient]
 const restartMessages=await Promise.all(clients.map(client=>client.next(message=>message.type==='server-deployment-restart')))
 const deploymentSaves=await Promise.all(clients.map((client,index)=>![0,2,3].includes(index)?null:client.next(message=>{
  if(message.type!=='server-save-checkpoint')return false
  const token=JSON.parse(message.save).continuation?.summary.partyRejoinToken
  return token!=null&&JSON.parse(Buffer.from(token.split('.')[1],'base64url')).targetRevision===targetRevision
 }).then(message=>message.save)))
 const upgradeStatus=await new Promise((resolve,reject)=>{
  const socket=new WebSocket(host.address.url.replace('http:','ws:')+'/game-hub',{origin})
  socket.once('unexpected-response',(_request,response)=>{response.resume();resolve(response.statusCode)})
  socket.once('open',()=>{socket.close();reject(new Error('deployment must reject the HTTP upgrade'))})
  socket.once('error',reject)
 })
 const rejectedAdmission=await post('/admin/hub/tickets',{content})
 rows.push({case:'deployment-draining',upgradeStatus,admission:rejectedAdmission})
 h2.send({type:'client-deployment-ready',checkpointSequence:restartMessages[3].checkpointSequence+1,targetRevision})
 const staleAcknowledgement=await h2.next(message=>message.type==='server-disconnect')
 rows.push({case:'deployment-stale-ack',code:staleAcknowledgement.code,reason:staleAcknowledgement.reason})
 for(const index of [0,1,2,4]){
  clients[index].send({type:'client-deployment-ready',checkpointSequence:restartMessages[index].checkpointSequence,targetRevision})
 }
 rows.push({case:'deployment-ready',...(await restarting)})
 const drained=(await get('/health')).body
 rows.push({case:'deployment-closed-colleges',draining:drained.draining,privateSessions:drained.privateSessions})

 await host.close()
 host=await start(targetRevision)
 rows.push({case:'rejoin-wrong-deployment',...(await post('/admin/rejoin',rejoinRequest))})
 const recoveryRequest=save=>({content,save,token:JSON.parse(save).continuation.summary.partyRejoinToken})
 const seed=await post('/admin/rejoin',recoveryRequest(deploymentSaves[3]))
 rows.push({case:'recovery-global-seed',status:seed.status,session:seed.body.sessionKind,globalPath:seed.body.path==='/game-hub'})
 const waitingMember=post('/admin/rejoin',recoveryRequest(deploymentSaves[2]))
 const recoveredDelta=await join(seed.body,'Delta',{save:deploymentSaves[3],saveIntent:'resume'})
 const recoveredRoster=(await recoveredDelta.next(message=>message.type==='server-party-state')).state
 rows.push({case:'recovery-offline-leader',samePlayer:recoveredDelta.welcome.playerId===departed,
   sameRun:recoveredDelta.welcome.snapshot.run.runId===partyWorld.runId,
   leaderPreserved:recoveredRoster.party.leaderPlayerId===h1.welcome.playerId,
   onlySeedActor:Object.values(recoveredDelta.welcome.snapshot.players).map(player=>player.config.displayName),
   offline:recoveredRoster.partyRoster.filter(row=>!row.connected).map(row=>row.displayName)})
 const secondEndpoint=await waitingMember
 rows.push({case:'recovery-waiting-member',status:secondEndpoint.status,samePath:secondEndpoint.body.path===seed.body.path})
 const recoveredGamma=await join(secondEndpoint.body,'Gamma',{save:deploymentSaves[2],saveIntent:'resume'})
 const recoveredParty=(await recoveredGamma.next(message=>message.type==='server-party-state')).state
 rows.push({case:'recovery-shared-world',samePlayer:recoveredGamma.welcome.playerId===h1.welcome.playerId,
   sameRun:recoveredGamma.welcome.snapshot.run.runId===partyWorld.runId,
   actorNames:Object.values(recoveredGamma.welcome.snapshot.players).map(player=>player.config.displayName).sort(),
   partyNames:recoveredParty.partyRoster.map(row=>row.displayName),
   onlyOneRun:(await get('/health')).body.runs===1})
 const privateSeed=await post('/admin/rejoin',recoveryRequest(deploymentSaves[0]))
 const recoveredAlpha=await join(privateSeed.body,'Alpha',{save:deploymentSaves[0],saveIntent:'resume'})
 rows.push({case:'recovery-private-seed',status:privateSeed.status,session:privateSeed.body.sessionKind,
   privatePath:privateSeed.body.path.startsWith('/game-sessions/'),samePlayer:recoveredAlpha.welcome.playerId===a.welcome.playerId,
   sameRun:recoveredAlpha.welcome.snapshot.run.runId===loaded.runId,privateSessions:(await get('/health')).body.privateSessions})

 writeFileSync(resolve(fixturesDir,'hosted-live-goldens.json'),JSON.stringify({source:'host/game-session-supervisor.ts + game-host.ts + game-social-broker.ts',generator:'tools/website-oracle/hosted-live-goldens.mjs',authoredContent:packageContent([authored]),rows})+'\n')
 console.log(`wrote hosted-live-goldens.json from ${rows.length} actual supervisor/socket observations`)
}finally{
 for(const socket of sockets)socket.close()
 await host.close()
}
