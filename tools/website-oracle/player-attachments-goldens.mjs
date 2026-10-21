// Actual articulated attachment frames, Enchant Staff and source orb bank boundaries.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { gameDir, loadKernel, writeFixture } from './oracle.mjs'

const enchant = await loadKernel('player-enchant-staff-presentation.ts')
const character = await loadKernel('player-character-presentation.ts')
const program = JSON.parse(readFileSync(resolve(gameDir, '../assets/game/player-staff-attachment-program.json'), 'utf8'))
const staff = []
for (let selector = 0; selector < 6; selector += 1) {
  for (const pose of [0, 5, 9]) {
    for (const headingIndex of [0, 7, 18, 23]) {
      const input = {
        headingIndex, learnedSkills: [[65, 1, 3]], living: true, nativeStaff: true,
        pose, selectedPrimarySkillId: [8, 16, 24, 32, 40, 80][selector], selector,
        tick: 193 + pose, weldBuildId: null,
      }
      staff.push({ input, frame: program.frames[pose][headingIndex], plan: enchant.nativeEnchantStaffDrawPlan(input) })
    }
  }
}
for (const override of [
  { widthSample: 0 }, { widthSample: 1.5 }, { tick: 193.25 }, { tick: 193.75 }, { living: false }, { nativeStaff: false },
  { learnedSkills: [] }, { selectedPrimarySkillId: -1 },
]) {
  const input = { ...staff[0].input, ...override }
  staff.push({ input, frame: program.frames[input.pose][input.headingIndex], plan: enchant.nativeEnchantStaffDrawPlan(input) })
}
const weldColors = Array.from({ length: 15 }, (_, index) => ({
  build: 1000 + index, tint: enchant.nativeEnchantStaffGlowTint(52, 1000 + index),
}))
const banks = []
for (const pose of [0, 4, 7, 9]) {
  for (const headingIndex of [4, 5, 6, 7, 16, 17, 18, 19]) {
    for (const phase of [0, Math.fround(0.1), Math.fround(0.1) + 0.000001]) {
      banks.push({
        pose, headingIndex, phase,
        front: character.playerCharacterStaffIsFront(headingIndex, pose),
        passes: character.playerCharacterStaffOrbPasses(headingIndex, pose, phase),
        scale: character.playerEquippedElementEffectScale(phase),
      })
    }
  }
}
writeFixture('player-attachments-goldens.json', 'player-attachments-goldens.mjs', [
  'player-enchant-staff-presentation.ts', 'player-character-presentation.ts',
], { staff, weldColors, banks })
