// Actual stock notice DataLines through the Source's MsgBox wrapping and layout.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const contract = await loadKernel('renderer/hub-inventory-render-contract.ts')
const plan = await loadKernel('native-ui/native-ui-plan.ts')
const notices = [contract.HUB_DOWSING_INSUFFICIENT_GOLD, contract.hubHagathaFullMindNotice(27),
  contract.hubHagathaFullMindNotice(26), contract.HUB_HAT_REMOVAL_MSGBOX, contract.HUB_ROBE_REMOVAL_MSGBOX]
const inputs = notices.map(notice => ({name:notice.title,anchor:{x:800,y:notice.nativeAnchorY},lines:notice.nativeLines}))
inputs.push({name:'native-overflow-carry-and-hyphen',anchor:{x:801.5,y:451.25},lines:[
  {font:'menu',gapAfter:10,text:'THE NATIVE MESSAGE'},
  {font:'medium',text:'A'.repeat(60)+'  next-word\r\nlast line'},
]})
const cases = inputs.map(input => ({...input,expected:plan.layoutNativeUiSingleActionMessage({
  anchorX:input.anchor.x,anchorY:input.anchor.y,height:900,width:1600,lines:input.lines,
})}))
writeFixture('native-ui-message-goldens.json','tools/website-oracle/native-ui-message-goldens.mjs',
  `Website@${execFileSync('git',['-C',websiteDir,'rev-parse','HEAD'],{encoding:'utf8'}).trim()}/native-ui/native-ui-plan.ts`,{cases})
