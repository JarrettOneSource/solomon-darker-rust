// Existing catalogs plus real queued Console commands over the Source game.
import { readFileSync } from 'node:fs'
import { loadKernel, writeFixture } from './oracle.mjs'
const simulation = await loadKernel('core-server/game-simulation.ts')
const { applyWebLuaCommands, createWebLuaFrameState } = await loadKernel('host/lua/web-lua-game-api.ts')
const catalogs = await loadKernel('host/lua/web-lua-developer-grants.ts')
const fixture = JSON.parse(readFileSync(new URL('../../crates/sim/tests/fixtures/website/mod-host-game-goldens.json',import.meta.url)))
let state=simulation.createGameSimulation({'player-1':{discipline:'arcane',displayName:'Tester',element:'ether'}})
const cases=[]
function capture(input) {
  let output=null
  if(input.kind==='commands') {
    const applied=applyWebLuaCommands(state,input.commands)
    state=applied.state
    output={enemySpawnIntents:applied.enemySpawnIntents,nextRunSeed:applied.nextRunSeed}
  } else if(input.kind==='select') state=simulation.selectGameSimulationPlayerSkill(state,'player-1',input.choice)
  else if(input.kind==='enter') state=simulation.enterBoneyardWorld(state,fixture.loaded)
  else if(input.kind==='items') output=catalogs.createWebLuaDeveloperGrantItems(input.key,input.quantity)
  const player=simulation.getPlayerCharacter(state,'player-1'),economy=simulation.getPlayerEconomy(state,'player-1'),book=state.playerEntities.skillBooks[0]
  cases.push({input,output,frame:createWebLuaFrameState(state,'player-1',state.world.kind==='boneyard'?fixture.loaded:null),backpack:economy.backpack,
    book:{permanentRanks:book.permanentRanks,effectiveRanks:book.effectiveRanks,learnedSkillOrder:book.learnedSkillOrder,advancedUnlocks:book.advancedUnlocks,primarySkillId:book.primarySkillId,weldBuildId:book.weldBuildId,weldComponentRanks:book.weldComponentRanks},
    rng:state.gameRng,offer:simulation.getPlayerProgression(state,'player-1').pendingOffer,primary:player.primaryCast})
}
capture({kind:'commands',commands:[]})
for(const [key,quantity] of [['health-potion',3],['sack',2],['skill-choice-book',2],['equipment:0',1],['equipment:46',1],['missing',1],['health-potion',0],['health-potion',101]]) capture({kind:'items',key,quantity})
for(const command of [
  {type:'set-gold',playerId:'player-1',value:1234},
  {type:'grant-gold',playerId:'player-1',amount:250},
  {type:'set-mana',playerId:'player-1',value:20},
  {type:'restore-mana',playerId:'player-1',amount:30},
  {type:'restore-health',playerId:'player-1',amount:10},
  {type:'grant-item',playerId:'player-1',itemKey:'health-potion',quantity:3},
  {type:'grant-item',playerId:'player-1',itemKey:'skill-choice-book',quantity:2},
  {type:'grant-item',playerId:'player-1',itemKey:'equipment:0',quantity:1},
  {type:'grant-experience',playerId:'player-1',amount:91},
  {type:'grant-skill',playerId:'player-1',skillId:72,ranks:2},
  {type:'grant-skill',playerId:'player-1',skillId:72,ranks:2},
  {type:'grant-skill',playerId:'player-1',skillId:52,ranks:1},
  {type:'grant-weld',playerId:'player-1',buildId:1000},
  {type:'grant-weld',playerId:'player-1',buildId:1000},
  {type:'grant-item',playerId:'player-1',itemKey:'missing',quantity:1},
  {type:'grant-gold',playerId:'missing',amount:500},
  {type:'grant-gold',playerId:'player-1',amount:-1},
  {type:'set-next-run-seed',seed:42},
  {type:'spawn-enemy',token:'SKELETON',requestId:1,x:200,y:300},
]) capture({kind:'commands',commands:[command]})
const pending=simulation.getPlayerProgression(state,'player-1').pendingOffer
capture({kind:'select',choice:{choiceIndex:0,offerSequence:pending.sequence,skillId:pending.options[0].skillId,weldBuildId:pending.options[0].weldBuildId}})
capture({kind:'enter'})
capture({kind:'commands',commands:[{type:'spawn-enemy',token:'SKELETON',requestId:2,x:200,y:300},{type:'set-next-run-seed',seed:80},{type:'set-next-run-seed',seed:90}]})
const defaultLoaded={...fixture.loaded,choice:{id:'default-random',source:'default',name:'Host Game'}}
const defaultGame=simulation.enterBoneyardWorld(simulation.createGameSimulation({'player-1':{discipline:'arcane',displayName:'Tester',element:'ether'}}),defaultLoaded)
const defaultFrame=createWebLuaFrameState(defaultGame,'player-1',defaultLoaded)
writeFixture('web-lua-commands-goldens.json','web-lua-commands-goldens.mjs','website-rust-current/host/lua/web-lua-{developer-grants,game-api}.ts',
  {sourceRevision:'fa08ae249ff5563570bdb2a653e2ef9358ce7b83',loaded:fixture.loaded,defaultFrame,items:catalogs.WEB_LUA_DEVELOPER_ITEMS,skills:catalogs.WEB_LUA_DEVELOPER_SKILLS,welds:catalogs.WEB_LUA_DEVELOPER_WELDS,cases})
