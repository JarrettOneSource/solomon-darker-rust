import { loadKernel, writeFixture } from './oracle.mjs'

const source = await loadKernel('renderer/skill-book-render-contract.ts')
const profile = await loadKernel('core-kernels/player-progression.ts')
const names = []
const wraps = []
const recordName = (name, rank) => {
  const expected = source.nativeSkillPageDisplayName(name, rank)
  const lines = source.nativeSkillPageWrappedLines(expected)
  names.push({ name, rank, expected, lines, height: source.nativeSkillPageTextHeight(lines) })
}
const recordWrap = (text) => {
  const lines = source.nativeSkillPageWrappedLines(text)
  wraps.push({ text, lines, height: source.nativeSkillPageTextHeight(lines) })
}
for (const skill of profile.NATIVE_SKILL_CATALOG) {
  for (const rank of [1, 3, 30]) recordName(skill.name, rank)
  recordWrap(skill.config?.mQDescription ?? skill.config?.mDescription ?? '')
}
for (const weld of profile.NATIVE_WELD_BUILDS) {
  for (const rank of [1, 3, 30]) recordName(weld.syntheticName, rank)
  recordWrap(weld.pairDescription)
}
for (const text of [
  '',
  'short',
  'a name with many words and spaces',
  'unbrokenabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz',
  'long-hyphenated-words-that-continue',
  'leading    and trailing     ',
  'line one\nline two\r\nthird line',
  'unsupported 🙂 symbols and accented é words',
]) recordWrap(text)
const tints = Array.from({ length: 8 }, (_, root) => ({
  root,
  expected: source.nativeSkillPageTint(root),
}))
writeFixture('skill-text-goldens.json', 'skill-text-goldens.mjs',
  'renderer/skill-book-render-contract.ts (pinned ed0a2d59)', { names, wraps, tints })
