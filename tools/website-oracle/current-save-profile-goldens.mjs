// Current public profile envelopes and hydration, including genuine older documents.
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const saves = await loadKernel('save/game-save-document.ts')
const contract = await loadKernel('save/game-save-contract.ts')
const playerId = 'profile-current'
const config = { displayName: 'Current Profile', element: 'fire', discipline: 'mind' }
const fresh = () => game.createGameSimulation({ [playerId]: config }, {
  gameRngSeed: 17, combatRngSeed: 91,
})
const baseline = JSON.parse(saves.createGameProfileSaveDocument({
  state: fresh(), playerId, integrity: 'global-clean', mods: [], modState: {},
}))
const profiles = [], contracts = []
function record(name, document) {
  const text = JSON.stringify(document)
  try {
    const parsed = contract.parseGameSaveDocument(text)
    contracts.push({ name, document, accepted: true,
      sourceSchemaVersion: parsed.sourceSchemaVersion })
  } catch (error) {
    contracts.push({ name, document, accepted: false, error: error.message })
  }
  try {
    const profile = saves.restoreGameSaveProfile(text)
    const state = saves.hydrateGameSaveProfile(fresh(), playerId, profile)
    const reencoded = JSON.parse(saves.createGameProfileSaveDocument({
      state, playerId, integrity: profile.integrity, mods: profile.mods,
      modState: profile.modState, nativeSource: profile.nativeSource,
    }))
    profiles.push({ name, document, accepted: true, reencoded,
      gameRng: state.gameRng, advancedUnlocks: profile.advancedUnlocks })
  } catch (error) {
    profiles.push({ name, document, accepted: false, error: error.message })
  }
}
function change(name, mutate) {
  const document = structuredClone(baseline)
  mutate(document)
  record(name, document)
}
record('current', baseline)
change('compatible-future', value => { value.schemaVersion = 999 })
for (let index = 0; index < 8; index++) {
  change(`permanent-advanced-unlock-${72 + index}`, value => {
    value.profile.advancedUnlocks[index] = true
  })
}
change('all-advanced-unlocks', value => { value.profile.advancedUnlocks.fill(true) })
change('missing-advanced-unlocks', value => { delete value.profile.advancedUnlocks })
change('short-advanced-unlocks', value => { value.profile.advancedUnlocks.pop() })
change('long-advanced-unlocks', value => { value.profile.advancedUnlocks.push(false) })
change('numeric-unlock', value => { value.profile.advancedUnlocks[3] = 1 })
change('null-unlock', value => { value.profile.advancedUnlocks[4] = null })
change('unknown-profile-field', value => { value.profile.extra = true })
change('missing-dowsing-phase', value => { delete value.profile.economy.dowsingRolled })
change('invalid-dowsing-phase', value => { value.profile.economy.dowsingRolled = 1 })
change('dowsing-rolled-empty', value => { value.profile.economy.dowsingRolled = true })

const feedback = { accepted: true, action: 'read-skill-book', dowsingPitch: null,
  reason: null, sequence: 2, transferDirection: null, transferGesture: null,
  unforgeOutcome: null, skillBookOutcome: { kind: 'choice' } }
for (const [name, mutate] of [
  ['book-choice', value => {}],
  ['book-rank', value => { value.skillBookOutcome = { kind: 'rank', skillId: 11 } }],
  ['book-capped-rank', value => { value.skillBookOutcome = { kind: 'rank', skillId: null } }],
  ['book-missing-outcome', value => { delete value.skillBookOutcome }],
  ['book-null-outcome', value => { value.skillBookOutcome = null }],
  ['book-wrong-kind', value => { value.skillBookOutcome = { kind: 'grant' } }],
  ['book-low-rank', value => { value.skillBookOutcome = { kind: 'rank', skillId: 7 } }],
  ['book-high-rank', value => { value.skillBookOutcome = { kind: 'rank', skillId: 82 } }],
  ['book-choice-extra', value => { value.skillBookOutcome.skillId = 11 }],
  ['unsupported-action', value => { value.action = 'forge' }],
  ['accepted-reason-mismatch', value => { value.reason = 'invalid-target' }],
  ['zero-sequence', value => { value.sequence = 0 }],
  ['unrelated-paint-session', value => { value.dyeSessionId = 'paint-1' }],
  ['rename-retired', value => { value.action = 'rename-sack'; value.skillBookOutcome = null }],
  ['paint-open', value => { value.action = 'open-dye'; value.skillBookOutcome = null; value.dyeSessionId = 'paint-1' }],
  ['paint-result', value => { value.action = 'dye'; value.skillBookOutcome = null; value.dyeSessionId = 'paint-1' }],
  ['paint-close', value => { value.action = 'close-dye'; value.skillBookOutcome = null; value.dyeSessionId = 'paint-1' }],
  ['paint-empty-session', value => { value.action = 'dye'; value.skillBookOutcome = null; value.dyeSessionId = '' }],
  ['paint-historical-result', value => { value.action = 'dye'; value.skillBookOutcome = null }],
  ['transfer-missing-metadata', value => { value.action = 'transfer'; value.skillBookOutcome = null }],
  ['transfer-to-storage-double', value => { value.action = 'transfer'; value.skillBookOutcome = null; value.transferDirection = 'to-storage'; value.transferGesture = 'double-activation' }],
  ['dowse-missing-pitch', value => { value.action = 'dowse'; value.skillBookOutcome = null }],
  ['dowse-valid-pitch', value => { value.action = 'dowse'; value.skillBookOutcome = null; value.dowsingPitch = 1 }],
  ['dowse-excess-pitch', value => { value.action = 'dowse'; value.skillBookOutcome = null; value.dowsingPitch = 1.2 }],
  ['unforge-no-outcome', value => { value.action = 'unforge'; value.skillBookOutcome = null }],
  ['unforge-fizzle', value => { value.action = 'unforge'; value.skillBookOutcome = null; value.unforgeOutcome = { kind: 'fizzle', amount: null, itemName: 'Empty Wand' } }],
  ['unforge-invalid-mind-dredge', value => { value.action = 'unforge'; value.skillBookOutcome = null; value.unforgeOutcome = { kind: 'mind-dredge', amount: 2, itemName: 'Wand' } }],
]) {
  change(`feedback-${name}`, document => {
    const result = structuredClone(feedback); mutate(result)
    document.profile.economy.actionFeedback = result
  })
}
for (const action of ['read-skill-book', 'dye']) {
  change(`legacy-feedback-${action}`, document => {
    document.schemaVersion = 41; delete document.profile.advancedUnlocks
    delete document.profile.economy.dowsingRolled
    document.profile.economy.actionFeedback = { ...feedback, action }
    delete document.profile.economy.actionFeedback.skillBookOutcome
  })
}

const historical = JSON.parse(readFileSync(resolve(fixturesDir, 'save-profile-goldens.json'), 'utf8'))
for (const source of historical.cases) {
  record(`historical-${source.name}`, source.document)
}
writeFixture('current-save-profile-goldens.json',
  'tools/website-oracle/current-save-profile-goldens.mjs',
  'Current Website save/game-save-contract.ts and save/game-save-document.ts public profile boundary',
  { sourceRevision: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim(),
    historicalInput: { source: historical.source, generator: historical.generator },
    playerId, config, profiles, contracts })
