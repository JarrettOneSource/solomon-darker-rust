import { loadKernel, writeFixture } from './oracle.mjs'

const profile = await loadKernel('core-kernels/player-progression.ts')
const panels = await loadKernel('skill-book-model.ts')
const inventory = await loadKernel('renderer/hub-inventory-render-contract.ts')
const cases = []
for (const element of ['air', 'earth', 'ether', 'fire', 'water']) {
  for (const discipline of ['arcane', 'body', 'mind']) {
    let book = profile.createPlayerSkillBook({ element, discipline, displayName: 'Oracle' })
    for (const extraSkills of [[], [9, 10, 13, 50, 51, 55, 56, 62, 65, 69, 72, 73, 74]]) {
      for (const id of extraSkills) book = profile.grantPlayerSkillRanks(book, id, 1)
      const progression = {
        learnedSkillOrder: book.learnedSkillOrder,
        learnedSkills: book.learnedSkillOrder.map(id => [id, book.permanentRanks[id], book.effectiveRanks[id]]),
        weldBuildId: book.weldBuildId,
      }
      const placements = panels.nativeSkillBookPagePlacements(panels.nativeSkillBookPages(progression))
      cases.push({ element, discipline, extraSkills, placements: placements.map(({page,x,y}) => ({
        root: page.rootSkillId, skills: page.rows.map(row => row.id), width: page.width, x, y,
      })) })
    }
  }
}
const formattedStats = []
for (const skill of profile.NATIVE_SKILL_CATALOG) {
  for (const rank of [1, 3, 10]) {
    for (const source of [...(skill.config?.mStats ?? []), ...(skill.config?.mBonus ?? [])]) {
      formattedStats.push({ id: skill.id, rank, source,
        expected: panels.formatNativeSkillBookTooltipLine(source, skill.config, rank) })
    }
  }
}
const rejectionNotices=[
  {action:'dowse',reason:'insufficient-gold',notice:inventory.HUB_DOWSING_INSUFFICIENT_GOLD},
  {action:'buy-hagatha',selector:0,reason:'perk-capacity-full',notice:inventory.hubHagathaFullMindNotice(0)},
  {action:'buy-hagatha',selector:27,reason:'perk-capacity-full',notice:inventory.hubHagathaFullMindNotice(27)},
  {action:'unequip',slot:'hat',reason:'required-clothing',notice:inventory.HUB_HAT_REMOVAL_MSGBOX},
  {action:'unequip',slot:'robe',reason:'required-clothing',notice:inventory.HUB_ROBE_REMOVAL_MSGBOX},
].map(({notice,...entry})=>({...entry,title:notice.title,body:notice.body}))
writeFixture('player-panels-goldens.json', 'tools/website-oracle/player-panels-goldens.mjs',
  'Website/frontend/src/game/skill-book-model.ts and renderer/hub-inventory-render-contract.ts', { cases, formattedStats, rejectionNotices })
