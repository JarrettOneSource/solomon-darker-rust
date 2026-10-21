#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const progression = await loadKernel('core-kernels/player-progression.ts')
const contract = await loadKernel('core-kernels/native-secondary-ability-contract.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const cases = []
for (const skillId of contract.NATIVE_SECONDARY_ABILITY_IDS) {
  const maximum = Math.max(progression.playerStatBook().entries[skillId].maximumLevel, 2)
  for (let rank = 1; rank <= maximum; rank += 1) {
    const book = progression.createPlayerSkillBook({ displayName: 'Rank oracle', discipline: 'arcane', element: 'ether' })
    const effectiveRanks = [...book.effectiveRanks]
    effectiveRanks[skillId] = rank
    const effective = progression.effectiveSecondaryAbilityRankStats({ ...book, effectiveRanks }, skillId)
    const direct = progression.nativeSecondaryAbilityRankStats(skillId, rank)
    if (JSON.stringify(effective) !== JSON.stringify(direct)) throw new Error('Source rank delegates diverged')
    cases.push(direct)
  }
}
writeFixture('current-secondary-rank-goldens.json', 'current-secondary-rank-goldens.mjs',
  'core-kernels/player-progression.ts nativeSecondaryAbilityRankStats and effective caller', { sourceRevision, cases })
