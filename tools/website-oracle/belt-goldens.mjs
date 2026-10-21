import { loadKernel, writeFixture } from './oracle.mjs'
const belt = await loadKernel('core-kernels/native-belt.ts')
const profile = await loadKernel('core-kernels/player-progression.ts')
const economy = await loadKernel('core-kernels/hub-economy.ts')
const creations=[]
for(const element of ['air','earth','ether','fire','water']) for(const discipline of ['arcane','body','mind']) {
  const book=profile.createPlayerSkillBook({element,discipline,displayName:'oracle'})
  creations.push({element,discipline,belt:belt.createNativePlayerBelt(book)})
}
const config={element:'ether',discipline:'arcane',displayName:'oracle'}
let book=profile.createPlayerSkillBook(config)
let state=economy.createHubEconomy(1)
const sack=economy.createFomentiusInventoryItem(economy.FOMENTIUS_STOCK_DEFINITIONS[5],100)
sack.contents=[economy.createFomentiusInventoryItem(economy.FOMENTIUS_STOCK_DEFINITIONS[0],101,3),
  economy.createFomentiusInventoryItem(economy.FOMENTIUS_STOCK_DEFINITIONS[6],102),
  economy.createFomentiusInventoryItem(economy.FOMENTIUS_STOCK_DEFINITIONS[3],103),
  economy.createEquipmentInventoryItem(economy.DOWSING_EQUIPMENT_RECIPES[35],104)]
state={...state,backpack:[...state.backpack,sack]}
let entries=belt.createNativePlayerBelt(book)
const initial={book,economy:state,belt:entries}
const records=[]
function snapshot(operation,accepted=true) {
  records.push({operation,accepted,belt:entries,skills:belt.nativeBeltSkillProjection(entries),
    health:{count:belt.nativeBeltPotionProjection(state.backpack,0).count,itemId:belt.nativeBeltPotionProjection(state.backpack,0).item?.id??null},
    mana:{count:belt.nativeBeltPotionProjection(state.backpack,1).count,itemId:belt.nativeBeltPotionProjection(state.backpack,1).item?.id??null},
    entryItems:entries.map(entry=>entry===null?null:belt.nativeBeltEntryItem(entry,state)?.id??null)})
}
for(const [itemId,slot] of [[101,2],[102,5],[103,6],[5,7],[99999,1]]) {
  let accepted=true
  try{entries=belt.bindNativeBeltItem(entries,state,itemId,slot)}catch{accepted=false}
  snapshot({type:'bind-item',itemId,slot},accepted)
}
for(const [skillId,slot] of [[null,0],[16,0],[0,1],[11,8],[11,23],[11,24]]) {
  let accepted=true
  try{entries=belt.bindNativeBeltSkill(entries,book,skillId,slot)}catch{accepted=false}
  snapshot({type:'bind-skill',skillId,slot},accepted)
}
const previous=book
book=profile.grantPlayerSkillRanks(book,16,1)
entries=belt.autofillNewlyLearnedNativeBeltSkills(entries,previous,book)
snapshot({type:'learn',skillId:16})
entries=belt.bindNativeBeltItem(entries,state,104,6)
snapshot({type:'bind-item',itemId:104,slot:6})
state=economy.transferInventoryItem(state,104,'to-storage').state
entries=belt.refreshNativePlayerBelt(entries,book,state)
snapshot({type:'store',itemId:104})
state=economy.consumeInventoryItem(state,102).state
entries=belt.refreshNativePlayerBelt(entries,book,state)
snapshot({type:'consume',itemId:102})
// Equipment can make a visible skill usable without a permanent rank. The
// public belt boundary receives that accepted skill book, then sees its expiry.
let previousVisibleBook=book
book={...book,effectiveRanks:book.effectiveRanks.map((rank,id)=>id===24?2:rank),
  learnedSkillOrder:[...book.learnedSkillOrder,24]}
entries=belt.autofillNewlyLearnedNativeBeltSkills(entries,previousVisibleBook,book)
snapshot({type:'temporary-skill',skillId:24,rank:2})
entries=belt.bindNativeBeltSkill(entries,book,24,16)
snapshot({type:'bind-skill',skillId:24,slot:16})
book={...book,effectiveRanks:book.effectiveRanks.map((rank,id)=>id===24?0:rank),
  learnedSkillOrder:book.learnedSkillOrder.filter(id=>id!==24)}
entries=belt.refreshNativePlayerBelt(entries,book,state)
snapshot({type:'expire-temporary-skill',skillId:24})
const migrations=[[11,null,null,null,null,null,null,null],[8,16,24,32,40,52,57,63],Array(8).fill(null)]
  .map(quickbar=>({quickbar,belt:belt.migrateSkillQuickbarToNativeBelt(quickbar)}))
const admission=[]
for(const nativeTypeId of [7000,...belt.NATIVE_BELT_ITEM_TYPE_IDS,7012]) for(const nativeSubtype of [null,0,1,5,6]) {
  const item={kind:'equipment',nativeSubtype,nativeTypeId}
  admission.push({item,accepted:belt.nativeInventoryItemCanBindToBelt(item)})
}
for(const kind of ['mod-item','mod-potion']) for(const nativeTypeId of [7001,7004]) {
  const item={kind,nativeSubtype:0,nativeTypeId}
  admission.push({item,accepted:belt.nativeInventoryItemCanBindToBelt(item)})
}
const equipmentSlots=[]
for(const equipmentType of [null,'hat','robe','staff','wand','ring','amulet']) for(const unlocked of [false,true]) {
  equipmentSlots.push({equipmentType,unlocked,slots:belt.nativeBeltEquipmentSlots({equipmentType},unlocked)})
}
writeFixture('belt-goldens.json','tools/website-oracle/belt-goldens.mjs',
  'website-rust-current/frontend/src/game/core-kernels/native-belt.ts',
  {sourceRevision:'fa08ae249ff5563570bdb2a653e2ef9358ce7b83',
    creations,initial,records,migrations,admission,equipmentSlots})
