// Shared real secondary authority/empty-world inputs for differential fixtures.
import { loadKernel } from './oracle.mjs'
const contract = await loadKernel('core-kernels/native-secondary-ability-contract.ts')
const profile = await loadKernel('core-kernels/player-progression.ts')
const equipment = await loadKernel('core-kernels/native-equipment-effects.ts')
const belt = await loadKernel('core-kernels/native-belt.ts')
const character = await loadKernel('core-kernels/player-character.ts')
const config = { element: 'ether', discipline: 'arcane', displayName: 'Secondary oracle' }

export function allLearnedBook(maximum = false) {
  const book = profile.createPlayerSkillBook(config)
  const ranks = [...book.effectiveRanks]
  for (const id of contract.NATIVE_SECONDARY_ABILITY_IDS) {
    ranks[id] = maximum ? profile.playerStatBook().entries[id].maximumLevel : 1
  }
  return { ...book, effectiveRanks: ranks, permanentRanks: ranks }
}
export function factors(equipped) {
  if (!equipped) return { damage: 1, manaCost: 1 }
  const modifiers = structuredClone(equipment.createNativeEquipmentModifiers())
  modifiers.globalDamageFlat = 2
  modifiers.globalDamageMultiplier = 1.25
  modifiers.globalManaCostFlat = 1.5
  modifiers.globalManaCostMultiplier = 0.8
  for (let id = 0; id < 8; id += 1) {
    modifiers.classDamageFlat[id] = id * 0.5
    modifiers.classDamageMultiplier[id] = 1 + id * 0.05
    modifiers.classManaCostFlat[id] = -id * 0.25
    modifiers.classManaCostMultiplier[id] = 1 - id * 0.025
    modifiers.classRecharge[id] = { offset: id * 0.125, scale: 1.5 }
  }
  for (let id = 0; id < 83; id += 1) {
    modifiers.skillDamageFlat[id] = id * 0.1
    modifiers.skillDamageMultiplier[id] = 1.1
  }
  return { damage: 1.5, manaCost: 0.75, globalFlatDamage: 2.5,
    globalManaReduction: 2, equipment: modifiers }
}
export function authority(skillBook, offensiveFactors) {
  return {
    alive: true,
    weaponKind: 'staff', belt: belt.createNativePlayerBelt(skillBook),
    character: character.createPlayerCharacter(config, { x: 0, y: 0 }),
    coldSlowFactor: 0.5, currentMana: 10000, eligible: true, enhancedEffects: false,
    explosiveShieldDamage: 0, explosiveShieldRawManaCost: 3,
    fireBurnDamage: 2, freezeDurationMultiplier: 1,
    focusInstantRechargeChancePercent: 0, golemIron: false, golemRawManaCost: 7,
    golemReflectFactor: 0, input: character.createIdlePlayerCharacterInput(),
    maximumMana: 10000, magicStormDurationBonusTicks: 0,
    magicStormFrequencyFactor: 1, magicStormRawManaCost: 11,
    maximumGolem: false, maximumLeviathan: false, maximumMagicStorm: false,
    maximumRingOfFire: false, maximumRingOfIce: false, manaRecoveryPerTick: 0.1,
    offensiveFactors, secondaryRechargeFactor: 1.25, skillBook, worldKey: 'boneyard:oracle',
  }
}
export function context(player, tick = 1) {
  return {
    dampenCandidates: () => ({ casterTargetIds: [], projectiles: [], shieldTargetIds: [] }),
    golemMovement: (_owner, _world, _origin, requested) => requested,
    golemPlacement: (_owner, _world, position, rng) => ({ position, rng }),
    phasingDestination: () => null,
    players: { player },
    teleportDestination: (_owner, rng) => ({ position: { x: 0, y: 0 }, rng }),
    targetExists: () => false, target: () => null, targets: () => [], tick,
  }
}
