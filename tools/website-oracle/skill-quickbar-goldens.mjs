import { loadKernel, writeFixture } from './oracle.mjs'
const quickbar = await loadKernel('skill-quickbar.ts')
const cooldowns = []
for (const row of [0,1,10,50]) for (const capacity of [0,1,100]) for (const global of [0,1,20,60]) {
  cooldowns.push({row,capacity,global,expected:quickbar.nativeSkillQuickbarCooldownPresentation(row,capacity,global)})
}
const paths = []
for (const capacity of [0,1,100]) for (const remaining of [0,0.125,1,12.5,25,37.5,50,62.5,75,87.5,99,100,150]) {
  paths.push({remaining,capacity,expected:quickbar.nativeCooldownSectorPath(remaining,capacity)})
}
const alpha = []
const bindings = ['1','7','K','SPACE','ENTER','ESCAPE','F12','SHIFT','RIGHT MOUSE','MIDDLE MOUSE','NUMPAD 0'].map(text=>{
  const layout=quickbar.layoutNativeQuickbarBinding(text)
  return {text,advance:layout.advance,backingLeft:layout.backingLeft,backingWidth:layout.backingWidth}
})
for (const cooldown of [false,true]) for (const unavailable of [false,true]) {
  alpha.push({cooldown,unavailable,expected:quickbar.nativeSkillQuickbarIconAlpha({cooldown,unavailable})})
}
writeFixture('skill-quickbar-goldens.json','tools/website-oracle/skill-quickbar-goldens.mjs',
  'Website/frontend/src/game/skill-quickbar.ts',{cooldowns,paths,alpha,bindings})
