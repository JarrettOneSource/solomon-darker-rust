import {loadKernel,writeFixture} from './oracle.mjs'
const source=await loadKernel('native-ui/native-ui-party-menu.ts')
const chip=await loadKernel('native-ui/native-ui-party-chip.ts')
const rect=value=>value===null?null:[value.left,value.top,value.width,value.height]
const texts=(nodes,clipped=false)=>nodes.flatMap(node=>node.kind==='clip'?texts(node.nodes,true):node.kind==='text'?[{label:node.label,...node.text,clipped}]:[])
const members=Array.from({length:9},(_,index)=>({id:`player-${index}`,name:['Wizard','A-Long_Name_with-fallback-font','Very Very Very Very Very Long Wizard Name','WIZARD🙂','Mégère'][index%5],removable:index>0,tags:index===0?['you','leader']:index===2?['offline']:[]}))
const requests=[{id:'request-a',name:'New Wizard'},{id:'request-b',name:'Another Very Very Very Long Name'}]
const base={width:1600,height:900,leader:true,code:'ABC123',members,requests,selectedTab:'members',visibility:'public',visibilityOptions:[{id:'public',label:'PUBLIC'},{id:'invite-only',label:'INVITE ONLY'},{id:'private',label:'PRIVATE'}],leaveLabel:'LEAVE PARTY'}
const inputs=[]
for(const selectedTab of source.NATIVE_UI_PARTY_MENU_TABS)for(const leader of [false,true])for(const code of [null,'ABC123'])for(const scrollY of [0,37,10000])inputs.push({...base,selectedTab,leader,code,scrollY,error:leader?'The requested wizard is no longer connected.':null})
inputs.push({...base,members:[],requests:[],leaveLabel:null})
inputs.push({...base,selectedTab:'settings',visibility:'private',copyLabel:'COPIED',visibilityOptions:[]})
const cases=inputs.map(input=>{
 const plan=source.planNativeUiPartyMenu(input)
 const actionIds=new Set(plan.actions.filter(action=>!action.id.startsWith('visibility:')).map(action=>`${action.id}:label`))
 const text=texts(plan.nodes).filter(node=>!node.label.startsWith('tab:')&&!actionIds.has(node.label)).map(node=>({label:node.label,text:node.text,font:node.font,align:node.align??'center',scale:node.scale??1,tint:node.tint,x:node.x,y:node.y,clipped:node.clipped}))
 return {input,expected:{contentHeight:plan.contentHeight,maximumScrollY:plan.maximumScrollY,scrollY:plan.scrollY,rows:plan.rows.map(row=>({bounds:rect(row.bounds),id:row.id,visibleBounds:rect(row.visibleBounds)})),rowActions:plan.rowActions.map(row=>({bounds:rect(row.bounds),id:row.id,label:row.label,rowId:row.rowId})),actions:plan.actions.map(action=>({bounds:rect(action.bounds),disabled:action.disabled,id:action.id})),texts:text}}
})
const fits=[]
for(const text of ['Wizard','A-Name_123','Két🙂 ﻿','Long Wizard Name    '])for(const font of ['world-and-roster','menu','body'])for(const maximum of [-1,0,30,250])fits.push({text,font,maximum,expected:source.fitNativeUiPartyMenuText(text,font,maximum),nameFont:source.nativeUiPartyMenuNameFont(text)})
const invitation={
 frame:rect(chip.NATIVE_UI_PARTY_INVITATION.bounds),
 title:chip.NATIVE_UI_PARTY_INVITATION.title,
 actions:chip.nativeUiPartyInvitationActionBounds().map(rect),
 bodies:['Wizard','Mégère','A name with spaces','Wizard🙂',''].map(inviter=>({inviter,expected:chip.nativeUiPartyInvitationBody(inviter)})),
}
writeFixture('party-ui-goldens.json','party-ui-goldens.mjs','native-ui/native-ui-party-menu.ts and native-ui-party-chip.ts (pinned ed0a2d59)',{cases,fits,invitation})
