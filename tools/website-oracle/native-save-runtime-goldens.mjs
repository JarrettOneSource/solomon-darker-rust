// Replay retained native/portable and schema37 browser inputs through current
// public Source APIs. Preserve these migration inputs instead of regenerating
// them with the current simulation/schema before asking Source for the output.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel as load, websiteDir, writeFixture } from './oracle.mjs'

const portable = await load('save/portable-game-profile.ts')
const bridge = await load('save/game-save-portability.ts')
const files = await load('save/game-save-files.ts')
const archive = await load('save/native-save-archive.ts')
const fixtureName = 'native-save-runtime-goldens.json'
const retained = JSON.parse(readFileSync(resolve(fixturesDir, fixtureName), 'utf8'))
const templateText = readFileSync(resolve(websiteDir, 'frontend/public/game/native/portable-profile-template.json'), 'utf8')
globalThis.fetch = async url => {
  assert.equal(url, '/game/native/portable-profile-template.json')
  return new Response(templateText, {status: 200})
}
const b64 = value => Buffer.from(value).toString('base64')
const from64 = value => new Uint8Array(Buffer.from(value, 'base64'))

const imports = []
for (const {name, input} of retained.imports) {
  try {
    const validated = await portable.parsePortableGameProfile(input)
    const result = bridge.createWebGameSaveFromPortableProfile(validated)
    imports.push({name, input, output: {...result, document: JSON.parse(result.document)}})
  } catch (error) { imports.push({name, input, error: error.message}) }
}

const exports = []
for (const {name, input} of retained.exports) {
  try {
    const result = await bridge.exportWebGameSaveToNativeArchive(input)
    const saved = await archive.readNativeSaveArchive(result.archive)
    const support = saved.retainedFiles.find(file => file.path === archive.WEB_GAME_SAVE_SUPPORT_ARCHIVE_PATH)
    assert(support)
    exports.push({name, input, portable: portable.encodePortableGameProfile(result.portable),
      native: {darkdata: b64(saved.darkdata), gamestate: b64(saved.gamestate), runName: saved.runName,
        retainedFiles: saved.retainedFiles.filter(file => file !== support).map(file => ({path: file.path, bytes: b64(file.bytes)}))},
      support: JSON.parse(new TextDecoder().decode(support.bytes)), warnings: result.warnings})
  } catch (error) { exports.push({name, input, error: error.message}) }
}

const selections = []
for (const {name, input} of retained.selections) {
  const selected = input.map(({name, bytes, relativePath}) => {
    const file = new File([from64(bytes)], name)
    Object.defineProperty(file, 'webkitRelativePath', {value: relativePath})
    return file
  })
  try {
    const result = await files.readGameSaveFileSelection(selected)
    selections.push({name, input, output: {...result, document: JSON.parse(result.document)}})
  } catch (error) { selections.push({name, input, error: error.message}) }
}

writeFixture(fixtureName, 'tools/website-oracle/native-save-runtime-goldens.mjs',
  'Current Website save/game-save-portability.ts and game-save-files.ts over retained historical inputs',
  {sourceRevision: execFileSync('git', ['-C', websiteDir, 'rev-parse', 'HEAD'], {encoding: 'utf8'}).trim(),
    imports, exports, selections})
console.log(JSON.stringify({imports: imports.length, exports: exports.length, selections: selections.length,
  errors: exports.filter(row => row.error).map(row => [row.name, row.error])}))
