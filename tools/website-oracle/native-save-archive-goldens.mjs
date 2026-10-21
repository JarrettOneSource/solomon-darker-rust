// Original deterministic ZIP output, compressed input and strict manifest rejection.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
import { deflateRawSync } from 'node:zlib'
const archive = await loadKernel('save/native-save-archive.ts')
const template = JSON.parse(readFileSync(resolve(websiteDir, 'frontend/public/game/native/portable-profile-template.json'), 'utf8'))
const base64 = bytes => Buffer.from(bytes).toString('base64')
const from64 = text => new Uint8Array(Buffer.from(text, 'base64'))
const fileJSON = file => ({ path: file.path, bytes: base64(file.bytes) })
const archiveJSON = value => ({ darkdata: base64(value.darkdata), gamestate: base64(value.gamestate),
  retainedFiles: value.retainedFiles.map(fileJSON), runName: value.runName })
const encoder = new TextEncoder()
const zipCases = []
for (const [name, files] of [
  ['empty file', [{ path: 'empty', bytes: new Uint8Array() }]],
  ['unicode and opaque files', [{ path: 'wizard/é🧙.bin', bytes: Uint8Array.of(0, 255, 17, 128) },
    { path: 'luthacus/storage.bin', bytes: encoder.encode('persistent opaque data') }]],
]) {
  const zip = archive.createStoredZip(files)
  const read = await archive.readZip(zip)
  assert.deepEqual([...read].map(([path, bytes]) => ({ path, bytes })), files)
  zipCases.push({ name, files: files.map(fileJSON), zip: base64(zip) })
}
const nativeCases = []
for (const slot of [0, 7]) {
  const save = { darkdata: from64(template.files.darkdata.base64), gamestate: from64(template.files.gamestate.base64),
    runName: 'wizard_1', retainedFiles: [
      { path: 'solomondark/savegames/wizard_1/luthacus.sav', bytes: Uint8Array.of(3, 1, 4, 1, 5) },
      { path: archive.WEB_GAME_SAVE_SUPPORT_ARCHIVE_PATH, bytes: encoder.encode('{"schemaVersion":37,"opaque":true}') },
    ] }
  const zip = await archive.createNativeSaveArchive(save, slot)
  assert.deepEqual(await archive.readNativeSaveArchive(zip), save)
  nativeCases.push({ slot, save: archiveJSON(save), zip: base64(zip) })
}
function rawZip(path, contents, options = {}) {
  const compressed = options.compressed ?? new Uint8Array(deflateRawSync(contents))
  const name = encoder.encode(path)
  const descriptor = options.descriptor ?? false
  const flags = 0x800 | (descriptor ? 8 : 0)
  const size = options.declaredSize ?? contents.length
  const crc = archive.nativeArchiveCrc32(contents)
  const local = new Uint8Array(30 + name.length)
  const lv = new DataView(local.buffer)
  lv.setUint32(0, 0x04034b50, true); lv.setUint16(4, 20, true); lv.setUint16(6, flags, true)
  lv.setUint16(8, 8, true)
  if (!descriptor) { lv.setUint32(14, crc, true); lv.setUint32(18, compressed.length, true); lv.setUint32(22, size, true) }
  lv.setUint16(26, name.length, true); local.set(name, 30)
  const central = new Uint8Array(46 + name.length)
  const cv = new DataView(central.buffer)
  cv.setUint32(0, 0x02014b50, true); cv.setUint16(4, 20, true); cv.setUint16(6, 20, true)
  cv.setUint16(8, flags, true); cv.setUint16(10, 8, true); cv.setUint32(16, crc, true)
  cv.setUint32(20, compressed.length, true); cv.setUint32(24, size, true); cv.setUint16(28, name.length, true)
  central.set(name, 46)
  const end = new Uint8Array(22); const ev = new DataView(end.buffer)
  ev.setUint32(0, 0x06054b50, true); ev.setUint16(8, 1, true); ev.setUint16(10, 1, true)
  ev.setUint32(12, central.length, true); ev.setUint32(16, local.length + compressed.length, true)
  return new Uint8Array(Buffer.concat([local, compressed, central, end]))
}
const compressedCases = []
for (const [name, content, options] of [
  ['deflated data', encoder.encode('wizard '.repeat(1024)), {}],
  ['zero length deflated', new Uint8Array(), {}],
  ['data descriptor local zeros', Uint8Array.of(0, 1, 2, 255), { descriptor: true }],
  ['deflate trailing input', encoder.encode('retained'), { compressed: new Uint8Array([...deflateRawSync(encoder.encode('retained')), 42]) }],
]) {
  const zip = rawZip('native.bin', content, options)
  const read = await archive.readZip(zip)
  compressedCases.push({ name, zip: base64(zip), files: [...read].map(([path, bytes]) => fileJSON({ path, bytes })) })
}
const errors = []
async function reject(name, operation, input) {
  let message
  try {
    if (operation === 'zip') await archive.readZip(input)
    if (operation === 'native') await archive.readNativeSaveArchive(input)
    if (operation === 'create') archive.createStoredZip(input)
    if (operation === 'createNative') await archive.createNativeSaveArchive(input.save, input.slot)
  } catch (error) { message = error.message }
  assert.equal(typeof message, 'string', name)
  errors.push({ name, operation, ...(operation === 'create' ? { files: input.map(fileJSON) }
    : operation === 'createNative' ? { save: archiveJSON(input.save), slot: input.slot } : { input: base64(input) }), error: message })
}
for (const path of ['', '../escape', '/absolute', 'a//b', 'a/./b', 'a\\b', 'a:b', 'a\u0085b', 'a/'.repeat(260)]) {
  await reject(`unsafe path ${JSON.stringify(path)}`, 'create', [{ path, bytes: new Uint8Array() }])
}
await reject('duplicate case insensitive', 'create', [{ path: 'Wizard', bytes: new Uint8Array() }, { path: 'wizard', bytes: new Uint8Array() }])
await reject('zero files', 'create', [])
await reject('257 files', 'create', Array.from({ length: 257 }, (_, i) => ({ path: `file${i}`, bytes: new Uint8Array() })))
await reject('empty zip', 'zip', new Uint8Array())
await reject('missing end', 'zip', Uint8Array.of(1, 2, 3, 4))
for (const [name, edit] of [
  ['multi disk', (b, end, central) => new DataView(b.buffer).setUint16(end + 4, 1, true)],
  ['central count', (b, end, central) => new DataView(b.buffer).setUint16(end + 10, 2, true)],
  ['central offset overflow', (b, end, central) => new DataView(b.buffer).setUint32(end + 16, 0xffffffff, true)],
  ['unsupported flags', (b, end, central) => new DataView(b.buffer).setUint16(central + 8, 0x801, true)],
  ['unsupported compression', (b, end, central) => new DataView(b.buffer).setUint16(central + 10, 99, true)],
  ['expanded ceiling', (b, end, central) => new DataView(b.buffer).setUint32(central + 24, 67108865, true)],
  ['CRC drift', (b, end, central) => { const view = new DataView(b.buffer); view.setUint32(14, 1, true); view.setUint32(central + 16, 1, true) }],
  ['local size drift', (b, end, central) => new DataView(b.buffer).setUint32(22, 1, true)],
  ['local metadata drift', (b, end, central) => { b[30] = 120 }],
  ['local offset overflow', (b, end, central) => new DataView(b.buffer).setUint32(central + 42, 0xffffffff, true)],
]) {
  const b = from64(zipCases[0].zip); const end = b.length - 22; const central = new DataView(b.buffer).getUint32(end + 16, true)
  edit(b, end, central); await reject(name, 'zip', b)
}
await reject('deflate expansion beyond declared size', 'zip', rawZip('native.bin', encoder.encode('A'.repeat(100000)), { declaredSize: 1 }))
await reject('deflate invalid stream', 'zip', rawZip('native.bin', encoder.encode('A'), { compressed: Uint8Array.of(255) }))
await reject('deflate truncated stream', 'zip', rawZip('native.bin', encoder.encode('wizard'), { compressed: new Uint8Array(deflateRawSync(encoder.encode('wizard'))).slice(0, -1) }))
await reject('native has no manifest', 'native', from64(zipCases[0].zip))
const originalZip = from64(nativeCases[0].zip)
const originalFiles = [...await archive.readZip(originalZip)].map(([path, bytes]) => ({ path, bytes }))
for (const [name, edit] of [
  ['manifest SHA drift', manifest => { manifest.files[0].sha256 = '0'.repeat(64) }],
  ['manifest extra key', manifest => { manifest.extra = 1 }],
  ['manifest schema', manifest => { manifest.schemaVersion = 2 }],
  ['manifest slot', manifest => { manifest.slot = 8 }],
  ['manifest duplicate paths', manifest => { manifest.files[1].path = manifest.files[0].path }],
  ['manifest unsafe path', manifest => { manifest.files[0].path = 'solomondark/../darkdata.cfg' }],
  ['manifest file extra key', manifest => { manifest.files[0].extra = 1 }],
  ['manifest size noninteger', manifest => { manifest.files[0].size = 0.5 }],
  ['manifest uppercase digest', manifest => { manifest.files[0].sha256 = manifest.files[0].sha256.toUpperCase() }],
]) {
  const files = structuredClone(originalFiles)
  const manifest = JSON.parse(new TextDecoder().decode(files[0].bytes)); edit(manifest)
  files[0].bytes = encoder.encode(JSON.stringify(manifest))
  await reject(name, 'native', archive.createStoredZip(files))
}
for (const [name, files] of [
  ['manifest missing file', originalFiles.slice(0, -1)],
  ['manifest unlisted file', [...originalFiles, { path: 'unlisted', bytes: new Uint8Array() }]],
  ['manifest invalid JSON', [{ path: 'manifest.json', bytes: Uint8Array.of(255) }]],
]) await reject(name, 'native', archive.createStoredZip(files))
const save = { darkdata: from64(template.files.darkdata.base64), gamestate: from64(template.files.gamestate.base64), runName: 'wizard', retainedFiles: [] }
await reject('invalid output slot', 'createNative', { save, slot: 8 })
await reject('invalid output run name', 'createNative', { save: { ...save, runName: '../wizard' }, slot: 0 })
for (const path of ['solomondark/settings.txt', 'solomondark/darkdata.cfg', 'solomondark/savegames/other/gamestate.sav', 'foreign/opaque']) {
  await reject(`invalid retained ${path}`, 'createNative', { save: { ...save, retainedFiles: [{ path, bytes: new Uint8Array() }] }, slot: 0 })
}
writeFixture('native-save-archive-goldens.json', 'tools/website-oracle/native-save-archive-goldens.mjs',
  'Website/frontend/src/game/save/native-save-archive.ts', { zipCases, nativeCases, compressedCases, errors })
