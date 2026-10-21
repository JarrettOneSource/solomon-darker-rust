import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, writeFixture } from './oracle.mjs'

const { compileModContentCatalog } = await loadKernel('modding/content/mod-content-catalog.ts')
const { PreparedModAssetCatalog } = await loadKernel('modding/assets/mod-asset-compiler.ts')
const { stableWebLuaContentId } = await loadKernel('modding/definition/web-lua-content-identity.ts')
const { ModSkillEngine } = await loadKernel('modding/content/mod-skill-engine.ts')
const { ModShopEngine } = await loadKernel('modding/content/mod-shop-engine.ts')
const { ModSemanticStateEngine } = await loadKernel('modding/content/mod-semantic-state-engine.ts')
const source = JSON.parse(readFileSync(resolve(fixturesDir, 'mod-content-goldens.json'))).cases[0]
const mods = structuredClone(source.mods)
const modId = mods[0].identity.id
const id = key => stableWebLuaContentId(modId, key)
mods[0].content.push({contentId:id('independent'),contentKind:'skill',key:'independent',fields:{art:{icon:{kind:'asset-reference',key:'full'}},ranks:[{modify:{power:{add:2}}},{modify:{power:{multiply:3}}},{modify:{power:{set:11}}}],offer:{weight:0.75}}})
for (const [key, stock_scope, restock] of [['player_stall','player','0ms'],['session_stall','session','10ms']]) {
 const shop = structuredClone(mods[0].content.find(row => row.key === 'stall'))
 shop.contentId=id(key);shop.key=key;shop.fields.stock_scope=stock_scope;shop.fields.restock=restock
 mods[0].content.push(shop)
}
const catalog = compileModContentCatalog(mods, new PreparedModAssetCatalog(source.assets))
const skills = new ModSkillEngine(catalog), shops = new ModShopEngine(catalog, 100), semantic = new ModSemanticStateEngine([modId])
const cases = []
function record(input) {
 let output=null,error=null
 try { switch(input.kind) {
  case 'offer': output=skills.offer(input.playerId,input.level,input.seed,input.count).map(skill=>skill.contentId);break
  case 'choose': output=skills.choose(input.playerId,input.contentId,input.offerSequence);break
  case 'bind': skills.bind(input.playerId,input.slot,input.contentId);break
  case 'unlocked': output=skills.spellUnlocked(input.playerId,input.contentId);break
  case 'filter': output=skills.filter(input.playerId,input.key,input.source);break
  case 'skill-restore': skills.restore(input.checkpoint);break
  case 'purchase': output=shops.purchase(input.playerId,input.shopContentId,input.row,input.gold,input.tick);break
  case 'remaining': output=shops.remaining(input.playerId,input.shopContentId,input.row,input.tick);break
  case 'shop-tick': output=shops.tick(input.tick);break
  case 'shop-restore': shops.restore(input.checkpoint);break
  case 'set': semantic.set(input.modId,input.scope,input.key,input.value);break
  case 'clear': output=semantic.clear(input.modId,input.scope,input.key);break
  case 'semantic-project': output=semantic.project(input.viewerId);break
  case 'semantic-restore': semantic.restore(input.checkpoint);break
 } } catch(caught) { error=caught.message }
 cases.push({input,output,error,skills:skills.checkpoint(),shops:shops.checkpoint(),semantic:semantic.checkpoint()})
}
record({kind:'unlocked',playerId:'a',contentId:id('bolt')})
record({kind:'bind',playerId:'a',slot:0,contentId:id('bolt')})
record({kind:'offer',playerId:'a',level:2,seed:123,count:3})
record({kind:'choose',playerId:'a',contentId:id('training'),offerSequence:skills.offers('a')[0].sequence})
record({kind:'choose',playerId:'a',contentId:id('independent'),offerSequence:skills.offers('a')[0].sequence})
record({kind:'filter',playerId:'a',key:'power',source:5})
record({kind:'offer',playerId:'a',level:3,seed:456,count:3})
record({kind:'choose',playerId:'a',contentId:id('training'),offerSequence:skills.offers('a')[0].sequence+1})
record({kind:'choose',playerId:'a',contentId:id('training'),offerSequence:skills.offers('a')[0].sequence})
record({kind:'unlocked',playerId:'a',contentId:id('bolt')})
record({kind:'bind',playerId:'a',slot:7,contentId:id('bolt')})
record({kind:'bind',playerId:'a',slot:7,contentId:id('bolt')})
record({kind:'bind',playerId:'a',slot:8,contentId:id('bolt')})
record({kind:'filter',playerId:'a',key:'power',source:5})
record({kind:'offer',playerId:'a',level:3,seed:9007199254740991,count:8})
record({kind:'choose',playerId:'a',contentId:id('advanced'),offerSequence:skills.offers('a')[0].sequence})
for(let rank=2;rank<=3;rank++) {
 record({kind:'offer',playerId:'a',level:3,seed:rank,count:8})
 record({kind:'choose',playerId:'a',contentId:id('independent'),offerSequence:skills.offers('a')[0].sequence})
 record({kind:'filter',playerId:'a',key:'power',source:5})
}
record({kind:'bind',playerId:'a',slot:7,contentId:null})
record({kind:'offer',playerId:'a',level:0,seed:1,count:3})
const learned=skills.checkpoint()
record({kind:'skill-restore',checkpoint:learned})
record({kind:'skill-restore',checkpoint:{...learned,ranks:[{playerId:'a',contentId:id('training'),rank:99}]}})
for (const playerId of ['B','á']) {
 record({kind:'offer',playerId,level:3,seed:1,count:3})
 record({kind:'choose',playerId,contentId:id('training'),offerSequence:skills.offers(playerId)[0].sequence})
 record({kind:'bind',playerId,slot:0,contentId:id('bolt')})
}
for(let i=0;i<5;i++) record({kind:'purchase',playerId:i<2?'a':'B',shopContentId:id('stall'),row:0,gold:100,tick:1+i})
record({kind:'remaining',playerId:'a',shopContentId:id('stall'),row:0,tick:54})
record({kind:'shop-tick',tick:54})
record({kind:'purchase',playerId:'a',shopContentId:id('stall'),row:0,gold:19,tick:55})
for(const playerId of ['a','B']) record({kind:'purchase',playerId,shopContentId:id('player_stall'),row:1,gold:100,tick:1})
record({kind:'purchase',playerId:'a',shopContentId:id('player_stall'),row:1,gold:100,tick:10000})
record({kind:'purchase',playerId:'a',shopContentId:id('session_stall'),row:1,gold:100,tick:1})
record({kind:'shop-tick',tick:2})
record({kind:'shop-restore',checkpoint:{revision:0,stock:[{playerId:'*',remaining:1,restockTick:null,row:0,shopContentId:id('player_stall')}]}})
record({kind:'shop-restore',checkpoint:shops.checkpoint()})
for(const [kind,owner,key,value] of [['participant-run','a:run:1','zoom',2],['participant-profile','B','visible',false],['party-run','party:1','visited',['entry','exit']],['session','host','ready',{n:1,flag:true}]]) {
 record({kind:'set',modId,scope:{kind,id:owner},key,value})
}
record({kind:'semantic-project',viewerId:'a'})
record({kind:'semantic-project',viewerId:'B'})
record({kind:'set',modId,scope:{kind:'session',id:'host'},key:'ready',value:{n:2}})
record({kind:'set',modId:'absent',scope:{kind:'session',id:'host'},key:'ready',value:true})
record({kind:'set',modId,scope:{kind:'session',id:'host'},key:'Ready',value:true})
record({kind:'set',modId,scope:{kind:'session',id:'host'},key:'bad',value:{'not valid':1}})
record({kind:'set',modId,scope:{kind:'session',id:'host'},key:'bad',value:'x'.repeat(4097)})
record({kind:'clear',modId,scope:{kind:'session',id:'host'},key:'missing'})
record({kind:'clear',modId,scope:{kind:'session',id:'host'},key:'ready'})
record({kind:'semantic-restore',checkpoint:semantic.checkpoint()})
const invalid=structuredClone(semantic.checkpoint());invalid.values=[...invalid.values,invalid.values[0]]
record({kind:'semantic-restore',checkpoint:invalid})
writeFixture('mod-progression-engines-goldens.json','mod-progression-engines-goldens.mjs','modding/content/mod-skill-engine.ts,mod-shop-engine.ts,mod-semantic-state-engine.ts',{catalog:catalog.all(),modIds:[modId],cases})
