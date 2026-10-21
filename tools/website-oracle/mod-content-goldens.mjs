// The prepared catalog boundary consumes compiled rows and accepted asset descriptors.
// These cases exercise that boundary, independently of Lua evaluation/graph compilation.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { createHash } from 'node:crypto'
import { fixturesDir, loadKernel, writeFixture } from './oracle.mjs'

const { compileModContentCatalog } = await loadKernel('modding/content/mod-content-catalog.ts')
const { PreparedModAssetCatalog, compileModAssets } = await loadKernel('modding/assets/mod-asset-compiler.ts')
const { compileWebLuaDefinition } = await loadKernel('modding/definition/web-lua-definition-compiler.ts')
const { stableWebLuaContentId } = await loadKernel('modding/definition/web-lua-content-identity.ts')
const assetFixture = JSON.parse(readFileSync(resolve(fixturesDir, 'mod-assets-goldens.json'), 'utf8'))
const assets = ['full', 'wav', 'music', 'scene', 'yard'].map(name => assetFixture.cases.find(row => row.name === name).output[0])
const identity = { id:'asset.test', name:'Catalog Test', version:'1.0.0' }
const source = { file:'scripts/main.lua', line:1, column:0 }
const png = new Uint8Array(24)
png.set([0x89,0x50,0x4e,0x47,0x0d,0x0a,0x1a,0x0a])
const view = new DataView(png.buffer)
view.setUint32(8,13); png.set(new TextEncoder().encode('IHDR'),12)
view.setUint32(16,4080); view.setUint32(20,850)
const assetMod = compileWebLuaDefinition(identity, {
  api:'1.0.0', assets:[{ kind:'asset-definition',assetKind:'sheet',key:'worn',source,
    fields:{image:'art/worn.png',frame:{width:170,height:170},headings:24,animations:{idle:[1],walk:[1,2,3,4,5]}} }],
  content:[],reducers:[],rules:[],
})
assets.push(...compileModAssets({
  assets:[{byteLength:png.length,contentType:'image/png',kind:'image',modId:identity.id,path:'art/worn.png',sha256:createHash('sha256').update(png).digest('hex')}],
  mods:[assetMod],sources:[{identity,files:{'art/worn.png':png}}],
}).all())
const art = key => ({kind:'asset-reference',key})
const ref = (key,targetKind) => ({kind:'resolved-content-reference',contentId:stableWebLuaContentId(identity.id,key),key,modId:identity.id,targetKind})
const rule = (operation='effect.heal') => ({kind:'rule-definition',operation,fields:{amount:5},source})
const content = []
function add(contentKind,key,fields) { content.push({contentId:stableWebLuaContentId(identity.id,key),contentKind,key,fields}) }
add('status','ward',{duration:'1.25s',stacking:'stack',modifiers:{incoming_damage:{multiply:0.5},mana_spend:{add:-2}}})
add('potion','tonic',{art:{icon:art('full')},duration:'2m',on_use:rule(),status:ref('ward','status'),loot:{ordinary:0.25,boss:0.75},presentation:{consume_vfx:{kind:'spell_glow',color:[0.1,0.2,0.3,0.4]}}})
add('item','token',{art:{icon:art('full')},stack:{maximum:12},use:rule()})
add('item','robe',{art:{icon:art('full'),icon_trim:art('full'),worn:art('worn'),worn_trim:art('worn')},equipment:{slot:'robe',dyeable:true,death_shape:2,tints:{cloth:123,trim:456}}})
add('powerup','orb',{art:{world:art('full'),sound:art('wav')},effect:rule(),pickup:{radius:16}})
add('affix','lucky',{applies_to:['robe','wand'],modifiers:{mana_spend:{set:0}}})
add('affix-pool','pool',{entries:[{affix:ref('lucky','affix'),weight:0.25}],applies_to:['robe'],rng_domain:'test.pool',rolls:3})
add('skill','training',{art:{icon:art('full')},ranks:[{modify:{power:{multiply:1.5}},grants:[ref('bolt','spell'),ref('map','ui')]},{grant:ref('bolt','spell')}],maximum_rank:1,offer:{minimum_level:3,weight:0.125},grants:ref('bolt','spell')})
add('skill','advanced',{art:{icon:art('full')},ranks:[{}],parent:ref('training','skill'),prerequisites:[ref('training','skill')]})
add('spell','bolt',{art:{icon:art('full'),effect:art('full'),sound:art('wav')},behavior:rule('prefab.projectile'),slot:'primary',mana:10,cooldown:'2.5ms'})
add('enemy','mote',{art:{atlas:art('full'),sound:art('wav'),attack_sound:art('wav'),death_sound:art('wav')},stats:{health:123,damage:4,speed:3,attack_range:90,attack_cooldown:'0.25s',collision_radius:9,scale:0.75},loot:{gold:{minimum:3,maximum:7},experience:11.5}})
add('boneyard','yard',{art:{layout:art('yard'),music:art('music')},source:'levels/yard.boneyard',roster:['Zombie',ref('mote','enemy')],waves:[{ordinal:null,roster:[ref('mote','enemy')]}],triggers:[rule()],environment:{mode:2},anchors:{entry:{x:-12,y:34}}})
add('boast','vow',{stock_icon:2,instruction:'Avoid potions',response:'Yes',statement:'I will abstain',fail_on:['potion-use','secondary-cast']})
add('boast','painted-vow',{art:{icon:art('full')},instruction:'Stay alert',response:'Indeed',statement:'I will watch',random_skill_choices:true,score_multiplier:1.75,success_wave:5})
add('room','first',{geometry:{kind:'inline',width:640,height:480,floor:'#12AbCD',walls:[{x:-4,y:3,width:5,height:8,color:{source:'unvalidated'}}]},props:[{x:12,y:34,radius:5,color:8,kind:false,label:null}]})
add('scene','hall',{art:{layout:art('scene'),ambience:art('wav')},rooms:[ref('first','room')]})
add('scene-extension','extra',{scene:'stock.boneyard',features:[rule()]})
add('shop','stall',{stock:[{item:ref('token','item'),price:20,quantity:4},{item:ref('tonic','potion'),price:3},{item:ref('robe','item'),price:50}],services:[{type:'reforge',pool:ref('pool','affix-pool'),price:70}],mount:{scene:'hub.courtyard',x:20,y:40,radius:90},npc:{name:'Shopkeeper'},restock:'500ms',stock_scope:'party'})
add('ui','map',{mount:'hud.top_right',view:rule('prefab.minimap'),bindings:{zoom:{state:'map.zoom'}},visible:{scenes:['hub','boneyard','room'],state:{state:'map.visible',equals:true}},accessible_name:'Map',actions:['zoom-in','zoom-out']})
const emptyMod = compileWebLuaDefinition(identity,{api:'1.0.0',assets:[],content:[],reducers:[],rules:[]})
const base = [{...structuredClone(emptyMod),content}]
const cases = []
function record(name,mods,prepared=assets) {
  let output=null,error=null
  try { output=compileModContentCatalog(mods,new PreparedModAssetCatalog(prepared)).all() }
  catch(caught) { error=caught.message }
  cases.push({name,mods,assets:prepared,output,error})
}
function change(name,key,mutate) {
  const mods=structuredClone(base)
  mutate(mods[0].content.find(row=>row.key===key).fields,mods)
  record(name,mods)
}
record('all-sixteen-families',base)
const defaults=structuredClone(base)
for(const entry of defaults[0].content) {
  const fields=entry.fields
  delete fields.description; delete fields.name
  if(entry.contentKind==='status') { delete fields.duration;delete fields.modifiers;delete fields.stacking }
  if(entry.contentKind==='potion') { delete fields.presentation;delete fields.loot;delete fields.status }
  if(entry.key==='token') { delete fields.stack;delete fields.use }
  if(entry.key==='robe') { fields.equipment={slot:'robe'};delete fields.art.icon_trim;delete fields.art.worn_trim }
  if(entry.contentKind==='powerup') delete fields.pickup
  if(entry.contentKind==='affix') { delete fields.applies_to;delete fields.modifiers }
  if(entry.contentKind==='affix-pool') { delete fields.applies_to;delete fields.rng_domain;delete fields.rolls;delete fields.entries[0].weight }
  if(entry.contentKind==='skill') { delete fields.offer;delete fields.maximum_rank;delete fields.parent;delete fields.prerequisites;delete fields.grants }
  if(entry.contentKind==='spell') { delete fields.mana;delete fields.cooldown;fields.slot='secondary' }
  if(entry.contentKind==='enemy') { delete fields.stats;delete fields.loot }
  if(entry.contentKind==='boneyard') { delete fields.roster;delete fields.waves;delete fields.triggers;delete fields.environment;delete fields.anchors }
  if(entry.contentKind==='shop') { delete fields.mount;delete fields.npc;delete fields.restock;delete fields.stock_scope;delete fields.services }
  if(entry.contentKind==='ui') { delete fields.bindings;delete fields.visible;delete fields.actions;delete fields.accessible_name }
}
record('source-defaults',defaults)
change('art-unknown-slot','tonic',f=>f.art.unknown=art('full'))
change('art-explicit-null','token',f=>f.art=null)
change('art-unavailable','token',f=>f.art.icon=art('absent'))
change('description-byte-limit','token',f=>f.description='é'.repeat(513))
change('status-modifier-operation','ward',f=>f.modifiers.incoming_damage={add:1,multiply:2})
change('status-modifier-lane','ward',f=>f.modifiers.extra=1)
change('status-stacking-null','ward',f=>f.stacking=null)
change('duration-leading-zero','tonic',f=>f.duration='01s')
change('duration-limit','tonic',f=>f.duration='24.01h')
change('potion-missing-rule','tonic',f=>delete f.on_use)
change('potion-status-unavailable','tonic',f=>f.status=ref('missing','status'))
change('potion-vfx-component','tonic',f=>f.presentation.consume_vfx.color[2]=2)
change('item-stack-null','token',f=>f.stack=null)
change('item-use-null','token',f=>f.use=null)
change('wearable-stack','robe',f=>f.stack={maximum:2})
change('wearable-trim-pair','robe',f=>delete f.art.icon_trim)
change('wearable-consume','robe',f=>f.use=rule())
change('wearable-sheet','robe',f=>f.art.worn=art('full'))
change('staff-dye-layers','robe',f=>f.equipment.slot='staff')
change('powerup-sound-kind','orb',f=>f.art.sound=art('music'))
change('powerup-radius','orb',f=>f.pickup.radius=0)
change('affix-equipment-type','lucky',f=>f.applies_to=['sword'])
change('text-values-before-duplicates','lucky',f=>f.applies_to=['robe','robe',7])
change('affix-pool-unavailable','pool',f=>f.entries[0].affix=ref('absent','affix'))
change('affix-pool-zero-weight','pool',f=>f.entries[0].weight=0)
change('skill-missing-parent','advanced',f=>f.parent=ref('absent','skill'))
change('skill-wrong-grant-family','training',f=>f.grants=ref('mote','enemy'))
change('skill-later-rank-validation','training',f=>f.ranks[1].modify={power:{add:'bad'}})
change('spell-wrong-slot','bolt',f=>f.slot='tertiary')
change('spell-effect-kind','bolt',f=>f.art.effect=art('wav'))
change('enemy-gold-order','mote',f=>f.loot.gold.maximum=2)
change('enemy-collision-radius','mote',f=>f.stats.collision_radius=3)
change('boneyard-layout-source','yard',f=>f.source='levels/other.boneyard')
change('boneyard-wave-roster','yard',f=>f.waves[0].roster=[7])
change('boast-icon-exclusivity','vow',f=>f.art={icon:art('full')})
change('boast-empty-instruction','vow',f=>f.instruction='')
change('boast-failure-producer','vow',f=>f.fail_on=['unknown'])
change('room-invalid-wall','first',f=>f.geometry.walls[0].width=0)
change('room-invalid-floor','first',f=>f.geometry.floor='#12345')
change('scene-unavailable-room','hall',f=>f.rooms=[ref('absent','room')])
change('scene-extension-target','extra',f=>f.scene='hub.courtyard')
change('shop-wearable-count','stall',f=>f.stock[2].quantity=2)
change('shop-reforge-pool','stall',f=>f.services[0].pool=ref('absent','affix-pool'))
change('shop-mount-null','stall',f=>f.mount=null)
change('ui-empty-binding','map',f=>f.bindings.zoom.state='')
change('ui-wrong-view','map',f=>f.view=rule())
change('ui-duplicate-actions','map',f=>f.actions=['zoom','zoom'])
const duplicate=structuredClone(base)
duplicate[0].content.push(structuredClone(duplicate[0].content.find(row=>row.key==='ward')))
record('duplicate-content-id',duplicate)
writeFixture('mod-content-goldens.json','mod-content-goldens.mjs','modding/content/mod-content-catalog.ts',{cases})
console.log(JSON.stringify({cases:cases.length,accepted:cases.filter(row=>row.error===null).length,rejected:cases.filter(row=>row.error!==null).length}))
