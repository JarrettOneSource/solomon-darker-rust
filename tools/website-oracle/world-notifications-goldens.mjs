import {loadKernel,writeFixture} from './oracle.mjs'

const source=await loadKernel('loot-message-presentation.ts')
const books=await loadKernel('skill-book-feedback.ts')
const scenarios=[]
function scenario(name,actions) {
  const presentation=new source.NativeLootMessagePresentation(0)
  const records=[]
  for(const action of actions) {
    if(action.sample!==undefined)records.push({sample:action.sample,visuals:presentation.sample(action.sample)})
    else if(action.loot)records.push({loot:action.loot,accepted:presentation.consume(action.loot)})
    else records.push({notification:action,accepted:presentation.consumeText(action)})
  }
  scenarios.push({name,records})
}
const event=(source,eventId,tick,text,tint=0xffffff)=>({source,eventId,tick,text,tint})
scenario('independent-equal-ids',[
  event('loot',1,1,'Health Potion'),{sample:1},
  event('combat',1,2,'CHEAT DEATH!'),{sample:2},
  event('secondary',1,3,'Overloaded Mana!'),{sample:3},
  books.nativeSkillBookWorldMessage(1,8,4),{sample:4},{sample:18},{sample:30},
  {sample:300},{sample:600},
])
scenario('gold-boundary-and-integer-format',[
  {loot:{eventId:1,tick:1,text:'4 GOLD',sound:'pickup-coin'}},{sample:1},
  {loot:{eventId:2,tick:2,text:'7 GOLD',sound:'pickup-coin'}},{sample:2},
  {sample:100},{loot:{eventId:3,tick:101,text:'0004 GOLD'}},{sample:101},
  {sample:202},{loot:{eventId:4,tick:203,text:'1 GOLD'}},{sample:203},
  {loot:{eventId:5,tick:204,text:'9007199254740991 GOLD'}},{sample:204},
  {loot:{eventId:6,tick:205,text:'9007199254740991 GOLD'}},{sample:205},
  {loot:{eventId:7,tick:206,text:'9007199254740992 GOLD'}},{sample:206},
  {loot:{eventId:8,tick:207,text:'4 GOLD '}},{sample:207},
  {loot:{eventId:9,tick:208,text:'1.5 GOLD'}},{sample:208},
  {sample:207},{sample:9007199254740992},{sample:700},
])
for(const [text,sound]of [[undefined,undefined],['12 GOLD','pickup-coin'],['DAMAGE x4',undefined],
  ['BONUS SKILL POINT',undefined],['Shield +1',undefined],['Mana Potion','pickup-bag']]) {
  scenario(`loot-tint-${text??'no-text'}`,[
    {loot:{eventId:1,tick:0,...(text===undefined?{}:{text}),...(sound?{sound}:{})}},
    ...Array.from({length:305},(_,sample)=>({sample})),
  ])
}
scenario('insertion-pressure',[
  ...Array.from({length:40},(_,index)=>[
    event(['loot','book','combat','secondary'][index%4],index%4===1?-(index+1):index+1,
      0,`NOTICE ${index}`,index%2?0x8080ff:0xffffff),{sample:0},
  ]).flat(),
  ...[1,2,3,18,25,50,100,300,600].map(sample=>({sample})),
])
const visibility=[]
for(const life of [null,'alive','lethal-pending','dying','spectating']) {
  for(const combat of [false,true])visibility.push({life,combat,visible:source.nativeWorldNotificationsVisible(life,combat)})
}
const text=[]
for(const skillId of [0,7,...Array.from({length:74},(_,index)=>index+8),82]) {
  try{text.push({skillId,text:books.nativeSkillBookResultText(skillId)})}
  catch(error){text.push({skillId,error:error.message})}
}
const feedback=(sequence,accepted=true,action='read-skill-book',skillBookOutcome={kind:'rank',skillId:8})=>
  ({sequence,accepted,action,skillBookOutcome})
const initial=feedback(3)
const cursor=new books.NativeSkillBookFeedbackCursor(initial)
const cursorActions=[null,initial,feedback(2),feedback(4,false),feedback(4),feedback(5,true,'consume'),
  feedback(5),feedback(6,true,'read-skill-book',null),feedback(7,true,'read-skill-book',{kind:'choice'}),
  feedback(8,true,'read-skill-book',{kind:'rank',skillId:null}),feedback(9),feedback(9)]
const cursorRecords=cursorActions.map(input=>({input,result:cursor.consume(input)}))
writeFixture('world-notifications-goldens.json','tools/website-oracle/world-notifications-goldens.mjs',
  'Website fa08 NativeLootMessagePresentation, nativeWorldNotificationsVisible and NativeSkillBookFeedbackCursor',
  {sourceRevision:'fa08ae249ff5563570bdb2a653e2ef9358ce7b83',scenarios,visibility,text,
    cursor:{initial,records:cursorRecords}})
