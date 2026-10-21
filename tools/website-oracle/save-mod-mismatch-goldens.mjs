import { loadKernel, writeFixture } from './oracle.mjs'

const { gameSaveModMismatch } = await loadKernel('save/game-save-mods.ts')
const mod = (id, version = '1.0', contentSha256 = 'abc123') => ({id, version, contentSha256})
const pairs = [
  [[], []],
  [[mod('one')], [mod('one')]],
  [[mod('ONE', '1.0', 'ABC123')], [mod('one')]],
  [[], [mod('one'), mod('two')]],
  [[mod('one'), mod('two')], []],
  [[mod('one')], [mod('one', '2.0')]],
  [[mod('one')], [mod('one', '1.0', 'new')]],
  [[mod('one')], [mod('one', '2.0', 'new')]],
  [[mod('one'), mod('two')], [mod('two'), mod('one')]],
  [[mod('one'), mod('old'), mod('two')], [mod('new'), mod('two', '2.0'), mod('one')]],
  [[mod('one', '1'), mod('ONE', '2')], [mod('One', '2')]],
  [[mod('one', '1')], [mod('one', '2'), mod('ONE', '1')]],
  [[mod('one', '1')], [mod('one', '1'), mod('ONE', '2')]],
  [[], [mod('one'), mod('ONE')]],
  [[mod('one'), mod('ONE')], []],
  [[mod('Σ'), mod('İ')], [mod('σ'), mod('i\u0307')]],
  [[mod('ΟΣ')], [mod('ος')]],
]
writeFixture('save-mod-mismatch-goldens.json', 'tools/website-oracle/save-mod-mismatch-goldens.mjs',
  'Website/frontend/src/game/save/game-save-mods.ts', {
    cases: pairs.map(([saved, active]) => ({saved, active, expected: gameSaveModMismatch(saved, active)})),
  })
