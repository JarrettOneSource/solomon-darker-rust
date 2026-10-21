#!/usr/bin/env node
// Actual pinned chat, roster, admission and ally projections; no browser/session mocks.
import {readFileSync} from 'node:fs'
import {resolve} from 'node:path'
import {stripTypeScriptTypes} from 'node:module'
import {gameDir,loadKernel,writeFixture} from './oracle.mjs'
const protocol=await loadKernel('protocol/game-chat.ts')
const chat=await loadKernel('game-chat.ts')
const party=await loadKernel('host/party-system.ts')
const menu=await loadKernel('party-menu-presentation.ts')
const allies=await loadKernel('ally-hud.ts')
const host=readFileSync(resolve(gameDir,'host/game-host.ts'),'utf8')
const rateSource=host.match(/function chatRateRetryAfter\([^]*?\n  \}/)?.[0]
if(!rateSource)throw new Error('Pinned chatRateRetryAfter function boundary changed')
const rate=new Function('GAME_CHAT_RATE_WINDOW_MS','GAME_CHAT_RATE_LIMIT',
 `${stripTypeScriptTypes(rateSource)};return chatRateRetryAfter;`)(5000,5)
const normalize=[]
for(const input of ['', ' \t\n', '  hello world  ', '\ufeffhello\ufeff', '\u0085hello\u0085', '\u180ehello\u180e',
 '\u2000\u2028hello\u2029\u3000', 'a\tb','a\nb','a\0b','a\x7fb','x'.repeat(180),'x'.repeat(181),
 '😀'.repeat(90),'😀'.repeat(91),'漢'.repeat(170),'漢'.repeat(171), 'x'.repeat(181)+'\0', '\u0085']){
 try{normalize.push({input,text:protocol.normalizeGameChatText(input),error:null})}
 catch(error){normalize.push({input,text:null,error:error.message})}
}
const channels=[]
for(const world of ['hub','boneyard'])for(const memberCount of [0,1,3])for(const sessionKind of ['standalone','private-college','global-hub'])for(const whisper of [false,true]){
 const state=memberCount?{party:{memberPlayerIds:Array.from({length:memberCount},(_,i)=>`player-${i}`)}}:null
 const available=chat.availableGameChatChannels(world,state,sessionKind,whisper)
 channels.push({world,memberCount,sessionKind,whisper,available,
  next:['party','boneyard','global','whisper'].map(current=>[current,chat.nextGameChatChannel(current,available)])})
}
const message=sequence=>({channel:'party',sender:{displayName:'Felix',playerId:'player-b',playerReference:'native:b'},sequence,text:`message ${sequence}`})
let history=[]
const historySteps=[]
for(const sequence of [0,1,1,0,3,2,...Array.from({length:98},(_,i)=>i+4)]){
 const next=chat.appendGameChatMessage(history,message(sequence))
 historySteps.push({sequence,changed:next!==history,sequences:next.map(item=>item.sequence)});history=next
}
const unread=[]
for(const sender of ['player-a','player-b'])for(const open of [false,true])for(const channel of ['party','boneyard','global','whisper'])for(const current of ['party','boneyard','global','whisper']){
 unread.push({sender,open,channel,current,value:chat.shouldIncrementGameChatUnread({...message(1),channel,sender:{...message(1).sender,playerId:sender}},'player-a',open,current)})
}
const fade=[]
for(const open of [false,true])for(const last of [0,100])for(const now of [0,4999,5000,5001,5100])fade.push({open,last,now,value:chat.isGameChatFaded(open,last,now)})
const rejections=[]
for(const channel of ['party','boneyard','global','whisper'])for(const reason of ['channel-unavailable','rate-limited','target-unavailable'])for(const retryAfterMs of [0,1,1000,1001,5000]){
 rejections.push({channel,reason,retryAfterMs,text:chat.gameChatRejectionText({channel,reason,retryAfterMs})})
}
const rateLimits=[]
for(const times of [[0,0,0,0,0,0,4999,5000,5000],[1000,1100,1200,1300,1400,1500,5999,6000,6100],[8000,8100,8200,8300,8400,100,5099,13000]]){
 const client={chatSentAtMs:[]}
 rateLimits.push(times.map(now=>({now,retry:rate(client,now),accepted:[...client.chatSentAtMs]})))
}
const membership={id:'native-session',joinCode:'ABCD-EFGH',listingId:'native-listing',leaderPlayerId:'player-b',memberPlayerIds:['player-b','player-a','player-c'],visibility:'private'}
const profiles=['player-c','player-a','player-b'].map((id,i)=>({accountUsername:null,highestWave:i,totalPlaytimeMs:i*60000,displayName:['Sirmin','Felix','Volusius'][i],playerId:id}))
const roster=profiles.map((profile,i)=>({connected:i!==0,currentHealth:[-2,25,60][i],displayName:profile.displayName,element:['air','ether','fire'][i],lifeState:i===0?'dying':'alive',maximumHealth:50,playerId:profile.playerId}))
const state={invitations:[],joinRequests:[],nextInvitationOrdinal:1,parties:[membership],revision:13}
const projections=[]
for(const playerId of membership.memberPlayerIds){
 const value=party.projectPartyState(state,playerId,new Map(profiles.map(row=>[row.playerId,row])),new Set(profiles.map(row=>row.playerId)),new Map(roster.map(row=>[row.playerId,row])))
 projections.push({playerId,value,menu:menu.partyMenuModel(value,playerId,'private-college')})
}
const kicks=[]
for(const requester of ['player-a','player-b','missing'])for(const target of ['player-a','player-b','missing']){
 const result=party.kickPartyPlayer(state,requester,target,{id:'new-party',joinCode:'JKLM-NPQR',listingId:'new-listing'})
 kicks.push({requester,target,reason:result.reason})
}
const departures=[]
for(const ids of [membership.memberPlayerIds,['player-b']])for(const playerId of ['player-a','player-b','missing']){
 const start={...state,parties:[{...membership,memberPlayerIds:ids}]}
 const next=party.removePartyPlayer(start,playerId)
 departures.push({ids,playerId,party:next.parties.find(row=>row.id===membership.id)??null})
}
const leaveLabels=[]
for(const count of [0,1,2,4])for(const kind of ['standalone','private-college','global-hub'])leaveLabels.push({count,kind,label:menu.partyMenuLeaveLabel(count,kind)})
const playerAllies=[]
const players=Object.fromEntries(roster.map(row=>[row.playerId,{config:{displayName:`live ${row.displayName}`,element:row.element},progression:{currentHealth:row.currentHealth+5,maximumHealth:row.maximumHealth,lifeState:row.lifeState}}]))
for(const retained of [null,roster,[...roster].map(row=>({...row,connected:false})),roster.filter(row=>row.playerId!=='player-b')])for(const local of ['player-a','player-b','missing']){
 const rows=allies.derivePlayerAllyHudRows(players,local,retained??undefined)
 playerAllies.push({retained,local,rows:rows.map(row=>({...row,status:allies.allyHudAccessibleStatus(row)}))})
}
const golemActors=[
 {id:20,kind:'golem',worldKey:'boneyard:one',golem:{currentHealth:25,maximumHealth:50}},
 {id:2,kind:'golem',worldKey:'boneyard:one',golem:{currentHealth:100,maximumHealth:50}},
 {id:1,kind:'golem',worldKey:'hub:courtyard',golem:{currentHealth:5,maximumHealth:50}},
 {id:4,kind:'acid-rain',worldKey:'boneyard:one',golem:null},
 {id:3,kind:'golem',worldKey:'boneyard:one',golem:null},
]
const golems=['boneyard:one','hub:courtyard','missing'].map(world=>({world,rows:allies.deriveGolemAllyHudRows(golemActors,world)}))
writeFixture('social-goldens.json','tools/website-oracle/social-goldens.mjs',[
 'protocol/game-chat.ts','game-chat.ts','host/party-system.ts','host/game-host.ts:chatRateRetryAfter',
 'party-menu-presentation.ts','ally-hud.ts',
],{normalize,channels,historySteps,unread,fade,rejections,rateLimits,membership,profiles,roster,projections,kicks,departures,leaveLabels,players,playerAllies,golemActors,golems})
