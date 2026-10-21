// Shared canonical player construction and projection for host source fixtures.
import { loadKernel } from './oracle.mjs'
const game = await loadKernel('core-server/game-simulation.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const skills = await loadKernel('core-kernels/player-skill-runtime.ts')
const hagatha = await loadKernel('core-kernels/native-hagatha-effects.ts')
const nativeRng = await loadKernel('core-kernels/native-rng.ts')
const economy = await loadKernel('core-kernels/hub-economy.ts')
const clone = value => structuredClone(value)
const combatFields = ['hitFeedback','circleSlowTicksRemaining','coldSlowTicksRemaining','currentHealth','currentMana',
  'dazzleTicksRemaining','deathAgeTicks','deathEpoch','deathTick','lifeState','lastDamageTick','maximumHealth','maximumMana',
  'poisonDamagePerTick','poisonBeforeCold','poisonTicksRemaining']
const runtimeFields = ['concentrationSkillIdA','concentrationSkillIdB','harden','hardenArmorMaximum','hardenArmorPerTick',
  'hurricaneCharge','hurricaneEnabled','hurricaneRefreshed','meditationActivityRampTicks','meditationIdleElapsedTicks',
  'mindstarActive','nextConcentrationReplacementSlot','staffMeleeAlternate']
export function snapshot(store, index = 0) {
  const p = store.progressions[index], r = store.skillRuntimes[index], e = store.economies[index]
  return { combat: Object.fromEntries(combatFields.map(key => [key,p[key]])),
    progression: { revision:p.revision,damageX4TicksRemaining:p.damageX4TicksRemaining,
      mindChugTicksRemaining:p.mindChugTicksRemaining,poisonImmunityTicksRemaining:p.poisonImmunityTicksRemaining,hagathaRuntime:p.hagathaRuntime },
    runtime: Object.fromEntries(runtimeFields.map(key => [key,r[key]])),
    economy: { revision:e.revision,boast:e.npc.boast,items:economy.projectInventoryItems(e.backpack).map(({item}) => [item.id,item.nativeTypeId,item.nativeSubtype,item.quantity]) } }
}
export function setup(spec) {
  const state = game.createGameSimulation({ local:{displayName:'Oracle',element:'ether',discipline:'arcane'} },{gameRngSeed:17})
  const store = clone(state.playerEntities)
  const e = store.economies[0]
  e.ownedPerkSelectors = spec.perks ?? []
  if (spec.removePotions) e.backpack = e.backpack.filter(item => item.nativeTypeId !== 7001)
  e.npc.boast = { ...e.npc.boast, selected:9, succeeded:false }
  let book = store.skillBooks[0]
  for (const [id,rank] of spec.ranks ?? []) {
    book.permanentRanks[id] = rank
    if (!book.learnedSkillOrder.includes(id)) book.learnedSkillOrder.push(id)
  }
  if (spec.primaryId !== undefined) book.primarySkillId = spec.primaryId
  const refreshed = skills.refreshPlayerSkillRuntime(store.skillRuntimes[0],book,store.statBooks[0],e)
  store.skillBooks[0] = refreshed.skillBook
  store.skillRuntimes[0] = { ...refreshed.runtime,...spec.runtime }
  let p = store.progressions[0]
  p.hagathaRuntime = hagatha.applyNativeHagathaPurchaseRuntime(p.hagathaRuntime,e.ownedPerkSelectors)
  const derived = skills.playerSkillDerivedStats(store.skillRuntimes[0],store.skillBooks[0],store.statBooks[0],p,e)
  p = skills.refreshPlayerCombatFromSkillStats(p,derived)
  store.progressions[0] = { ...p,...spec.combat,...spec.potionClocks }
  store.primaryCasts[0] = { ...store.primaryCasts[0],...spec.primaryCast }
  store.locomotions[0] = { ...store.locomotions[0],position:{x:321.25,y:245.5},velocity:spec.velocity ?? {x:0,y:0} }
  return store
}
