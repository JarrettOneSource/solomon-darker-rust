import { loadKernel, writeFixture } from './oracle.mjs'

const model = await loadKernel('skill-book-model.ts')
const progression = await loadKernel('core-kernels/player-progression.ts')
const text = await loadKernel('renderer/skill-book-render-contract.ts')
const box = await loadKernel('renderer/native-skill-hover-box-layout.ts')
const picker = await loadKernel('renderer/skill-picker-render-contract.ts')

const rows = []
const layouts = []
const pickerRows = []
for (const skill of progression.NATIVE_SKILL_CATALOG) {
  if (progression.nativeSkillCategory(skill.id) === null) continue
  for (const [permanentRank, effectiveRank] of [[1, 1], [1, 3], [0, 2]]) {
    const row = {
      category: progression.nativeSkillCategory(skill.id),
      dependencyIds: progression.nativeSkillDependencies(skill.id),
      description: skill.config?.mQDescription ?? skill.config?.mDescription ?? '',
      effectiveRank, iconRecord: skill.id === 52 ? 0 : progression.nativeSkillIconRecord(skill.id),
      id: skill.id, name: skill.name, permanentRank, weldBuildId: null,
    }
    const expected = model.nativeSkillBookTooltipLines(row)
    rows.push({ skillId: skill.id, permanentRank, effectiveRank, expected })
    if (permanentRank === effectiveRank) {
      const source = { x: skill.id % 2 ? 50 : 1550, y: skill.id % 3 ? 200 : 750 }
      layouts.push({ lines: expected, source, expected: box.nativeSkillHoverBoxLayout(expected, source.x, source.y) })
    }
  }
}
for (const skill of progression.NATIVE_SKILL_CATALOG) {
  if (progression.nativeSkillCategory(skill.id) === null) continue
  const welds = skill.id === 52 ? progression.NATIVE_WELD_BUILDS : [null]
  for (const weld of welds) {
    const option = {
      insight: weld !== null || skill.id % 2 === 0,
      skillId: skill.id,
      targetRank: 3,
      ...(weld ? { weldBuildId: weld.id } : {}),
    }
    const expected = picker.skillPickerDetailPresentation(option).lines
    pickerRows.push({ option, expected })
  }
}
const runs = [
  '', 'Plain text', 'TITLE _s(.7)_o(0,1)2/30', '_iitalic_i normal',
  '_s(1.25)scaled _o(-2.5,.75)offset', '_s(1.)valid _s(+2)literal',
  '_s(1e2)literal _s( 2)literal _o(1, 2)literal', '_s(.)bad _i_i_twice',
  'first\nsecond _s(.7)third\nline', '🙂 accent é unsupported',
].map(source => ({
  source, expected: text.nativeSkillExactTextRuns(source),
  width: text.measureNativeSkillExactText(source),
}))
for (const source of [{ x: -20, y: -20 }, { x: 800, y: 450 }, { x: 1600, y: 899 }]) {
  for (const lines of [[], [{ kind: 'description', text: 'a very long line '.repeat(30) }], [{ kind: 'title', text: 'A _s(.7)_o(0,1)3/30' }]]) {
    layouts.push({ lines, source, expected: box.nativeSkillHoverBoxLayout(lines, source.x, source.y) })
  }
}
writeFixture('skill-hover-goldens.json', 'skill-hover-goldens.mjs',
  'Current Website fa08ae249ff5563570bdb2a653e2ef9358ce7b83 unchanged skill-book-model.ts, native-skill-hover-box-layout.ts and skill-picker-render-contract.ts',
  { rows, pickerRows, layouts, runs })
