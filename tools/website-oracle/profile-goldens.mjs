import { loadKernel, writeFixture } from './oracle.mjs'
const profile = await loadKernel('core-kernels/player-progression.ts')
const cases = []
for (const element of ['air', 'earth', 'ether', 'fire', 'water']) {
  for (const discipline of ['arcane', 'body', 'mind']) {
    const config = { element, discipline, displayName: 'oracle' }
    const fresh = profile.createPlayerSkillBook(config)
    const granted = profile.grantPlayerSkillRanks(fresh, 72, 2)
    const capped = profile.grantPlayerSkillRanks(granted, fresh.primarySkillId, 100)
    cases.push({ element, discipline, fresh, granted, capped })
  }
}
const welds = profile.NATIVE_WELD_BUILDS.map(build => ({ build,
  learned: profile.grantPlayerWeldBuild(profile.createPlayerSkillBook({
    element: 'ether', discipline: 'arcane', displayName: 'oracle',
  }), build.id),
}))
writeFixture('profile-goldens.json', 'tools/website-oracle/profile-goldens.mjs',
  'Website/frontend/src/game/core-kernels/player-progression.ts and native-skill-catalog.json', {
    statBook: profile.playerStatBook().entries,
    catalog: profile.NATIVE_SKILL_CATALOG,
    cases, welds,
  })
