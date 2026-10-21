#!/usr/bin/env node
import {loadKernel,writeFixture} from './oracle.mjs'
const source=await loadKernel('hud-skill-selector.ts')
const options=[]
for(const binding of [12,16,20]) for(const slots of [[null,null],[60,70],[70,60],[null,60]]) {
 for(const weldBuildId of [null,1000]) for(const exhausted of [false,true]) {
  const target=source.nativeHudSkillSelectorTarget(binding)
  const learnedSkills=[80,70,69,65,60,54,52,50,48,32,24,21,16,8].map(id=>[id,1,exhausted&&id===8?0:1])
  const progression={learnedSkills,concentrationSkillIds:slots,weldBuildId}
  options.push({binding,slots,weldBuildId,learnedSkills,title:source.nativeHudSkillSelectorTitle(target),
   options:source.nativeHudSkillSelectorOptions(progression,target)})
 }
}
const layouts=[]
for(const count of [0,1,6,20]) for(const width of [0,190,400]) {
 layouts.push({count,width,layout:source.nativeHudSkillSelectorLayout(count,width)})
}
writeFixture('hud-skill-selector-goldens.json','tools/website-oracle/hud-skill-selector-goldens.mjs',
 ['hud-skill-selector.ts'],{options,layouts})
