// Account save controls over the current public Source file/portable APIs.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const portable = await loadKernel('save/portable-game-profile.ts')
const portability = await loadKernel('save/game-save-portability.ts')
const saveFiles = await loadKernel('save/game-save-files.ts')
const saves = await loadKernel('save/game-save-document.ts')
const archive = await loadKernel('save/native-save-archive.ts')
const simulation = await loadKernel('core-server/game-simulation.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const economy = await loadKernel('core-kernels/hub-economy.ts')

const templateText = readFileSync(resolve(websiteDir, 'frontend/public/game/native/portable-profile-template.json'), 'utf8')
globalThis.fetch = async url => {
  assert.equal(url, '/game/native/portable-profile-template.json')
  return new Response(templateText, { status: 200 })
}
const template = JSON.parse(templateText)
const darkdata = new Uint8Array(Buffer.from(template.files.darkdata.base64, 'base64'))
const gamestate = new Uint8Array(Buffer.from(template.files.gamestate.base64, 'base64'))
const base = await portable.createPortableGameProfileFromNative(darkdata, gamestate, '_survival', [])

const imports = []
for (let root = 0; root < 8; root += 1) {
  const profile = structuredClone(base)
  profile.wizard.disciplineRoot = root
  const input = portable.encodePortableGameProfile(profile)
  const validated = await portable.parsePortableGameProfile(input)
  const imported = portability.createWebGameSaveFromPortableProfile(validated)
  imports.push({ root, input, expected: imported.character })
}

function newGame() {
  return simulation.createGameSimulation({ web: { displayName: 'Account Save Oracle', element: 'water', discipline: 'arcane' } })
}
function document(state) {
  return saves.createGameSaveDocument({ state, playerId: 'web', loadedBoneyard: null,
    integrity: 'global-clean', mods: [], modState: {}, nativeSource: null })
}
const exports = []
for (const mode of ['ordinary', 'second-hotbar', 'third-hotbar', 'equipment-only-visible-row']) {
  let state = newGame()
  if (mode === 'second-hotbar' || mode === 'third-hotbar') {
    state = simulation.bindGameSimulationPlayerSkillQuickbar(state, 'web', 32, mode === 'second-hotbar' ? 8 : 23)
  }
  if (mode === 'equipment-only-visible-row') {
    const current = simulation.getPlayerEconomy(state, 'web')
    const ring = economy.createEquipmentInventoryItem(economy.DOWSING_EQUIPMENT_RECIPES[0], 1000)
    state = { ...state, playerEntities: entities.replacePlayerEconomy(state.playerEntities, 'web', {
      ...current, equipment: { ...current.equipment, rings: [ring, null, null] },
    }) }
    assert.equal(state.playerEntities.skillBooks[0].permanentRanks[11], 0)
    assert.equal(state.playerEntities.skillBooks[0].effectiveRanks[11], 1)
    assert(state.playerEntities.skillBooks[0].learnedSkillOrder.includes(11))
  }
  const input = document(state)
  const exported = await portability.exportWebGameSaveToNativeArchive(input)
  exports.push({ mode, input, expected: {
    warnings: exported.warnings, learnedOrder: exported.portable.wizard.learnedOrder,
    skillQuickbar: exported.portable.wizard.skillQuickbar, disciplineRoot: exported.portable.wizard.disciplineRoot,
  } })
}

const selections = []
function file(name, bytes, relativePath = '') {
  const value = new File([bytes], name)
  Object.defineProperty(value, 'webkitRelativePath', { value: relativePath })
  return value
}
async function selection(name, files) {
  const input = await Promise.all(files.map(async value => ({
    name: value.name, relativePath: value.webkitRelativePath,
    bytes: Buffer.from(await value.arrayBuffer()).toString('base64'),
  })))
  try {
    const preview = await saveFiles.readGameSaveFileSelection(files)
    const { document: ignored, ...expected } = preview
    selections.push({ name, input, expected })
  } catch (error) {
    selections.push({ name, input, error: error.message })
  }
}
await selection('browser-json', [file('wizard.JSON', new TextEncoder().encode(exports[0].input))])
await selection('native-pair-directory', [file('DARKDATA.CFG', darkdata), file('GAMESTATE.SAV', gamestate, 'savegames/AccountRun/gamestate.sav')])
await selection('standalone-sav', [file('gamestate.sav', gamestate)])
await selection('native-archive', [file('wizard.zip', await archive.createNativeSaveArchive({ darkdata, gamestate, runName: 'AccountRun', retainedFiles: [] }))])
await selection('empty', [])
await selection('missing-pair', [file('darkdata.cfg', darkdata)])
await selection('invalid-json-utf8', [file('invalid.json', Uint8Array.of(0xff))])

writeFixture('current-save-tools-goldens.json', 'tools/website-oracle/current-save-tools-goldens.mjs',
  'website-rust-current save/game-save-{files,portability}.ts through Account.tsx public calls',
  { sourceRevision: 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83', imports, exports, selections })
