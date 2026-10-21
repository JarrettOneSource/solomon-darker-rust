#!/usr/bin/env node
// Full descriptor rows, native Welds and the Source's rank/property boundary behavior.
import {loadKernel,writeFixture} from './oracle.mjs'
const skills=await loadKernel('core-kernels/player-progression.ts')
const {describeMlBotPolicySkill,mlBotPolicySkillCoverageKey}=await loadKernel('core-server/ml-bot-policy/skill-descriptors.ts')
const book=structuredClone(skills.createPlayerSkillBook({displayName:'Policy Bot',element:'fire',discipline:'arcane'}))
const statBook=skills.playerStatBook()
for(let id=8;id<83;id++){book.permanentRanks[id]=id%4;book.effectiveRanks[id]=id%5}
const cases=[]
function capture(name,input,override=null){
 const stats=structuredClone(statBook)
 if(override)Object.assign(stats.entries[input.skillId].numericProperties,override)
 try {cases.push({name,input,override,output:Array.from(describeMlBotPolicySkill(book,stats,input)),coverageKey:mlBotPolicySkillCoverageKey(input.skillId,input.weldBuildId)})}
 catch(error){cases.push({name,input,override,error:error.message})}
}
for(let id=0;id<83;id++)for(const targetRank of [0,1,3,99])capture(`skill${id} rank${targetRank}`,{applyCount:Math.max(0,targetRank-(book.permanentRanks[id]??0)),skillId:id,targetRank})
for(const build of skills.NATIVE_WELD_BUILDS)capture(`weld${build.id}`,{applyCount:1,skillId:52,targetRank:3,weldBuildId:build.id})
for(const id of [999,65535,65536])capture(`unknown${id}`,{applyCount:1,skillId:id,targetRank:1})
capture('invalid Weld identity',{applyCount:1,skillId:52,targetRank:1,weldBuildId:9999})
for(const rank of [-1,0.5,1.5,4])capture(`authored stats rank${rank}`,{applyCount:1.5,skillId:16,targetRank:rank},{mManaCost:[5,7,9],mDamage:42.25,mRange:-12,mQuantity:[],mAbsorb:[0,100],mSpeed:3.5,mArmorPlus:[2,4,6],mChance:-0.25})
writeFixture('ml-bot-skills-goldens.json','tools/website-oracle/ml-bot-skills-goldens.mjs','Website core-server/ml-bot-policy/skill-{descriptors,options,loadout,chooser}.ts',{book,cases})
console.log(JSON.stringify({cases:cases.length}))
