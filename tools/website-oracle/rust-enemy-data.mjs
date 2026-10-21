// Typed data emission shared by the retained wave and generated survival-program banks.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { root } from './oracle.mjs'

const source = readFileSync(resolve(root, 'crates/sim/src/enemies.rs'), 'utf8')
const names = new Map([...source.matchAll(/Self::(\w+)\s*=>\s*"([A-Z_]+)"/g)]
  .map((match) => [match[2], match[1]]))

function canonicalName(id) {
  const name = names.get(id)
  if (!name) throw new Error(`retained native data has no canonical Rust identity ${id}`)
  return name
}

export const rustEnemy = (id) => `EnemyToken::${canonicalName(id)}`
export const rustFlag = (id) => `EnemyFlag::${canonicalName(id)}`
export const rustString = (value) => JSON.stringify(value)
export const rustFloat = (value) => {
  if (!Number.isFinite(value)) throw new Error('retained native data contains a non-finite scalar')
  if (Object.is(value, -0)) return '-0.0'
  return Number.isInteger(value) ? `${value}.0` : String(value)
}

function family(recipe) {
  const value = recipe.family
  const number = rustFloat
  switch (value.kind) {
    case 'demon-skull': return `AuthoredEnemyFamily::DemonSkull { capabilities: ${value.capabilities} }`
    case 'heartmonger': return `AuthoredEnemyFamily::Heartmonger { crow_count: ${value.crowCount}, blind_mode: ${value.blindMode}, summon_mode: ${value.summonMode} }`
    case 'faculty': return `AuthoredEnemyFamily::Faculty(FacultyAppearance {
      body_color: [${value.bodyColor.map(number)}], head_color: [${value.headColor.map(number)}],
      female: ${value.female}, headgear: ${value.headgear}, primary: ${value.primary}, secondary: ${value.secondary} })`
    case 'skeleton': {
      const weapon = { claw: 'Claw', sword: 'Sword', mace: 'Mace', flail: 'Flail', axe: 'Axe', pike: 'Pike' }[value.weapon]
      if (!weapon) throw new Error(`unknown retained Skeleton weapon ${value.weapon}`)
      return `AuthoredEnemyFamily::Skeleton { armor: ${value.armor}, headgear: ${value.headgear}, weapon: SkeletonWeapon::${weapon} }`
    }
    case 'archer': {
      const arrow = { normal: 'Normal', fire: 'Fire', poison: 'Poison' }[value.arrowType]
      if (!arrow) throw new Error(`unknown retained Archer arrow ${value.arrowType}`)
      return `AuthoredEnemyFamily::Archer { arrow_type: ArrowType::${arrow}, extra_arrows: ${value.extraArrows}, headgear: ${value.headgear}, multi_arrow_mode: ${value.multiArrowMode}, range_mode: ${value.rangeMode}, strafing: ${value.strafing} }`
    }
    case 'portal': return `AuthoredEnemyFamily::Portal { frequency: ${value.frequency} }`
    case 'zombie': return `AuthoredEnemyFamily::Zombie { body_type: ${value.bodyType}, flyblown: ${value.flyblown}, poison_duration: ${number(value.poisonDuration)}, poison_pool_damage: ${number(value.poisonPoolDamage)}, poison_punch_damage: ${number(value.poisonPunchDamage)} }`
    default: throw new Error(`unknown retained authored family ${value.kind}`)
  }
}

export function rustRecipe(value) {
  const classification = { normal: 'Normal', boss: 'Boss', miniboss: 'Miniboss', 'multiple-boss': 'MultipleBoss' }[value.classification]
  if (!classification) throw new Error(`unknown retained classification ${value.classification}`)
  if (value.onDeathProgram !== null && value.onDeathProgram !== 'miniboss-die') {
    throw new Error(`unknown retained death program ${value.onDeathProgram}`)
  }
  const loot = value.lootPolicies
  return `AuthoredEnemyRecipe {
    uid: ${value.uid}, name: ${rustString(value.name)}.into(), classification: EnemyClassification::${classification},
    maximum_health: ${rustFloat(value.maximumHealth)}, primary_damage: ${rustFloat(value.primaryDamage)},
    secondary_damage: ${rustFloat(value.secondaryDamage)}, tertiary_damage: ${rustFloat(value.tertiaryDamage)},
    extra_damage: ${rustFloat(value.extraDamage)}, attack_speed: ${rustFloat(value.attackSpeed)},
    chase_speed: ${rustFloat(value.chaseSpeed)}, movement_scale: ${rustFloat(value.movementScale)},
    experience_bonus: ${rustFloat(value.experienceBonus)}, archer_accuracy_mode: ${value.archerAccuracyMode},
    family: ${family(value)},
    loot_policies: EnemyLootPolicies { gold: ${loot.gold}, item: ${loot.item}, orb: ${loot.orb}, potion: ${loot.potion}, powerup: ${loot.powerup}, specific_item: ${loot.specificItem} },
    miniboss_die_program: ${value.onDeathProgram === 'miniboss-die'},
  }`
}
