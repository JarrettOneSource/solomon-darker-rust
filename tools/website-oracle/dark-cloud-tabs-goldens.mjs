import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { resolve } from 'node:path'
import { gameDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const require = createRequire(resolve(websiteDir, 'frontend/package.json'))
const ts = require('typescript')
const { transformSync } = require('esbuild')
const contract = await loadKernel('native-ui/native-dark-cloud-contract.ts')
const { nativeUiRect } = await loadKernel('native-ui/native-ui-plan.ts')
const file = resolve(gameDir, 'native-ui/NativeDarkCloudPresentation.tsx')
const source = ts.createSourceFile(file, readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX)
const declaration = source.statements.find(node => ts.isFunctionDeclaration(node) && node.name?.text === 'NativeDarkCloudTabs')
if (!declaration) throw new Error('The actual Source NativeDarkCloudTabs declaration was not found')
let minimumScale
for (const statement of source.statements) {
  if (!ts.isVariableStatement(statement)) continue
  for (const declaration of statement.declarationList.declarations) {
    if (declaration.name.getText(source) === 'MINIMUM_TAB_SCALE') {
      minimumScale = Number(declaration.initializer.getText(source))
    }
  }
}
if (!Number.isFinite(minimumScale)) throw new Error('Source minimum tab scale was not found')
// Run the actual component body. Only DOM measurement/ref/effect I/O is supplied;
// its scale, scroll extent and every native band calculation remain unchanged.
const code = transformSync(declaration.getText(source).replace(/^export /, ''), {loader:'tsx', jsx:'transform', jsxFactory:'React.createElement'}).code
const React = {createElement:(type, props, ...children) => ({type, props:{...props, children}})}
let size
const component = new Function('React', 'useRef', 'useLayoutEffect', 'useNativeUiElementSize',
  'NATIVE_DARK_CLOUD_TAB_STRIP', 'NATIVE_DARK_CLOUD_TABS', 'NativeUiTabs', 'MINIMUM_TAB_SCALE', 'nativeUiRect',
  code + '\nreturn NativeDarkCloudTabs;')(
  React, () => ({current:null}), () => {}, () => size,
  contract.NATIVE_DARK_CLOUD_TAB_STRIP, contract.NATIVE_DARK_CLOUD_TABS, 'NativeUiTabs', minimumScale, nativeUiRect)
const frames = []
for (const [width,height] of [[1122,69],[1490,69],[882,69],[768,58],[390,58],[366,44],[290,44],[100,44],[449,69],[448.8,69],[448,69],[1122,20]]) {
  for (const selected of contract.NATIVE_DARK_CLOUD_TABS) {
    size = {width,height}
    const nav = component({selectedId:selected.id, onSelect(){}})
    const plan = nav.props.children[0].props
    frames.push({width,height,selected:selected.id,expected:{width:plan.width,height:plan.height,scale:plan.scale,tabs:plan.tabs.map(({id,label,bounds}) => ({id,label,bounds}))}})
  }
}
writeFixture('dark-cloud-tabs-goldens.json','dark-cloud-tabs-goldens.mjs',
  'Current Website fa08ae249ff5563570bdb2a653e2ef9358ce7b83 NativeDarkCloudPresentation.tsx NativeDarkCloudTabs executed unchanged with supplied measured-size hook input; native-dark-cloud-contract.ts all five bands',
  {strip:contract.NATIVE_DARK_CLOUD_TAB_STRIP,tabs:contract.NATIVE_DARK_CLOUD_TABS,frames})
