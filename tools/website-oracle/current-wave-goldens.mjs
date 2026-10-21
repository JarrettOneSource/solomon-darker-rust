#!/usr/bin/env node
// Current compiler output is independent of the retained historical compiler fixture.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const { NATIVE_RETAIL_WAVES } = await loadKernel('core-kernels/native-retail-wave-schedule.ts')
const timeline = await loadKernel('core-kernels/boneyard-wave-timeline.ts')
const native = await loadKernel('core-kernels/native-rng.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const cases = []
for (const seed of ['0', '1', '7', '12345678', 'ffffffff']) {
  let rng = native.createNativeRng(timeline.seedBoneyardWaveRng(`${seed}:wave-compiler`))
  const opening = timeline.compileBoneyardOpening(rng)
  rng = opening.rngState
  const sections = []
  for (const [index, row] of NATIVE_RETAIL_WAVES.entries()) {
    const compiled = timeline.compileBoneyardWaveSection(row, index + 1, rng)
    sections.push(compiled)
    rng = compiled.rngState
  }
  cases.push({ seed, opening, sections })
}

const wave = (entries, spawn = 2) => ({
  groups: entries.map(entries => ({ entries })), maxEnemies: 40, next: [0],
  spawn, spawnDelay: [50, 50], waveDelay: [0, 0],
})
const entry = (enemy, flags = []) => ({ enemy, flags })
const scenarios = []
function record(name, wave, ordinal, rng = native.createNativeRng(1)) {
  let result = null
  let error = null
  try { result = timeline.compileBoneyardWaveSection(wave, ordinal, rng) }
  catch (failure) { error = failure.message }
  scenarios.push({ name, wave, ordinal, rng, result, error })
}
for (const enemy of ['SKELETON', 'ZOMBIE', 'DEMON', 'IMP']) {
  for (const ordinal of [1, 4, 5, 6, 9]) {
    record(`singleton-${enemy}-${ordinal}`, wave([[entry(enemy)]], 1), ordinal)
  }
}
for (const [enemy, flags, ordinal] of [
  ['ZOMBIE', [], 27], ['ZOMBIE', [], 28],
  ['IMP', [], 36], ['IMP', [], 37],
  ['IMP', ['FLAG_SPLIT'], 36], ['IMP', ['FLAG_SPLIT'], 37],
  ['IMP', ['FLAG_SPLITMANY'], 36], ['IMP', ['FLAG_SPLITMANY'], 37],
  ['SKELETON', ['FLAG_PIKE'], 36], ['SKELETON', ['FLAG_PIKE'], 37],
  ['DEMON', [], 28], ['DEMON', [], 37],
]) {
  const rng = { indexA: 0, indexB: 31, words: Array(55).fill(0) }
  rng.words[40] = 64
  record(`first-entry-${enemy}-${flags.join('+')}-${ordinal}`,
    wave([Array.from({length: 3}, () => entry(enemy, flags)),
      Array.from({length: 3}, () => entry('DEMON'))], 4), ordinal, rng)
}
for (const [enemy, flag] of [['SKELETON', 'FLAG_PIKE'], ['IMP', 'FLAG_SPLIT'], ['IMP', 'FLAG_SPLITMANY']]) {
  record(`later-member-${flag}`, wave([[entry(enemy), entry(enemy, [flag])], [entry('DEMON')]], 6), 16,
    native.createNativeRng(0x12345678))
  record(`reset-only-${flag}`, wave([[entry(enemy, [flag])]], 2), 16)
  record(`reset-singleton-${flag}`, wave([[entry(enemy, [flag])]], 1), 16)
  record(`reset-large-${flag}`, wave([Array.from({length: 3}, () => entry(enemy, [flag]))], 2), 16)
}
record('coffin-exhaustion', wave([[entry('COFFIN')]], 2), 16)
record('coffin-singleton', wave([[entry('COFFIN')]], 1), 16)
record('coffin-then-recurring-reset', wave([[entry('COFFIN')], [entry('SKELETON', ['FLAG_PIKE'])]], 2), 16)
const coffinRng = { indexA: 0, indexB: 31, words: Array(55).fill(0) }
coffinRng.words[35] = 64
coffinRng.words[38] = 64
record('coffin-removed-before-zombie', wave([[entry('COFFIN')], [entry('ZOMBIE')]], 2), 16, coffinRng)
const initialRng = { indexA: 0, indexB: 31, words: Array(55).fill(0) }
initialRng.words[32] = 3 * 64
record('initial-scalar-draw-order', wave([Array.from({length: 5}, () => entry('SKELETON'))], 8), 1, initialRng)
writeFixture('current-wave-compiler-goldens.json', 'tools/website-oracle/current-wave-goldens.mjs',
  'core-kernels/boneyard-wave-timeline.ts and native-retail-wave-schedule.ts',
  { sourceRevision, schedule: NATIVE_RETAIL_WAVES, cases, scenarios })
console.log(`recorded ${cases.length * NATIVE_RETAIL_WAVES.length} complete retail sections and ${scenarios.length} boundary/error cases`)
