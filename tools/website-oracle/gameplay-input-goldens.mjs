import { loadKernel, writeFixture } from './oracle.mjs'
const pads = await loadKernel('input/standard-gamepad.ts')
const pointer = await loadKernel('input/gameplay-pointer.ts')
const pad = (index, buttons=[], axes=[0,0,0,0]) => ({index,connected:true,mapping:'standard',axes,
  buttons:Array.from({length:16},(_,i)=>({pressed:buttons.includes(i),value:buttons.includes(i)?1:0}))})
const steps = [[],[pad(1,[],[1,0,0,0])],[pad(1,[],[0,0,.7,-.4])],[pad(1)],
  [pad(1,[7])],[pad(1,[2,5,7])],[pad(1,[2])],[pad(1,[5])],[pad(1)],
  [pad(1,[5])],[pad(1)],[pad(1,[4])],[pad(1)],[pad(1,[4,5])],
  [pad(1,[0,3,8,9])],[pad(1,[0,3,8,9])],[pad(1)],[pad(1),pad(2,[7])],
  [pad(2,[7])],[pad(2)],[pad(2,[5])],[pad(2,[2])],{clear:true},
  [pad(2,[2,7])],[pad(2)],[pad(2,[7])],[pad(2,[],[0,0,-1,0])],[pad(2)],[]]
const state=pads.createStandardGamepadGameplayState()
const trace=steps.map(step=>{
  if (step.clear) {state.clear();return{clear:true}}
  const result=state.sample(step)
  return {gamepads:step.map(p=>({index:p.index,buttons:p.buttons.flatMap((b,i)=>b.pressed?[i]:[]),axes:p.axes})),expected:{
    actions:result.actions,aimActive:result.aimActive,aimDirection:result.aimDirection,
    gamepadIndex:result.gamepad?.index??null,primary:result.primary,quickbar:result.quickbar,
    quickbarSelectionChanged:result.quickbarSelectionChanged,selectedQuickbarSlot:result.selectedQuickbarSlot}}
})
const pointers=[]
for(const [width,height] of [[1600,900],[390,219.375],[1920,1080]]) {
  for(const position of [[0,0],[width/2,height/2],[width,height],[width+20,-10]]) {
    const args=[{x:position[0]+31,y:position[1]+49},{left:31,top:49,width,height},{width:1600,height:900},{x:100,y:-30},2.25]
    pointers.push({args,expected:pointer.projectNativeWorldPointer(...args)})
  }
}
const sticks=[]
for(const direction of [[1,0],[0,-1],[.7,.2],[0,0]]) {
  const args=[{x:direction[0],y:direction[1]},{x:800,y:420},{width:1600,height:900},2.25]
  sticks.push({args,expected:pointer.projectNativeStickAim(...args)})
}
writeFixture('gameplay-input-goldens.json','tools/website-oracle/gameplay-input-goldens.mjs',
 'Website/frontend/src/game/input/standard-gamepad.ts and gameplay-pointer.ts',{trace,pointers,sticks})
