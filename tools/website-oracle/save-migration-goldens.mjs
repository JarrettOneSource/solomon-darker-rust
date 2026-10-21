// Public restore is the oracle: the checkpoint edits below model persisted legacy fields,
// then the unmodified Website validates, migrates and rebuilds its actual simulation owners.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, writeFixture } from './oracle.mjs'

const saves = await loadKernel('save/game-save-document.ts')
const { createHubSkorchaAtVariant } = await loadKernel('core-server/hub-skorcha.ts')
const { createNativeRng } = await loadKernel('core-kernels/native-rng.ts')
const documents = JSON.parse(readFileSync(resolve(fixturesDir, 'save-document-goldens.json')))
const baseline = documents.cases.find(row => row.name === 'hub').document
const cases = []
function record(name, version, mutate) {
  let document = structuredClone(baseline)
  document.schemaVersion = version
  if (version < 17) delete document.nativeSource
  if (version < 10) delete document.continuation.summary.partyRejoinToken
  if (version < 6) delete document.continuation.summary.activeRun
  const raw = document.continuation.simulation
  if (version < 18) {
    raw.playerEntities.skillBooks[0].skillQuickbar = raw.playerEntities.belts[0]
      .map(entry => entry?.kind === 'skill' ? entry.skillId : null)
    delete raw.playerEntities.belts
  }
  if (version < 15) delete Object.values(raw.world.participants)[0].collegeIntro
  if (version < 8) delete raw.world.skorcha
  mutate(document.continuation.simulation, document)
  if (version < 5) {
    document = { ...document.continuation, schemaVersion: version,
      ...(version >= 2 ? { mods: [], modState: {} } : {}),
      ...(version === 4 ? { integrity: 'global-clean' } : {}) }
  }
  try {
    const restored = saves.restoreGameSaveDocument(JSON.stringify(document))
    const state = restored.state
    cases.push({ name, document, accepted: true, restored: {
      playerEntities: state.playerEntities, primarySpells: state.primarySpells,
      secondaryAbilities: state.secondaryAbilities, run: state.run,
      worldManagerOrder: state.worldManagerOrder, gameRng: state.gameRng,
      hubParticipants: state.world.participants,
    } })
  } catch (error) { cases.push({ name, document, accepted: false, error: error.message }) }
}

