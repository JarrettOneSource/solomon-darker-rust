import { loadKernel, writeFixture } from './oracle.mjs'
const material = await loadKernel('renderer/native-scenery-glyph-material.ts')
const cases = []
for (const treeAlpha of [null, 0.4, 0.49999999, 0.5, 0.8]) {
  for (const callback of [[1,1,1,1], [Math.fround(.137),Math.fround(.137),Math.fround(.137),1],
    [Math.fround(Math.fround(.65)*Math.fround(.7)),0,0,Math.fround(.38*Math.fround(.7))]]) {
    const colors = new Uint32Array(4)
    material.writeNativeSceneryGlyphColors(colors, treeAlpha === null ? null : Math.fround(treeAlpha), callback)
    cases.push({treeAlpha,callback,colors:Array.from(colors)})
  }
}
writeFixture('scenery-material-goldens.json','tools/website-oracle/scenery-material-goldens.mjs',
  'Website@fa08ae24/renderer/native-scenery-glyph-material.ts',{cases})
