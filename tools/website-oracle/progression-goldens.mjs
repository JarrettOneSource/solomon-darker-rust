import { loadKernel, writeFixture } from './oracle.mjs'
const profile = await loadKernel('core-kernels/player-progression.ts')
const nativeRng = await loadKernel('core-kernels/native-rng.ts')
const hagatha = await loadKernel('core-kernels/native-hagatha-effects.ts')
const project = value => ({
  deferredSkillChoices:value.deferredSkillChoices,damageX4TicksRemaining:value.damageX4TicksRemaining,
  disciplineOfferBias:value.disciplineOfferBias,excludeActiveWeldBuildFromOffers:value.excludeActiveWeldBuildFromOffers,
  experience:value.experience,forcedOfferSkillIds:value.forcedOfferSkillIds,hagathaRuntime:value.hagathaRuntime,
  level:value.level,mindChugTicksRemaining:value.mindChugTicksRemaining,nextThreshold:value.nextThreshold,
  offerCycle:value.offerCycle,offerSeed:value.offerSeed,pendingLevels:value.pendingLevels,
  pendingOffer:value.pendingOffer,previousThreshold:value.previousThreshold,
  poisonImmunityTicksRemaining:value.poisonImmunityTicksRemaining,revision:value.revision,
  sorcerorsCharmAvailable:value.sorcerorsCharmAvailable,weldOfferMarker:value.weldOfferMarker,weldingOfferBias:value.weldingOfferBias,
  currentHealth:value.currentHealth,currentMana:value.currentMana,maximumHealth:value.maximumHealth,maximumMana:value.maximumMana,
  poisonBeforeCold:value.poisonBeforeCold,poisonDamagePerTick:value.poisonDamagePerTick,poisonTicksRemaining:value.poisonTicksRemaining,
})
const offers=[]
for (const element of ['air','earth','ether','fire','water']) {
  for (const level of [1,5,10,20,40,75]) {
    for (const bias of [false,true]) {
      let book=profile.createPlayerSkillBook({element,discipline:bias?'body':'arcane',displayName:'oracle'})
      book=profile.grantPlayerSkillRanks(book,book.primarySkillId,Math.min(15,level))
      if(level>=10) book=profile.grantPlayerSkillRanks(book,16,2)
      const state={...profile.createPlayerProgression(12345+level),level,maximumMana:40+level*5,
        disciplineOfferBias:bias,weldingOfferBias:bias,offerCycle:5,weldOfferMarker:0,
        forcedOfferSkillIds:level===40?[63]:[]}
      const rng=nativeRng.createNativeRng(77)
      offers.push({state:project(state),book,rng,result:profile.buildPlayerSkillOffer(state,book,7,rng)})
    }
  }
}
const journeys=[]
for(const element of ['air','earth','ether','fire','water']) {
  const initialRng=nativeRng.createNativeRng(12345)
  const creation=profile.drawNativePlayerCreationOfferSeed(initialRng)
  let rng=creation.rng
  let state=profile.createPlayerProgression(creation.seed)
  let book=profile.createPlayerSkillBook({element,discipline:'arcane',displayName:'oracle'})
  const initial={state:project(state),book,rng}
  const records=[]
  for(let index=0;index<24;index+=1) {
    const amount=index===0?90:index===1?1:(index+1)*500
    const awarded=profile.grantPlayerExperience(state,book,amount,rng,true)
    state=awarded.progression;rng=awarded.rng
    records.push({type:'experience',amount,state:project(state),book,rng})
    let remaining=10
    while(state.pendingOffer!==null && remaining-->0) {
      const offer=state.pendingOffer
      if(index===3 && state.sorcerorsCharmAvailable) {
        const rerolled=profile.rerollPlayerSkillOffer(state,book,offer.sequence,999,rng)
        state=rerolled.progression;rng=rerolled.rng
        records.push({type:'reroll',sequence:offer.sequence,seed:999,state:project(state),book,rng})
      }
      const chosen=state.pendingOffer.options[0]
      const selection={choiceIndex:0,offerSequence:state.pendingOffer.sequence,skillId:chosen.skillId}
      const applied=profile.applyPlayerSkillChoice(state,book,selection,rng,[6])
      state=applied.progression;book=applied.skillBook;rng=applied.rng
      records.push({type:'choice',selection,state:project(state),book,rng})
      const opened=profile.openNextPlayerSkillOffer(state,book,rng,true)
      state=opened.progression;rng=opened.rng
      records.push({type:'open',state:project(state),book,rng})
    }
  }
  journeys.push({element,initial,records})
}
const rewards=[]
for(const level of [1,2,5,6,15,16,30,31,75]) for(const count of [1,2,4]) {
  const input={evaluatedActorReward:12.5,arenaPlayerCount:count,receiverLevel:level,receiverXpBonus:0.1,gameplayScalar:1.25}
  rewards.push({input,experience:profile.boneyardEnemyExperienceAward(input)})
}
// These command journeys exercise timers, deferred choices and purchase-time skill effects
// against the source kernels rather than encoding their expected behavior in the test.
function auxiliaryJourney(name, initialState, initialBook, initialRng, commands) {
  let state = initialState
  let book = initialBook
  let rng = initialRng
  const initial = { state: project(state), book, rng }
  const records = []
  for (const command of commands) {
    const action = { ...command }
    let accepted = null
    let recoveredHealth = null
    let skillId = null
    if (['defer', 'choice', 'reroll'].includes(action.type) && action.sequence === undefined) {
      action.sequence = state.pendingOffer?.sequence ?? 0
    }
    switch (action.type) {
      case 'potion':
        state = profile.applyPlayerPotionEffect(state, action.subtype)
        break
      case 'step-potions':
        for (let tick = 0; tick < action.ticks; tick += 1) {
          state = profile.stepPlayerPotionEffects(state)
        }
        break
      case 'reset-potions':
        state = profile.resetPlayerPotionEffects(state)
        break
      case 'damage-bonus':
        state = profile.applyPlayerDamageX4Bonus(state)
        break
      case 'mind-dredge':
        state = profile.grantNativeUnforgeMindDredge(state)
        break
      case 'bonus-choice': {
        const result = profile.grantPlayerBonusSkillChoice(state, book, rng, action.charmOwned)
        state = result.progression
        rng = result.rng
        break
      }
      case 'automatic-choice': {
        const result = profile.setAutomaticPlayerSkillChoice(state, action.index)
        accepted = result !== null
        if (result) state = result
        break
      }
      case 'defer': {
        const result = profile.deferPlayerSkillChoice(state, book, action.sequence, rng, action.charmOwned)
        accepted = result !== null
        if (result) {
          state = result.progression
          rng = result.rng
        }
        break
      }
      case 'reroll': {
        const result = profile.rerollPlayerSkillOffer(state, book, action.sequence, action.seed, rng)
        accepted = result !== null
        if (result) {
          state = result.progression
          rng = result.rng
        }
        break
      }
      case 'choice': {
        if (action.skillId === undefined) action.skillId = state.pendingOffer.options[action.index].skillId
        const result = profile.applyPlayerSkillChoice(state, book, {
          choiceIndex: action.index, offerSequence: action.sequence, skillId: action.skillId,
        }, rng, [])
        accepted = result !== null
        if (result) {
          state = result.progression
          book = result.skillBook
          rng = result.rng
        }
        break
      }
      case 'open': {
        const result = profile.openNextPlayerSkillOffer(state, book, rng, action.charmOwned)
        state = result.progression
        rng = result.rng
        break
      }
      case 'random-skill': {
        const result = profile.increaseRandomLearnedSkill(book, rng)
        book = result.skillBook
        rng = result.rng
        skillId = result.skillId
        break
      }
      case 'weird-caster': {
        const result = profile.grantNativeWeirdCasterSkill(book, rng, action.owned)
        book = result.skillBook
        rng = result.rng
        skillId = result.skillId
        break
      }
      case 'revelation':
        book = profile.applyNativeRevelationToStartingSkills(book, { element: action.element })
        break
      case 'hagatha-purchase':
        state = { ...state, hagathaRuntime: hagatha.applyNativeHagathaPurchaseRuntime(state.hagathaRuntime, action.selectors) }
        break
      case 'hagatha-remove':
        state = { ...state, hagathaRuntime: hagatha.removeNativeHagathaRuntime(state.hagathaRuntime, action.selector) }
        break
      case 'hagatha-hurt':
        state = { ...state, hagathaRuntime: hagatha.clearNativeHagathaUntilHurt(state.hagathaRuntime, action.damage) }
        break
      case 'cheat-death': {
        const result = hagatha.consumeNativeHagathaCheatDeath(state.hagathaRuntime, state.maximumHealth)
        state = { ...state, hagathaRuntime: result.runtime }
        recoveredHealth = result.triggered ? result.currentHealth : null
        break
      }
      default:
        throw new Error(`unknown auxiliary command ${action.type}`)
    }
    records.push({ action, accepted, recoveredHealth, skillId, state: project(state), book, rng })
  }
  return { name, initial, records }
}
const auxiliaryJourneys = []
const freshBook = element => profile.createPlayerSkillBook({ element, discipline: 'arcane', displayName: 'oracle' })
const impairedState = {
  ...profile.createPlayerProgression(417),
  currentHealth: 12,
  currentMana: 4,
  maximumHealth: 123.456,
  maximumMana: 76.543,
  poisonBeforeCold: true,
  poisonDamagePerTick: Math.fround(0.3),
  poisonTicksRemaining: 77,
}
for (let subtype = 0; subtype < 6; subtype += 1) {
  auxiliaryJourneys.push(auxiliaryJourney(`potion-${subtype}`, impairedState, freshBook('air'), nativeRng.createNativeRng(77), [
    { type: 'potion', subtype },
    ...[1, 999, 4999, 1, 1].map(ticks => ({ type: 'step-potions', ticks })),
    { type: 'reset-potions' },
  ]))
}
auxiliaryJourneys.push(auxiliaryJourney('bonuses-and-reset', impairedState, freshBook('air'), nativeRng.createNativeRng(77), [
  { type: 'potion', subtype: 4 },
  { type: 'potion', subtype: 3 },
  { type: 'damage-bonus' },
  { type: 'step-potions', ticks: 1 },
  { type: 'mind-dredge' },
  { type: 'reset-potions' },
  { type: 'reset-potions' },
]))
auxiliaryJourneys.push(auxiliaryJourney('deferred-and-automatic-choices', profile.createPlayerProgression(417), freshBook('fire'), nativeRng.createNativeRng(77), [
  { type: 'automatic-choice', index: 0 },
  { type: 'bonus-choice', charmOwned: true },
  { type: 'automatic-choice', index: 999 },
  { type: 'automatic-choice', index: 1 },
  { type: 'automatic-choice', index: 1 },
  { type: 'defer', sequence: 999, charmOwned: true },
  { type: 'defer', charmOwned: false },
  { type: 'defer', charmOwned: true },
  { type: 'bonus-choice', charmOwned: true },
  { type: 'bonus-choice', charmOwned: true },
  { type: 'defer', charmOwned: true },
  { type: 'reroll', sequence: 999, seed: 321 },
  { type: 'choice', index: 999, skillId: 0 },
  { type: 'choice', index: 1 },
  { type: 'open', charmOwned: true },
]))
for (const element of ['air', 'earth', 'ether', 'fire', 'water']) {
  for (const owned of [[], [6]]) {
    auxiliaryJourneys.push(auxiliaryJourney(`purchase-skills-${element}-${owned.length}`, profile.createPlayerProgression(417), freshBook(element), nativeRng.createNativeRng(77), [
      { type: 'weird-caster', owned },
      { type: 'weird-caster', owned },
      { type: 'revelation', element },
      { type: 'revelation', element },
      { type: 'random-skill' },
      { type: 'random-skill' },
    ]))
  }
}
const cappedBook = freshBook('ether')
const cappedRanks = cappedBook.permanentRanks.map((rank, id) => id >= 8 && rank > 0
  ? profile.playerStatBook().entries[id].maximumLevel
  : rank)
