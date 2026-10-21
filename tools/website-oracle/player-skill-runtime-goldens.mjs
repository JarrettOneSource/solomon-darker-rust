import { loadKernel, writeFixture } from './oracle.mjs'
import { authoredStatBook } from './stat-book-overrides.mjs'
const skills = await loadKernel('core-kernels/player-skill-runtime.ts')
const profile = await loadKernel('core-kernels/player-progression.ts')
const economy = await loadKernel('core-kernels/hub-economy.ts')
const equipment = await loadKernel('core-kernels/native-equipment-effects.ts')
const primary = await loadKernel('core-kernels/native-primary-skill-profile.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const hagatha = await loadKernel('core-kernels/native-hagatha-effects.ts')
const statBook = profile.playerStatBook()
const learnedRows = [22, 28, 29, 36, 39, 53, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 75]
const defaultSetup = { element: 'ether', discipline: 'arcane', learned: [], recipes: [], owned: [],
  concentrationA: null, concentrationB: null, mindstar: false, mindChug: 0, quad: 0,
  weapon: true, effects: null, weldBuild: null, unforge: null }
function setup(input) {
  const spec = { ...defaultSetup, ...input }
  const authored = input.authored ? authoredStatBook(statBook) : statBook
  let book = profile.createPlayerSkillBook({ element: spec.element, discipline: spec.discipline, displayName: 'Stat oracle' })
  if (spec.weldBuild !== null) book = profile.grantPlayerWeldBuild(book, spec.weldBuild)
  for (const [id, count] of spec.learned) book = profile.grantPlayerSkillRanks(book, id, count)
  let money = economy.createHubEconomy(77, { starterElement: spec.element })
  let equipped = { ...money.equipment, rings: [...money.equipment.rings] }
  let ring = 0
  for (const [index, recipeIndex] of spec.recipes.entries()) {
    const item = economy.createEquipmentInventoryItem(economy.DOWSING_EQUIPMENT_RECIPES[recipeIndex], 1000 + index)
    const slot = item.equipmentType === 'staff' || item.equipmentType === 'wand' ? 'weapon' : item.equipmentType
    if (slot === 'ring') equipped.rings[ring++ % 3] = item
    else equipped[slot] = item
  }
  if (!spec.weapon) equipped.weapon = null
  if (spec.effects !== null) equipped.hat = { ...equipped.hat, nativeEffects: spec.effects }
  money = { ...money, equipment: equipped, ownedPerkSelectors: spec.owned,
    unforgeBonuses: spec.unforge === null ? money.unforgeBonuses : { ...money.unforgeBonuses, ...spec.unforge } }
  const created = skills.createPlayerSkillRuntime(book, authored, money)
  let runtime = { ...created.runtime, concentrationSkillIdA: spec.concentrationA,
    concentrationSkillIdB: spec.concentrationB, mindstarActive: spec.mindstar }
  const refreshed = skills.refreshPlayerSkillRuntime(runtime, created.skillBook, authored, money)
  runtime = refreshed.runtime
  book = refreshed.skillBook
  const progression = { ...profile.createPlayerProgression(417), damageX4TicksRemaining: spec.quad,
    mindChugTicksRemaining: spec.mindChug,
    hagathaRuntime: hagatha.applyNativeHagathaPurchaseRuntime(hagatha.createNativeHagathaRuntimeState(), spec.owned) }
  return { spec, book, money, runtime, progression, statBook:authored }
}
function derivedSnapshot(current) {
  const derived = skills.playerSkillDerivedStats(current.runtime, current.book, current.statBook, current.progression, current.money)
  const factors = { damage: derived.offensiveDamageFactor, equipment: current.runtime.equipmentModifiers,
    globalFlatDamage: derived.offensiveDamageFlat, globalManaReduction: derived.offensiveManaCostReduction, manaCost: derived.offensiveManaCostFactor }
  const vitals = skills.refreshPlayerCombatFromSkillStats({ currentHealth: 12.3, currentMana: 45.6, maximumHealth: 50, maximumMana: 100 }, derived)
  return { derived, vitals, primary: primary.nativePrimarySpellSummary(current.book, current.statBook, factors),
    poison: [0, 0.01, 1.234, 9.999].map(seconds => skills.playerPoisonDurationSeconds(derived, seconds)) }
}
const cases = []
function record(name, input) {
  const current = setup(input)
  cases.push({ name, setup: current.spec, book: current.book, runtime: current.runtime,
    ...(input.authored ? {statBook:current.statBook} : {}), ...derivedSnapshot(current) })
}
for (const element of ['air', 'earth', 'ether', 'fire', 'water']) {
  for (const discipline of ['arcane', 'body', 'mind']) record(`starter-${element}-${discipline}`, { element, discipline })
}
const learned = learnedRows.map(id => [id, 2])
for (const id of skills.CONCENTRATABLE_SKILL_IDS) record(`concentrate-${id}`, { learned, concentrationA: id })
for (const owned of [[], [0, 1], [2], [6], [10], [11], [16], [18], [20], [24, 25], [26], [21], [6, 16, 18]]) {
  record(`perks-${owned.join('-')}`, { learned, owned, concentrationA: 61, concentrationB: 70 })
}
record('bare-hands', { learned, owned: [20], weapon: false })
record('temporary-primary-and-secondary', { effects: [
  { kind: 4, magnitude: 2, operator: 0, target: 24 },
  { kind: 7, magnitude: 1, operator: 0, target: 35 },
] })
record('melee-flat-multiplier-quad-and-perks', { learned: [[65, 2]], quad: 10, owned: [16, 26], effects: [
  { kind: 3, magnitude: 1.25, operator: 0, target: 0 },
  { kind: 3, magnitude: 17.5, operator: 2, target: 0 },
] })
record('mind-chug-quad-and-unforge', { learned, mindChug: 3, quad: 100,
  unforge: { experience: 0.15, manaCostReduction: 3.5, maximumHealth: 2.25, maximumMana: 4.75, offensiveDamage: 2.5 } })
for (const recipe of economy.DOWSING_EQUIPMENT_RECIPES) record(`equipment-${recipe.sourceIndex}`, { learned, recipes: [recipe.sourceIndex] })
for (const [index, set] of equipment.nativeEquipmentTooltipSets().entries()) {
  record(`set-${index}`, { learned, recipes: set.memberRecipeIndices })
}
for (const build of profile.NATIVE_WELD_BUILDS) {
  for (const mode of ['frozen', 'spellwelder', 'energized']) {
    record(`weld-${build.id}-${mode}`, { learned, weldBuild: build.id, mindstar: true,
      owned: mode === 'spellwelder' ? [13] : [],
      effects: mode === 'energized' ? [{ kind: 37, magnitude: 1, operator: 0, target: 0 }] : null })
  }
}
record('maximum-weld-keeps-unlearned-primaries', { mindstar: true, effects: [{ kind: 37, magnitude: 1, operator: 0, target: 0 }] })
for (const concentrationA of [null,58,63]) {
  record(`authored-runtime-${concentrationA}`, { learned, concentrationA, mindstar:true, authored:true })
}

const selectionJourneys = []
for (const splitMind of [false, true]) {
  let current = setup({ learned, owned: splitMind ? [21] : [] })
  let rng = random.createNativeRng(77)
  const initial = { setup: current.spec, book: current.book, runtime: current.runtime, rng }
  const records = []
  const actions = [{ type: 'autofill' }, ...[57, 58, 60, 63, 71, 999].map(id => ({ type: 'concentration', id })),
    { type: 'slot', id: 57, slot: 0 }, { type: 'slot', id: 57, slot: 1 }, { type: 'slot', id: 65, slot: 1 },
    { type: 'mindstar', active: true }, { type: 'mindstar', active: false }, { type: 'toggle-staff' }]
  for (const action of actions) {
    let accepted = true
    try {
      let result
      if (action.type === 'autofill') {
        result = skills.autofillPlayerSkillSelections(current.runtime, current.book, statBook, current.money, rng)
        rng = result.rng
      } else if (action.type === 'concentration') result = skills.setPlayerConcentration(current.runtime, current.book, statBook, current.money, action.id)
      else if (action.type === 'slot') result = skills.setPlayerConcentrationSlot(current.runtime, current.book, statBook, current.money, action.id, action.slot)
      else if (action.type === 'mindstar') result = skills.setPlayerMindstarActive(current.runtime, action.active, current.book, statBook, current.money)
      else result = { runtime: skills.togglePlayerStaffMeleeLane(current.runtime), skillBook: current.book }
      current = { ...current, runtime: result.runtime, book: result.skillBook }
    } catch (error) { if (!(error instanceof RangeError)) throw error; accepted = false }
    records.push({ action, accepted, runtime: current.runtime, book: current.book, rng, ...derivedSnapshot(current) })
  }
  selectionJourneys.push({ initial, records })
}
const tickJourneys = []
for (const concentrationA of [null, 58]) {
  const current = setup({ element: 'water', learned, concentrationA })
  const initial = { setup: current.spec, runtime: current.runtime, book: current.book }
  let runtime = current.runtime
  const records = []
  const stages = [
    { ticks: 600, activity: { acting: false, moving: true, primaryChannel: null } },
    { ticks: 600, activity: { acting: false, moving: false, primaryChannel: null } },
    { ticks: 800, activity: { acting: true, moving: false, primaryChannel: 'air' } },
    { ticks: 40, activity: { acting: false, moving: false, primaryChannel: null } },
    { ticks: 300, activity: { acting: true, moving: false, primaryChannel: 'water' } },
    { ticks: 10, activity: { acting: true, moving: false, primaryChannel: 'fire' } },
    { ticks: 1, activity: { acting: true, moving: false, primaryChannel: 'water', primaryUnderpowered: true } },
  ]
  let tick = 0
  for (const stage of stages) {
    for (let index = 0; index < stage.ticks; index += 1) {
      tick += 1
      const derived = skills.playerSkillDerivedStats(runtime, current.book, statBook, current.progression, current.money)
      const result = skills.stepPlayerSkillRuntime(runtime, derived, stage.activity)
      runtime = result.runtime
      if (tick === 1 || tick % 25 === 0 || index === stage.ticks - 1) records.push({ tick, ...result })
    }
  }
  tickJourneys.push({ initial, stages, records })
}
// These helpers share gameplay RNG; replay both their outputs and every word.
const staffAttacks = []
const staffSetups = [
  { learned: [[65, 2], [71, 2]] },
  { learned: [[65, 2], [71, 2]], concentrationA: 71 },
  { learned: [[65, 2], [71, 2]], concentrationB: 71, owned: [16, 21, 26] },
  { learned: [[65, 2], [71, 2]], mindChug: 10, owned: [16, 26] },
]
for (const input of staffSetups) {
  const current = setup(input)
  const derived = skills.playerSkillDerivedStats(
    current.runtime, current.book, statBook, current.progression, current.money,
  )
  for (const chance of [derived.flailingChancePercent, 0, 100]) {
    const selected = { ...derived, flailingChancePercent: chance }
    for (let seed = 0; seed < 32; seed += 1) {
      const result = skills.resolvePlayerStaffAttack(selected, random.createNativeRng(seed))
      const damages = ['normal', 'knockback', 'disabling-hit', 'critical-hit', 'whirl']
        .map(outcome => ({ outcome, value: skills.playerStaffDamage(
          current.runtime, selected, current.progression, outcome,
        ) }))
      staffAttacks.push({ setup: current.spec, chance, seed, result, damages })
    }
  }
}
const insightCases = []
const insightSetups = [
  { name: 'slot-a', learned, concentrationA: 63, options: [11, 22, 52, 57] },
  { name: 'slot-b', learned, concentrationA: 57, concentrationB: 63, owned: [21], options: [22, 57] },
  { name: 'mind-chug', learned, concentrationA: 57, mindChug: 10, options: [22, 57] },
  { name: 'empty', learned, concentrationA: 63, options: [] },
  { name: 'capped', learned: learnedRows.map(id => [id, 10]), concentrationA: 63, options: [22, 57, 63] },
  { name: 'unlearned-secondary', learned: [[63, 2]], concentrationA: 63, options: [11, 21, 27, 35, 45] },
  { name: 'duplicate-choice', learned, concentrationA: 63, options: [22, 22, 57] },
  { name: 'authored-maxima', learned, concentrationA:63, options:[56,57,63],authored:true },
]
for (const { name, options, ...input } of insightSetups) {
  const current = setup(input)
  const offer = { level: 7, sequence: 3, options: options.map(skillId => ({
    skillId, targetRank: current.book.effectiveRanks[skillId] + 1,
  })) }
  for (let seed = 0; seed < (input.authored ? 8 : 32); seed += 1) {
    const result = skills.markPlayerCreativityInsight(
      current.runtime, offer, current.book, current.statBook, random.createNativeRng(seed),
    )
    insightCases.push({ name, setup: current.spec, offer, seed, result,
      ...(input.authored ? {statBook:current.statBook} : {}) })
  }
}
const equipmentJourneys = []
for (const [name, input, actions] of [
  ['provider-replacement-and-permanent-acquisition', {}, [
    { type: 'equipment', slot: 'hat', effects: [
      { kind: 4, magnitude: 2, operator: 0, target: 24 },
      { kind: 7, magnitude: 1, operator: 0, target: 35 },
    ] },
    { type: 'equipment', slot: 'robe', effects: [
      { kind: 4, magnitude: 1, operator: 0, target: 24 },
      { kind: 4, magnitude: 1, operator: 0, target: 16 },
    ] },
    { type: 'primary', skillId: 24 },
    { type: 'equipment', slot: 'hat', effects: [] },
    { type: 'grant', skillId: 16 },
    { type: 'equipment', slot: 'robe', effects: [] },
  ]],
  ['random-increase-can-learn-a-temporary-skill', { learned: [[8, 100], [11, 100]] }, [
    { type: 'equipment', slot: 'hat', effects: [{ kind: 4, magnitude: 2, operator: 0, target: 24 }] },
    { type: 'increase' },
    { type: 'equipment', slot: 'hat', effects: [] },
  ]],
]) {
  let current = setup(input)
  let rng = random.createNativeRng(9)
  const initial = { setup: current.spec, book: current.book, runtime: current.runtime, rng }
  const records = []
  for (const action of actions) {
    let skillId = null
    if (action.type === 'equipment') {
      current.money = { ...current.money, equipment: { ...current.money.equipment,
        [action.slot]: { ...current.money.equipment[action.slot], nativeEffects: action.effects } } }
    } else if (action.type === 'primary') {
      current.book = profile.selectPlayerPrimarySkill(current.book, action.skillId)
    } else if (action.type === 'grant') {
      current.book = profile.grantPlayerSkillRanks(current.book, action.skillId, 1)
    } else {
      const result = profile.increaseRandomLearnedSkill(current.book, rng)
      current.book = result.skillBook
      rng = result.rng
      skillId = result.skillId
    }
    const refreshed = skills.refreshPlayerSkillRuntime(current.runtime, current.book, statBook, current.money)
    current = { ...current, book: refreshed.skillBook, runtime: refreshed.runtime }
    records.push({ action, skillId, book: current.book, runtime: current.runtime, rng })
  }
  equipmentJourneys.push({ name, initial, records })
}
writeFixture('player-skill-runtime-goldens.json', 'tools/website-oracle/player-skill-runtime-goldens.mjs',
  'website-rust-current/frontend/src/game/core-kernels/player-skill-runtime.ts and native-harden.ts and native-hurricane.ts', {
    sourceRevision: 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83',
    cases, selectionJourneys, tickJourneys, staffAttacks, insightCases, equipmentJourneys,
  })
