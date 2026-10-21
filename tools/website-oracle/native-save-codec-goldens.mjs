import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const codec = await loadKernel('save/native-save-codec.ts')
const template = JSON.parse(readFileSync(resolve(websiteDir, 'frontend/public/game/native/portable-profile-template.json'), 'utf8'))
const base64 = bytes => Buffer.from(bytes).toString('base64')
const bytes = value => new Uint8Array(Buffer.from(value, 'base64'))
const node = (payload = [], children = []) => ({ payload: Uint8Array.from(payload), children, offset: 0, payloadOffset: 0 })
const buffer = (root, namedBuffers = []) => ({ root, namedBuffers, offset: 0, endOffset: 0 })
const named = (name, root) => ({ name, buffer: buffer(root), nameOffset: 0 })
function tree(value) {
  const chunk = value => ({ children: value.children.map(chunk), offset: value.offset,
    payload: base64(value.payload), payloadOffset: value.payloadOffset })
  return { endOffset: value.endOffset, namedBuffers: value.namedBuffers.map(item => ({
    buffer: tree(item.buffer), name: item.name, nameOffset: item.nameOffset,
  })), offset: value.offset, root: chunk(value.root) }
}
const syncCases = []
for (const [name, input] of [
  ['empty', buffer(node())],
  ['nested and named UTF8', buffer(node([0, 255, 1], [node([3, 2, 1]), node([], [node([19])])]), [
    named('Wizárd 🧙', node([3, 7, 11])), named('embedded\0NUL', node()),
  ])],
  ['compressor repeated matches', buffer(node(Array.from({ length: 4096 }, (_, i) => i % 73)))],
  ['compressor all marker frequencies', buffer(node(Array.from({ length: 768 }, (_, i) => i % 256)))],
]) {
  const raw = codec.encodeNativeSyncBuffer(input)
  const parsed = codec.parseNativeSyncBuffer(raw)
  const encoded = codec.encodeNativeDarkdata(parsed)
  assert.deepEqual(codec.decodeNativeDarkdata(encoded), parsed)
  syncCases.push({ name, raw: base64(raw), darkdata: base64(encoded), parsed: tree(parsed) })
}
for (const [name, raw, darkdata] of [
  ['retail darkdata', codec.encodeNativeSyncBuffer(codec.decodeNativeDarkdata(bytes(template.files.darkdata.base64))), bytes(template.files.darkdata.base64)],
  ['retail gamestate', bytes(template.files.gamestate.base64), null],
]) {
  const parsed = codec.parseNativeSyncBuffer(raw)
  const encoded = codec.encodeNativeDarkdata(parsed)
  if (darkdata) assert.deepEqual(encoded, darkdata)
  assert.deepEqual(codec.encodeNativeSyncBuffer(parsed), raw)
  syncCases.push({ name, raw: base64(raw), darkdata: base64(encoded), parsed: tree(parsed) })
}
const errors = []
function reject(name, kind, raw) {
  let message
  try { (kind === 'sync' ? codec.parseNativeSyncBuffer : codec.decodeNativeDarkdata)(raw) }
  catch (error) { message = error.message }
  assert.equal(typeof message, 'string', `${name} must fail`)
  errors.push({ name, kind, raw: base64(raw), error: message })
}
for (const size of [0, 1, 3, 4, 7, 8, 9, 11]) reject(`truncated empty ${size}`, 'sync', new Uint8Array(size))
const rawGame = bytes(template.files.gamestate.base64)
reject('trailing byte', 'sync', new Uint8Array([...rawGame, 1]))
reject('impossible child count', 'sync', Uint8Array.of(0, 0, 0, 0, 255, 255, 255, 255))
reject('impossible named count', 'sync', Uint8Array.of(0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255))
let root = node()
for (let depth = 0; depth < 129; depth += 1) root = node([], [root])
reject('depth exceeds128', 'sync', codec.encodeNativeSyncBuffer(buffer(root)))
const oneNamed = codec.encodeNativeSyncBuffer(buffer(node(), [named('a', node())]))
const badTerminator = oneNamed.slice(); badTerminator[17] = 1
reject('name not terminated', 'sync', badTerminator)
const badUTF8 = oneNamed.slice(); badUTF8[16] = 255
reject('name not UTF8', 'sync', badUTF8)
for (const [name, input] of [
  ['empty', []], ['unterminated marker', [0, 0]], ['truncated length', [0, 0, 128]],
  ['truncated distance', [0, 0, 1]], ['zero distance', [0, 0, 1, 0]],
  ['distance beyond prefix', [0, 0, 1, 1]],
  ['overflow length', [0, 0, 255, 255, 255, 255, 127]],
  ['overlong length', [0, 0, 129, 128, 128, 128, 128]],
  ['decompression ceiling', [0, 1, 0, 144, 128, 128, 0, 1]],
]) reject(name, 'darkdata', Uint8Array.from(input))
writeFixture('native-save-codec-goldens.json', 'tools/website-oracle/native-save-codec-goldens.mjs',
  'Website/frontend/src/game/save/native-save-codec.ts', {
    retailTemplate: { darkdata: template.files.darkdata, gamestate: template.files.gamestate }, syncCases, errors,
  })