auxiliaryJourneys.push(auxiliaryJourney('random-skill-at-cap', profile.createPlayerProgression(417), {
  ...cappedBook, permanentRanks: cappedRanks, effectiveRanks: cappedRanks,
}, nativeRng.createNativeRng(77), [{ type: 'random-skill' }]))
auxiliaryJourneys.push(auxiliaryJourney('hagatha-runtime', impairedState, freshBook('air'), nativeRng.createNativeRng(77), [
  { type: 'hagatha-purchase', selectors: [7, 24, 25] },
  { type: 'hagatha-hurt', damage: 0 },
  { type: 'cheat-death' },
  { type: 'cheat-death' },
  { type: 'hagatha-purchase', selectors: [7] },
  { type: 'hagatha-remove', selector: 24 },
  { type: 'hagatha-remove', selector: 25 },
  { type: 'hagatha-purchase', selectors: [24, 25] },
  { type: 'hagatha-hurt', damage: 0.5 },
  { type: 'hagatha-remove', selector: 7 },
  { type: 'hagatha-remove', selector: 28 },
]))
const hagathaEffects = {
  factors: hagatha.NATIVE_HAGATHA_FACTORS,
  modifiers: [],
  bosses: [],
  drinkerHealth: [],
  drinkerMana: [],
  revelation: [],
}
const selectorGroups = [
  [],
  ...[2, 10, 11, 16, 18, 20, 22, 24, 25, 26].map(selector => [selector]),
  [2, 10, 11, 16, 18, 20, 22, 24, 25, 26],
  [10, 20],
  [16, 20, 26],
]
for (const owned of selectorGroups) {
  for (const weaponEquipped of [false, true]) {
    for (const reverieActive of [false, true]) {
      for (const serendipityActive of [false, true]) {
        const runtime = { cheatDeathCharges: 1, reverieActive, serendipityActive }
        hagathaEffects.modifiers.push({
          owned, runtime, weaponEquipped,
          result: hagatha.nativeHagathaDerivedModifiers(owned, runtime, weaponEquipped),
        })
      }
    }
  }
}
for (const owned of [[], [22]]) {
  for (const nativeTypeId of [0, 1007, 1008, 1009, 1010, 1011, 1012]) {
    hagathaEffects.bosses.push({ owned, nativeTypeId,
      result: hagatha.nativeHagathaBossDamageFactor(owned, nativeTypeId) })
  }
}
for (const owned of [[], [15]]) {
  for (const currentHealth of [-10.01, -10, -9.99, 0, 'NaN', 'Infinity', '-Infinity']) {
    hagathaEffects.drinkerHealth.push({ owned, currentHealth,
      result: hagatha.nativeHagathaDrinkerShouldUseHealthPotion(owned, Number(currentHealth)) })
  }
  const manaBoundaries = [
    ...[49, 50, 50.01, 99.999, 100, 100.01].map(cost => [50, 100, cost]),
    ['NaN', 100, 60], [50, 'Infinity', 60], [50, 100, 'NaN'],
  ]
  for (const [currentMana, maximumMana, cost] of manaBoundaries) {
    hagathaEffects.drinkerMana.push({ owned, currentMana, maximumMana, cost,
      result: hagatha.nativeHagathaDrinkerShouldUseManaPotion(owned,
        Number(currentMana), Number(maximumMana), Number(cost)) })
  }
}
for (const owned of [[], [6]]) {
  for (const rank of [0, 1, 2, 3, 20]) {
    hagathaEffects.revelation.push({ owned, rank,
      result: hagatha.nativeHagathaRevelationRank(rank, owned) })
  }
}
writeFixture('progression-goldens.json', 'tools/website-oracle/progression-goldens.mjs',
  'Website/frontend/src/game/core-kernels/player-progression.ts and native-hagatha-effects.ts', {
    offers, journeys, rewards, auxiliaryJourneys, hagathaEffects,
  })
