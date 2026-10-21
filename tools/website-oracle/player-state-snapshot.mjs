import { loadKernel } from './oracle.mjs'

const runtime = await loadKernel('core-kernels/player-skill-runtime.ts')

export function playerStateSnapshot(state, index = 0) {
  const store = state.playerEntities
  const economy = store.economies[index]
  const book = store.skillBooks[index]
  const progression = store.progressions[index]
  const skills = store.skillRuntimes[index]
  const derived = runtime.playerSkillDerivedStats(skills, book, store.statBooks[index], progression, economy)
  return {
    book,
    gold: economy.gold,
    inventory: economy.backpack.map(item => [item.id, item.nativeTypeId, item.quantity]),
    equipped: [economy.equipment.hat, economy.equipment.robe, economy.equipment.weapon].map(item => item?.id ?? null),
    economyRevision: economy.revision,
    ownedPerks: economy.ownedPerkSelectors,
    economyRng: economy.rng,
    gameRng: state.gameRng,
    resources: [progression.currentHealth, progression.currentMana, progression.maximumHealth, progression.maximumMana],
    progression: { level: progression.level, experience: progression.experience, offerSeed: progression.offerSeed,
      offerCycle: progression.offerCycle, revision: progression.revision, pendingOffer: progression.pendingOffer },
    concentrations: [skills.concentrationSkillIdA, skills.concentrationSkillIdB],
    derived: [derived.castProgressFactor, derived.movementFactor, derived.maximumHealth, derived.maximumMana,
      derived.manaRecoveryPerSecond, derived.offensiveDamageFactor, derived.offensiveManaCostFactor],
  }
}