record('derived-runtime-is-rebuilt', 37, state => {
  const runtime = state.playerEntities.skillRuntimes[0]
  Object.assign(runtime, { equipmentModifiers: {}, hardenArmorMaximum: 999,
    hardenArmorPerTick: 999, hurricaneEnabled: true, meditationActivityRampTicks: 999,
    meditationIdleElapsedTicks: 999, mindstarActive: true, staffMeleeAlternate: true })
  state.playerEntities.skillBooks[0].effectiveRanks.fill(9)
  const player = Object.values(state.secondaryAbilities.players)[0]
  Object.assign(player, { castSpinTicksRemaining: 42, heldSlot: 0, lastSkillId: 21,
    mindstar: true, planeOrbHeld: true, regenerate: true, reservedMana: 999,
    castAction: { invalid: 'not retained' }, staffCastTicksRemaining: 20 })
})
record('legacy-book-and-concentration-runtime', 22, state => {
  const book = state.playerEntities.skillBooks[0]
  delete book.advancedUnlocks; delete book.learnedSkillOrder
  delete book.weldBuildId; delete book.weldComponentRanks
  book.permanentRanks[65] = 1
  book.effectiveRanks[65] = 1
  book.concentrationSkillIds = [65, null]
  book.nextConcentrationSlot = 1
  delete state.playerEntities.skillRuntimes
  delete state.combatRng; delete state.hallOfFameClockStartedAtTick
  delete state.run.gameOverExitKind; delete state.run.loadoutReadyPlayerIds
})
record('legacy-weld-cache', 27, state => {
  const book = state.playerEntities.skillBooks[0]
  book.primarySkillId = 52; book.permanentRanks[52] = 1
  book.permanentRanks[8] = 1; book.effectiveRanks[8] = 1
  book.activeWeldBuildId = 1000
  delete book.weldBuildId; delete book.weldComponentRanks
  delete state.playerEntities.primaryCasts[0].selectedPrimaryId
})
record('legacy-hit-timer-and-status-order', 30, state => {
  const progression = state.playerEntities.progressions[0]
  progression.lastDamageTick = state.tick - 3
  delete progression.hitFeedback; delete progression.poisonBeforeCold
  delete progression.circleSlowTicksRemaining
  delete state.playerEntities.lightings[0].blindnessTicksRemaining
})
record('persistent-harden-with-clean-action-clocks', 37, state => {
  state.playerEntities.skillBooks[0].permanentRanks[36] = 1
  Object.assign(state.playerEntities.skillRuntimes[0], {
    harden: { armor: 2, coating: .75 }, staffMeleeAlternate: true,
    hurricaneCharge: .6, hurricaneRefreshed: true,
  })
  Object.values(state.secondaryAbilities.players)[0].firewalker = true
})
record('pre-harden-schema-drops-coating', 29, state => {
  state.playerEntities.skillRuntimes[0].harden = { armor: 999, coating: 2 }
})
for (const version of [1, 4, 17, 18, 20, 21]) {
  record(`legacy-hub-painter-and-belt-${version}`, version, state => {
    if (version < 21) {
      state.lightProviderOrder = state.worldManagerOrder
      delete state.worldManagerOrder
      state.playerOfferRng = state.gameRng
      delete state.gameRng
      for (const student of state.world.studentPopulation.students) delete student.painterRegistration
    }
  })
}
record('legacy-college-walk-restarts-at-source-spawn', 14, state => {
  Object.values(state.world.participants)[0].transition = { phase: 'college-intro',
    sourceRegion: 'courtyard', destination: 'office', alpha: 1, scriptedSpeed: .45,
    scriptedTarget: { x: 881.5, y: -1000 } }
})
record('completed-tutorial-hub-attachment', 37, (state, document) => {
  state.playerEntities.economies[0].tutorialPending = false
  document.continuation.loadedBoneyard = structuredClone(documents.cases.find(row => row.name === 'tutorial').document.continuation.loadedBoneyard)
})
record('retained-college-office-walk', 37, state => {
  Object.assign(Object.values(state.world.participants)[0], { region: 'office', collegeIntro: {
    contactCounter: 2, coverAlpha: .5, dialogueSequence: 3, officeSpeed: .75,
    pathCursor: 1.25, phase: 'office-walk', titleCursor: 4,
  } })
  state.world.skorcha = createHubSkorchaAtVariant(createNativeRng(31), 1)
})
record('invalid-hub-transition-edge', 37, state => {
  Object.values(state.world.participants)[0].transition = { phase: 'outgoing',
    sourceRegion: 'courtyard', destination: 'courtyard', alpha: .5, scriptedSpeed: 1,
    scriptedTarget: { x: 0, y: 0 } }
})
record('college-intro-region-mismatch', 37, state => {
  Object.values(state.world.participants)[0].collegeIntro = {
    contactCounter: 0, coverAlpha: 1, dialogueSequence: 0, officeSpeed: 1,
    pathCursor: 0, phase: 'office-walk', titleCursor: 0,
  }
})
record('skorcha-placement-drift', 37, state => {
  state.world.skorcha = structuredClone(createHubSkorchaAtVariant(createNativeRng(31), 1))
  state.world.skorcha.position.x += 1
})
record('foreign-belt-item-is-rejected', 37, state => {
  state.playerEntities.belts[0][0] = { kind: 'item', itemId: 99999, nativeTypeId: 7001 }
})
record('unlearned-belt-skill-is-rejected', 37, state => {
  state.playerEntities.belts[0][0] = { kind: 'skill', skillId: 27 }
})
record('invalid-harden-is-rejected', 30, state => {
  state.playerEntities.skillRuntimes[0].harden = { armor: 1, coating: 0 }
})
record('future-hit-is-rejected', 37, state => {
  state.playerEntities.progressions[0].hitFeedback.tick = state.tick + 1
})

writeFixture('save-migration-goldens.json', 'tools/website-oracle/save-migration-goldens.mjs',
  'Website/frontend/src/game/save/game-save-document.ts public legacy/current restore', { cases })
