// Public secondary profiles and player clocks from unchanged Website kernels.
import { loadKernel, writeFixture } from './oracle.mjs'
import { allLearnedBook, factors, authority, context } from './secondary-inputs.mjs'
const secondary = await loadKernel('core-kernels/native-secondary-abilities.ts')
const contract = await loadKernel('core-kernels/native-secondary-ability-contract.ts')
const casting = await loadKernel('core-kernels/native-secondary-cast-action.ts')
const profile = await loadKernel('core-kernels/player-progression.ts')
const rng = await loadKernel('core-kernels/native-rng.ts')
const belt = await loadKernel('core-kernels/native-belt.ts')

const profiles = []
for (const skillId of contract.NATIVE_SECONDARY_ABILITY_IDS) {
  const entry = profile.playerStatBook().entries[skillId]
  for (const rank of new Set([1, entry.capLevel || 1, entry.maximumLevel])) {
    for (const equipped of [false, true]) {
      const source = allLearnedBook()
      const ranks = [...source.effectiveRanks]
      ranks[skillId] = rank
      const book = { ...source, effectiveRanks: ranks }
      const player = authority(book, factors(equipped))
      profiles.push({ skillId, book, factors: player.offensiveFactors,
        stats: profile.effectiveSecondaryAbilityRankStats(book, skillId),
        manaCost: secondary.nativeSecondaryAbilityManaCost(player, skillId),
        cooldown: secondary.nativeSecondaryCooldownCapacityTicks(book, skillId) })
    }
  }
}
const castActions = []
for (const weaponKind of ['staff', 'wand', null]) {
  for (const fasterCasterPercent of [0, 1.25, 50, 150]) {
    let action = { weaponKind, progress: 0 }
    const records = [action]
    while (action !== null) {
      action = casting.stepNativeSecondaryCastAction(action, fasterCasterPercent)
      records.push(action)
    }
    castActions.push({ weaponKind, fasterCasterPercent, records })
  }
}
const reserves = []
for (const maximum of [false, true]) {
  const book = allLearnedBook(maximum)
  for (const maximumMana of [100, 400.5]) {
    for (let mask = 0; mask < 8; mask += 1) {
      const player = { ...secondary.createNativeSecondaryPlayerState(),
        firewalker: Boolean(mask & 1), mindstar: Boolean(mask & 2), regenerate: Boolean(mask & 4) }
      const reserve = secondary.nativeSecondaryManaReserve(player, { skillBook: book, maximumMana })
      reserves.push({ book, maximumMana, player, reserve,
        ceiling: secondary.nativeSecondaryManaCeiling(maximumMana, { reservedMana: reserve }) })
    }
  }
}
const cooldownJourneys = []
for (const weaponKind of ['staff', 'wand', null]) {
  for (const equipped of [false, true]) {
    const book = allLearnedBook()
    const player = authority(book, factors(equipped))
    const ranks = [...book.effectiveRanks]
    ranks[70] = 3
    player.skillBook = { ...book, effectiveRanks: ranks }
    const initial = { ...secondary.createNativeSecondaryPlayerState(),
      castAction: { weaponKind, progress: 0 }, castSpinTicksRemaining: 3,
      globalCooldownTicks: 152.5, magicShieldPulseTicks: 3,
      planewalkerTicksRemaining: 3, stoneskinTicksRemaining: 3 }
    initial.cooldownTicksBySkill = initial.cooldownTicksBySkill.map((_, id) => (
      contract.NATIVE_SECONDARY_ABILITY_IDS.includes(id) ? 10001.25 : 0
    ))
    let state = { ...secondary.createNativeSecondarySimulation(77), players: { player: initial } }
    const records = []
    for (let tick = 1; tick <= 160; tick += 1) {
      state = secondary.stepNativeSecondaryAbilities(state, context(player, tick)).state
      records.push(state.players.player)
    }
    cooldownJourneys.push({ book: player.skillBook, factors: player.offensiveFactors,
      rechargeFactor: player.secondaryRechargeFactor, initial, records })
  }
}
const focusCases = []
for (const seed of [1, 2, 3, 4, 5]) {
  for (const chance of [0, 10, 50, 100]) {
    const book = allLearnedBook()
    const player = authority(book, factors(false))
    player.secondaryRechargeFactor = 0
    player.focusInstantRechargeChancePercent = chance
    player.belt = belt.bindNativeBeltSkill(player.belt, book, 46, 0)
    player.input = { ...player.input, cast: { primary: false, quickbar: 0 } }
    const initial = secondary.createNativeSecondaryPlayerState()
    const ticks = [...initial.cooldownTicksBySkill]
    ticks[11] = 100; ticks[21] = 150; ticks[35] = 150.5; ticks[50] = 200
    const source = { ...secondary.createNativeSecondarySimulation(seed),
      players: { player: { ...initial, cooldownTicksBySkill: ticks } } }
    const result = secondary.stepNativeSecondaryAbilities(source, context(player))
    focusCases.push({ seed, chance, initial: source.players.player,
      result: result.state.players.player, rng: result.state.rng })
  }
}
writeFixture('secondary-goldens.json', 'tools/website-oracle/secondary-goldens.mjs',
  'Website/frontend/src/game/core-kernels/native-secondary-abilities.ts and native-secondary-cast-action.ts', {
    profiles, castActions, reserves, cooldownJourneys, focusCases,
    planeOrbDamage: secondary.nativePlaneOrbDamage(allLearnedBook()),
  })
