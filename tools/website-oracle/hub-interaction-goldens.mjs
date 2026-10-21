import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { gameDir, loadKernel, writeFixture } from './oracle.mjs'
const native = await loadKernel('core-kernels/native-hub-npc.ts')
const presentation = await loadKernel('hub-inventory-presentation.ts')
const chat = await loadKernel('hub-npc-dialogue.ts')
const markers = await loadKernel('renderer/hub-npc-marker-presentation.ts')
const catalog = JSON.parse(readFileSync(resolve(gameDir, 'native-hub-npc-catalog.json')))
const dialogues=[]
for (const id of presentation.HUB_INTERACTION_IDS) for (const storyOffice of [false,true]) {
  const npc=native.createNativeHubNpcState()
  dialogues.push({id,storyOffice,
    intro:chat.createHubNpcChatContent(id,npc,1,null,storyOffice),
    choices:chat.hubNpcChatChoices(id,storyOffice),
    questions:presentation.hubInteractionDialogue(id,storyOffice).questions.map(question=>({key:question.key,content:chat.hubNpcQuestion(id,question.key,storyOffice)})),
    dismissal:chat.hubNpcDismissal(id,-7,storyOffice),
  })
}
const geometry=[]
for (const id of presentation.HUB_INTERACTION_IDS) {
  const authored=presentation.HUB_INTERACTION_GEOMETRY[id]
  for (const offset of [[0,0],[10,0],[50,0],[52,0],[100,0],[0,40]]) {
    const point={x:authored.position.x+offset[0],y:authored.position.y+offset[1]}
    for (const storyOffice of [false,true]) {
      const availability={skorchaPosition:{x:1437.5,y:732.5},storyOffice}
      geometry.push({region:authored.region,point,availability,
        nearest:presentation.nearestHubInteraction(authored.region,point,availability),
        atPoint:presentation.hubInteractionAtPoint(authored.region,point,availability),
        withinRange:presentation.hubInteractionWithinRange(id,authored.region,point,availability),id})
    }
  }
}
const markerFrames=[]
for(const actor of catalog.markers.actors) for(const tick of [0,37,6000]) for(const seed of [0,0x5eedc0de]) {
  for(const suppressed of [false,true]) for(const surface of [null,'dialogue','inventory','modal','service']) {
    const suppression=[suppressed,suppressed,suppressed]
    const options={skorchaPosition:{x:1637,y:403.5},skorchaVariant:1,surface}
    markerFrames.push({id:actor.interactionId,tick,seed,suppression,options,
      frame:markers.hubNpcMarkerFrame(actor.interactionId,tick,seed,suppression,options)})
  }
}
const selectors=[]
for(const selector of ['boast','books','teacher-spells']) for(const read of [false,true]) {
  const npc={...native.createNativeHubNpcState(),librarianLaceRead:read}
  selectors.push({selector,read,title:chat.hubNpcSelectorTitle(selector),
    rows:chat.hubNpcSelectorRows(selector,npc,{advancedUnlocks:[true,false,false,true,false,false,false,false]})})
}
const selectorResponses=[]
for(const selector of ['boast','books','teacher-spells']) {
  for(const row of chat.hubNpcSelectorRows(selector,native.createNativeHubNpcState(),{advancedUnlocks:Array(8).fill(false)})) {
    selectorResponses.push({selector,id:row.id,response:chat.hubNpcSelectorResponse(selector,row.id),action:chat.hubNpcSelectorAction(selector,row.id)})
  }
}
for(const tick of [0,37,6000]) for(const seed of [0,0x5eedc0de]) {
  for(const suppressed of [false,true]) for(const surface of [null,'dialogue','inventory','modal','service']) {
    markerFrames.push({id:'polisher',tick,seed,suppression:[suppressed,suppressed,suppressed],options:{surface},
      frame:{...markers.hubStoryOfficePolisherMarkerFrame(tick,seed,surface),interactionId:'polisher',region:'office',side:'left',style:'help'}})
  }
}
const onboarding=[]
for(let bits=0;bits<8;bits+=1) for(const tick of [0,40,41,79,80]) for(const surface of [null,'dialogue']) {
  const flags=Array(10).fill(false)
  for(let index=0;index<3;index+=1)flags[index]=Boolean(bits&(1<<index))
  onboarding.push({flags,tick,surface,plans:markers.hubNpcOnboardingPlan(flags,tick,surface)})
}
writeFixture('hub-interaction-goldens.json','tools/website-oracle/hub-interaction-goldens.mjs',
  'Website: hub-npc-dialogue.ts; hub-inventory-presentation.ts; renderer/hub-npc-marker-presentation.ts; native-hub-npc-catalog.json',
  {catalog,dialogues,geometry,markerFrames,selectors,selectorResponses,onboarding})
