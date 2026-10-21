import { execFileSync } from 'node:child_process'
// Cast-time payloads and offensive resolution, recorded from the unchanged Website kernels.
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
import { authoredStatBook } from './stat-book-overrides.mjs'

const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const progression = await loadKernel('core-kernels/player-progression.ts')
const primary = await loadKernel('core-kernels/native-primary-skill-profile.ts')
const offensive = await loadKernel('core-kernels/native-offensive-resolution.ts')
const equipment = await loadKernel('core-kernels/native-equipment-effects.ts')
const statBook = progression.playerStatBook()
const profiles = []
const neutral = { damage: 1, manaCost: 1 }
const boosted = { damage: 1.5, manaCost: 0.75 }

function book(element, ranks) {
  const source = progression.createPlayerSkillBook({
    discipline: 'arcane',
    displayName: 'Primary oracle',
    element,
  })
  const effectiveRanks = [...source.effectiveRanks]
  for (const [id, rank] of Object.entries(ranks)) effectiveRanks[Number(id)] = rank
  return { ...source, effectiveRanks }
}

function record(name, skillBook, factors, authored = statBook) {
  profiles.push({
    name,
    skillBook,
    factors,
    ...(authored === statBook ? {} : { statBook:authored }),
    profile: primary.nativePrimarySkillProfile(skillBook, authored, factors),
    summary: primary.nativePrimarySpellSummary(skillBook, authored, factors),
    descriptor: primary.nativePrimarySpellDescriptor(
      skillBook.primarySkillId,
      skillBook.weldBuildId,
    ),
  })
}

for (const element of ['ether', 'fire', 'air', 'water', 'earth']) {
  record(`${element}-fresh`, book(element, {}), neutral)
}
const upgraded = [
  ['ether', { 8: 2, 9: 2, 10: 3, 13: 2, 14: 3 }],
  ['fire', { 16: 3, 17: 2, 18: 2, 19: 2, 20: 2, 22: 2 }],
  ['fire', { 16: 4, 17: 2, 18: 3, 20: 2, 22: 3 }],
  ['air', { 24: 4, 25: 3, 26: 2, 29: 2, 31: 2 }],
  ['water', { 32: 2, 33: 2, 34: 3, 36: 2, 37: 2, 38: 2, 39: 1 }],
  ['earth', { 40: 3, 42: 2, 43: 3, 44: 2, 47: 2 }],
]
for (const [index, [element, ranks]] of upgraded.entries()) {
  record(`${element}-upgrades-${index}`, book(element, ranks), boosted)
}
record('water-short-tables-clamp', book('water', {
  32: 100, 33: 100, 34: 100, 36: 100, 37: 100, 38: 100, 39: 100,
}), neutral)

const modifiedEquipment = structuredClone(equipment.createNativeEquipmentModifiers())
modifiedEquipment.globalDamageFlat = 2
modifiedEquipment.globalDamageMultiplier = 1.25
modifiedEquipment.globalManaCostFlat = 1.5
modifiedEquipment.globalManaCostMultiplier = 0.8
modifiedEquipment.classDamageFlat[1] = 3
modifiedEquipment.classDamageMultiplier[1] = 1.2
modifiedEquipment.classManaCostFlat[1] = -0.5
modifiedEquipment.classManaCostMultiplier[1] = 0.75
modifiedEquipment.skillDamageFlat[16] = 4
modifiedEquipment.skillDamageMultiplier[16] = 1.1
const equippedFactors = {
  ...boosted,
  globalFlatDamage: 2.5,
  globalManaReduction: 2,
  equipment: modifiedEquipment,
}
record('fire-equipment-and-unforge', book('fire', { 16: 3, 17: 2 }), equippedFactors)

for (const build of progression.NATIVE_WELD_BUILDS) {
  for (const mode of ['bare', 'learned', 'frozen']) {
    const rank = mode === 'frozen' ? 3 : 1
    const ranks = { 52: 2 }
    for (const skillId of build.componentSkillIds) {
      ranks[skillId] = mode === 'bare' && !build.primarySkillIds.includes(skillId) ? 0 : rank
    }
    const skillBook = {
      ...book('ether', ranks),
      primarySkillId: 52,
      weldBuildId: build.id,
      weldComponentRanks: mode === 'frozen' ? [2, 3, 1, 2, 3, 1] : null,
    }
    const modifiers = structuredClone(equipment.createNativeEquipmentModifiers())
    modifiers.weldEffect = mode === 'frozen' ? 1.25 : 1
    record(`weld-${build.id}-${mode}`, skillBook, {
      ...(mode === 'frozen' ? boosted : neutral),
      equipment: modifiers,
    })
  }
}

const authored = authoredStatBook(statBook)
for (const [index,[element,ranks]] of upgraded.entries()) {
  record(`authored-${element}-${index}`,book(element,ranks),boosted,authored)
}
for (const build of progression.NATIVE_WELD_BUILDS) {
  let skillBook = progression.grantPlayerWeldBuild(book('ether',{}),build.id)
  skillBook = {...skillBook,weldComponentRanks:[2,3,1,2,3,1]}
  record(`authored-weld-${build.id}`,skillBook,boosted,authored)
}

const damageLanes = {
  actorBaseDamage: 2, baseDamage: 7, globalFlatDamage: 3,
  skillFlatDamage: 1, classFlatDamage: 2, globalMultiplier: 1.2,
  skillMultiplier: 1.1, classMultiplier: 1.3, elementOrClassMultiplier: 0.75,
}
const manaLanes = {
  baseManaCost: 12, globalManaReduction: 1, globalFlatManaCost: -2,
  classFlatManaCost: 3, skillFlatManaCost: 1, globalMultiplier: 0.9,
  classMultiplier: 1.2, skillMultiplier: 1.1, elementMultiplier: 0.8,
}
const resolution = [8, 16, 36, 52, 80].map(skillId => ({
  skillId,
  factors: equippedFactors,
  damageLanes,
  manaLanes,
  damage: offensive.resolveNativeSkillDamage(skillId, equippedFactors, damageLanes),
  mana: offensive.resolveNativeSkillManaCost(skillId, equippedFactors, manaLanes),
  freeMana: offensive.resolveNativeSkillManaCostValue(skillId, 0, equippedFactors),
}))

writeFixture(
  'primary-profile-goldens.json',
  'tools/website-oracle/primary-profile-goldens.mjs',
  'Website/frontend/src/game/core-kernels/native-primary-skill-profile.ts, native-weld-primary-profile.ts, native-offensive-resolution.ts',
  { sourceRevision, profiles, resolution },
)
