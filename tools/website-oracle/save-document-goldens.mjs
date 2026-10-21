// Public Website save boundaries, including owner projection and disk-only resets.
import { loadKernel, writeFixture } from './oracle.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const saves = await loadKernel('save/game-save-document.ts')
const contract = await loadKernel('save/game-save-contract.ts')
const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const { STOCK_TUTORIAL_BONEYARD } = await loadKernel('host/stock-tutorial-boneyard.ts')
const playerId = 'sourceOwner_With-Dashes'
const config = { displayName: 'Save Oracle', element: 'fire', discipline: 'mind' }
const cases = []
for (const name of ['hub', 'hub-shared', 'tutorial', 'boneyard']) {
  const participants = { [playerId]: name === 'tutorial'
    ? { ...config, element: 'ether', discipline: 'arcane' } : config }
  if (name === 'hub-shared') participants.otherPlayer = { ...config, element: 'water' }
  let state = game.createGameSimulation(participants, { gameRngSeed: 17, combatRngSeed: 91 })
  let loadedBoneyard = null
  if (name === 'tutorial' || name === 'boneyard') {
    const template = name === 'tutorial' ? STOCK_TUTORIAL_BONEYARD : NATIVE_GENERATED_BONEYARDS[0]
    loadedBoneyard = { ...template, choice: {
      id: name === 'tutorial' ? 'stock-tutorial' : 'default-random',
      name: name === 'tutorial' ? 'Tutorial' : 'Random Boneyard', source: 'default',
    }, seed: '00000000000000000000000000000000', runId: `save-${name}` }
    state = game.enterBoneyardWorld(state, loadedBoneyard)
  }
  for (let tick = 0; tick < 40; tick += 1) {
    state = game.stepGameSimulationTick(state, Object.fromEntries(Object.keys(participants).map(id => [id, {
      movement: { x: 1, y: 0 }, aim: { x: 1100, y: 1000 },
      cast: { primary: name === 'boneyard', quickbar: null },
      viewportWidth: 1600, viewportHeight: 900,
    }])))
  }
  const options = { integrity: 'global-clean', loadedBoneyard, mods: [], modState: {}, playerId, state }
  const document = saves.createGameSaveDocument(options)
  const restored = saves.restoreGameSaveDocument(document)
  const profileDocument = saves.createGameProfileSaveDocument(options)
  cases.push({ name, document: JSON.parse(document), restored: restored.state,
    reencoded: JSON.parse(saves.createGameSaveDocument({ ...options, state: restored.state,
      loadedBoneyard: restored.loadedBoneyard })),
    profileDocument: JSON.parse(profileDocument),
    retired: JSON.parse(saves.retireGameSaveWizard(document)),
    summary: contract.parseGameSaveDocument(document).continuation.summary,
  })
}
const contracts = []
function contractCase(name, document) {
  const text = typeof document === 'string' ? document : JSON.stringify(document)
  try {
    const parsed = contract.parseGameSaveDocument(text)
    contracts.push({ name, text, accepted: true, sourceSchemaVersion: parsed.sourceSchemaVersion,
      summary: parsed.continuation?.summary ?? null, integrity: parsed.integrity,
      mods: parsed.mods, modState: parsed.modState, nativeSource: parsed.nativeSource })
  } catch (error) { contracts.push({ name, text, accepted: false, error: String(error.message) }) }
}
const baseline = cases[0].document
for (const schemaVersion of [1, 2, 4, 5, 6, 9, 10, 11, 12, 16, 17, 37, 38]) {
  const envelope = structuredClone(baseline)
  envelope.schemaVersion = schemaVersion
  if (schemaVersion < 17) delete envelope.nativeSource
  if (schemaVersion < 10) delete envelope.continuation.summary.partyRejoinToken
  if (schemaVersion < 6) delete envelope.continuation.summary.activeRun
  if (schemaVersion < 5) {
    const legacy = { ...envelope.continuation, schemaVersion }
    if (schemaVersion >= 2) Object.assign(legacy, { mods: [], modState: {} })
    if (schemaVersion === 4) legacy.integrity = 'global-clean'
    contractCase(`schema-${schemaVersion}`, legacy)
  } else contractCase(`schema-${schemaVersion}`, envelope)
}
for (const [name, change] of [
  ['missing-version', value => { delete value.schemaVersion }],
  ['unknown-root-field', value => { value.unknown = 1 }],
  ['empty-name', value => { value.continuation.summary.character.displayName = '' }],
  ['utf16-name-limit', value => { value.continuation.summary.character.displayName = '😀'.repeat(33) }],
  ['unknown-element', value => { value.continuation.summary.character.element = 'ice' }],
  ['negative-tick', value => { value.continuation.summary.savedAtTick = -1 }],
  ['invalid-token', value => { value.continuation.summary.partyRejoinToken = 'sdrpr2.bad.signature' }],
  ['valid-signed-token', value => { value.continuation.summary.partyRejoinToken = `sdrpr2.payload.${'a'.repeat(43)}` }],
  ['case-duplicate-mods', value => { value.mods = ['Test', 'test'].map(id => ({ id, version: '1', contentSha256: 'a'.repeat(64) })) }],
  ['ordered-mod-state', value => { value.modState = { Test: { z: 1e21, a: -0, m: { '2': 2, '1': 1, z: 3 } } } }],
  ['unsafe-state-key', value => { value.modState = { Test: { constructor: 1 } } }],
  ['too-many-state-keys', value => { value.modState = { Test: Object.fromEntries(Array.from({ length: 257 }, (_, i) => [`k${i}`, i])) } }],
]) {
  const value = structuredClone(baseline)
  change(value)
  contractCase(name, value)
}
contractCase('empty', '')
contractCase('invalid-json', '{')
writeFixture('save-document-goldens.json', 'tools/website-oracle/save-document-goldens.mjs',
  'Website/frontend/src/game/save/game-save-document.ts schema 37 public create/restore/retire', { cases, contracts })
