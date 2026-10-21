// Authored Tutorial callout glyph widths, baseline truncation and frame margins.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const source = await loadKernel('tutorial-modal-callouts.ts')
const texts = [source.TUTORIAL_MODAL_TEXT.resume('I'), source.TUTORIAL_MODAL_TEXT.backpack,
  source.TUTORIAL_MODAL_TEXT.quickUseSkills, source.TUTORIAL_MODAL_TEXT.concentration,
  source.TUTORIAL_MODAL_TEXT.tap]
const cases = []
for (const text of texts) for (const center of [{x:800,y:450},{x:412.75,y:320.25},{x:-10.5,y:-12.5}]) {
  cases.push({text,center,expected:source.tutorialCalloutGeometry(text,center.x,center.y)})
}
writeFixture('tutorial-callout-goldens.json', 'tools/website-oracle/tutorial-callout-goldens.mjs',
  `Website@${execFileSync('git',['-C',websiteDir,'rev-parse','HEAD'],{encoding:'utf8'}).trim()}/tutorial-modal-callouts.ts`,
  {cases})
