import { loadKernel, writeFixture } from './oracle.mjs'
const timing = await loadKernel('renderer/level-up-presentation.ts')
const picker = await loadKernel('renderer/skill-picker-render-contract.ts')
const profile = await loadKernel('core-kernels/player-progression.ts')
const reveals = [-1,0,9,10,200,390,400,500].map(elapsed=>({elapsed,expected:timing.nativeSkillPickerReveal(elapsed)}))
const closes = [-1,-.75].flatMap(direction=>[0,10,200,400,540].map(elapsed=>({elapsed,direction,expected:timing.nativeSkillPickerClose(elapsed,direction)})))
const cards = profile.NATIVE_SKILL_CATALOG.filter(skill=>skill.id>=8&&skill.id<=79&&skill.id!==profile.SPELL_WELDING_SKILL_ID).map(skill=>({skillId:skill.id,targetRank:3,insight:false}))
for(const build of profile.NATIVE_WELD_BUILDS) cards.push({skillId:profile.SPELL_WELDING_SKILL_ID,targetRank:1,insight:true,weldBuildId:build.id})
writeFixture('skill-picker-goldens.json','tools/website-oracle/skill-picker-goldens.mjs',
 'Website/frontend/src/game/renderer/skill-picker-render-contract.ts and level-up-presentation.ts',{
 reveals,closes,cards:cards.map(option=>({option,expected:picker.skillPickerCardPresentation(option)})),
 layouts:[3,4].map(count=>({count,panel:picker.skillPickerPanelBounds(count),centers:picker.skillPickerCardCenters(count)}))})
