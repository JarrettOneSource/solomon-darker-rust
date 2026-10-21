import {loadKernel,writeFixture} from './oracle.mjs'
const source=await loadKernel('cheat-menu-contract.ts')
const outcome=action=>{try{return {ok:true,value:source.compileCheatMenuAction(action)}}catch(error){return {ok:false,error:error.message}}}
const playerId="wizard\\'\n\r\t🙂"
const actions=[{kind:'restore-health',playerId},{kind:'restore-mana',playerId}]
actions.push({kind:'spawn-next-wave'})
for(const [kind,key,min,max] of [['set-gold','gold',0,source.CHEAT_MENU_GOLD_MAX],['grant-experience','amount',0,source.CHEAT_MENU_EXPERIENCE_MAX],['set-run-seed','seed',1,source.CHEAT_MENU_RUN_SEED_MAX],['spawn-enemy','count',1,source.CHEAT_MENU_SPAWN_COUNT_MAX],['grant-item','quantity',1,100],['grant-skill','ranks',1,100],['grant-weld','buildId',1000,1009]]){
 for(const value of [min-1,min,max,max+1,1.5,NaN,Infinity])actions.push({kind,playerId,enemyKey:'Dark Mage',itemKey:'novice_wand',skillId:8,[key]:value})
}
for(const skillId of [7,8,79,80])actions.push({kind:'grant-skill',playerId,skillId,ranks:1})
for(const discipline of source.CHEAT_MENU_BOT_DISCIPLINES)for(const element of source.CHEAT_MENU_BOT_ELEMENTS)actions.push({kind:'summon-bot',discipline,element})
for(const discipline of ['air','earth','ether','fire','water'])actions.push({kind:'summon-bot',discipline,element:'ether'})
for(const value of ['', 'x'.repeat(128), 'x'.repeat(129), '🙂'.repeat(64), '🙂'.repeat(65)])actions.push({kind:'restore-health',playerId:value})
const commands=actions.map(action=>({action,expected:outcome(action)}))
const authority=[]
for(const developerAccess of [false,true])for(const cheatsEnabled of [false,true])for(const isHost of [false,true])authority.push({developerAccess,cheatsEnabled,isHost,expected:source.gameCheatMenuAvailable({developerAccess,cheatsEnabled,isHost})})
const valid=[[{base:'dark_mage',key:'mage',native_type_id:77}],[{key:'ring',kind:'equipment',name:'Ring',native_type_id:39}],[{family:'body',id:8,maximum_rank:7,name:'Skill',weld_only:false}],[{component_skill_ids:[8,9],id:1000,name:'Weld'}]]
const catalogInputs=[valid,[[],{},{},{}],[{},{},{},{}],[],[null,[],[],[]]]
for(const [index,field,bad] of [[0,'base',''],[0,'key',3],[0,'native_type_id',1.5],[0,'native_type_id',Number.MAX_SAFE_INTEGER+1],[1,'kind',null],[2,'family',false],[2,'maximum_rank','3'],[2,'weld_only',0],[3,'component_skill_ids',[8,1.5]],[3,'component_skill_ids',{}],[3,'id',-7]]){
 const values=structuredClone(valid);values[index][0][field]=bad;catalogInputs.push(values)
}
for(let index=0;index<4;index++){const values=structuredClone(valid);values[index]=[[]];catalogInputs.push(values)}
const catalogs=catalogInputs.map(values=>{try{return {values,expected:{ok:true,value:source.decodeCheatMenuCatalogs(values)}}}catch(error){return {values,expected:{ok:false,error:error.message}}}})
let current=[];const history=[]
for(const code of ['', '\ufeff a \ufeff', 'a',' \u0085 ',...Array.from({length:55},(_,index)=>` return ${index} `), 'return 54']){
 current=source.appendCheatConsoleHistory(current,code);history.push({code,expected:current})
}
writeFixture('cheat-menu-goldens.json','cheat-menu-goldens.mjs','Current Website fa08ae249ff5563570bdb2a653e2ef9358ce7b83 cheat-menu-contract.ts',{authority,commands,catalogs,history,catalogQuery:source.CHEAT_MENU_CATALOG_QUERY})
