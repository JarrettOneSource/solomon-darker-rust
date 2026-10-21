import { loadKernel, writeFixture } from './oracle.mjs'
const notice = await loadKernel('native-ui/native-ui-notebox.ts')
const texts = ['', 'FAILED "A PAUCITY OF POTIONS"', 'ONE LINE\nTWO LINES', 'I will keep this promise.', 'ÁéΩ 🙂', '  spaced   text  ']
const layouts = texts.map(text => ({ text, expected: notice.nativeNoteboxLayout(text) }))
const frames = []
for (const kind of ['failure', 'instruction']) {
  for (const elapsed of [-1, 0, 25, 50, 99, 100, 101, 4999, 5000, 5100, 5200, 9999, 10000, 10100, 10200, 20000]) {
    for (const dismissed of [null, 0, 50, 5000]) {
      frames.push({kind, elapsed, dismissed, duration: notice.nativeNoteboxDurationMs(kind), opacity: notice.nativeNoteboxOpacity(kind, elapsed, dismissed)})
    }
  }
}
writeFixture('native-notebox-goldens.json', 'tools/website-oracle/native-notebox-goldens.mjs', 'Website native-ui/native-ui-notebox.ts and shared bitmap font layout', { layouts, frames })
console.log(`${layouts.length} layouts and ${frames.length} notice frames`)
