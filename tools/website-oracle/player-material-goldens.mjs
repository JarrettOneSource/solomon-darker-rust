import { loadKernel, writeFixture } from './oracle.mjs'
const material = await loadKernel('renderer/player-material.ts')
const texture = await loadKernel('renderer/native-texture-color.ts')
const colors = await loadKernel('core-kernels/native-skill-colors.ts')
const cases = []
for (const worldTint of [0xffffff, 0x804020, 0x916491]) {
  for (const poison of [false, true]) for (const cold of [false, true]) {
    for (const poisonBeforeCold of [false, true]) for (const stoneskin of [false, true]) {
      const tint = material.nativePlayerMaterialTint(worldTint, { stoneskinTicksRemaining: stoneskin ? 1 : 0 }, {
        coldSlowTicksRemaining: cold ? 250 : 0, poisonTicksRemaining: poison ? 1_000 : 0, poisonBeforeCold,
      })
      cases.push({ worldTint, poison, cold, poisonBeforeCold, stoneskin, tint,
        robeTint: texture.multiplyNativeTints(0x916491, tint),
        hitTint: texture.nativePuppetHitTint(true, tint), flatHitTint: texture.nativePuppetHitTint(false, tint) })
    }
  }
}
const primaryColors = [0, 1, 2, 3, 4, 5, 6, 7, 8, 16, 24, 32, 40, 52, 999].map(skillId => ({
  skillId, weldBuildId: null, tint: colors.nativePrimarySpellTint(skillId, null),
}))
for (let weldBuildId = 999; weldBuildId <= 1015; weldBuildId++) {
  primaryColors.push({ skillId: 52, weldBuildId, tint: colors.nativePrimarySpellTint(52, weldBuildId) })
}
writeFixture('player-material-goldens.json', 'tools/website-oracle/player-material-goldens.mjs',
  'Website/frontend/src/game/renderer/player-material.ts, native-texture-color.ts, core-kernels/native-skill-colors.ts', { cases, primaryColors })
