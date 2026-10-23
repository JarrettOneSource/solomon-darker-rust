import { loadKernel, writeFixture } from './oracle.mjs'

const books = await loadKernel('skill-book-feedback.ts')
const cases = Array.from({ length: 74 }, (_, index) => {
  const skillId = index + 8
  return {
    skillId,
    text: books.nativeSkillBookResultText(skillId),
    layout: books.nativeSkillBookResultLayout(skillId),
  }
})
writeFixture('skill-book-feedback-goldens.json',
  'tools/website-oracle/skill-book-feedback-goldens.mjs',
  'frontend/src/game/skill-book-feedback.ts::nativeSkillBookResultLayout', {
    sourceRevision: 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83', cases,
  })
