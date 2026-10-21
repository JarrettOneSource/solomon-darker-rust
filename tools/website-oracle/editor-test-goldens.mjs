import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const { materializeEditorTestBoneyard, MAX_EDITOR_TEST_BYTES } = await loadKernel('host/editor-test-boneyard.ts')
const { parseBoneyard, serializeBoneyard } = await loadKernel('../editor/format/boneyard.ts')
const flat = readFileSync(resolve(websiteDir, 'tests/fixtures/flat_multiplayer_test.boneyard'))
const binaries = { flat: flat.toString('base64') }
const native = parseBoneyard(flat)
native.geometry.playerSpawn = { x: 123.5, y: -456.25 }
native.geometry.playerSpawnFacingDeg = 90
binaries.spawn = Buffer.from(serializeBoneyard(native)).toString('base64')
for (const [key, width] of [['zeroBounds', 0], ['wideBounds', 131073]]) {
  const doc = parseBoneyard(flat)
  doc.meta.bounds.w = width
  doc.meta.bounds.right = doc.meta.bounds.left + width
  binaries[key] = Buffer.from(serializeBoneyard(doc)).toString('base64')
}
const valid = { name: 'Editor map', bytesBase64: { binary: 'flat' } }
function expand(value) {
  if (Array.isArray(value)) return value.map(expand)
  if (value && typeof value === 'object') {
    if ('binary' in value) return binaries[value.binary]
    if ('zeroBytes' in value) return Buffer.alloc(value.zeroBytes).toString('base64')
    return Object.fromEntries(Object.entries(value).map(([key, child]) => [key, expand(child)]))
  }
  return value
}
const cases = [
  ['null', null], ['array', []], ['empty-object', {}],
  ['empty-name', { ...valid, name: '' }], ['blank-name', { ...valid, name: ' ' }],
  ['long-name', { ...valid, name: 'x'.repeat(161) }],
  ['surrogate-name-limit', { ...valid, name: '😀'.repeat(81) }],
  ['non-string-bytes', { ...valid, bytesBase64: 42 }],
  ...['', 'a', '!!!!', 'AA=A', 'AB==', 'AAA=', 'AA==\n'].map((bytes, index) => [`bad-encoding-${index}`, { ...valid, bytesBase64: bytes }]),
  ['truncated-native', { ...valid, bytesBase64: flat.subarray(0, 32).toString('base64') }],
  ['near-byte-limit', { ...valid, bytesBase64: { zeroBytes: MAX_EDITOR_TEST_BYTES - 1 } }],
  ['at-byte-limit', { ...valid, bytesBase64: { zeroBytes: MAX_EDITOR_TEST_BYTES } }],
  ['over-byte-limit', { ...valid, bytesBase64: { zeroBytes: MAX_EDITOR_TEST_BYTES + 1 } }],
  ['over-encoded-limit', { ...valid, bytesBase64: { zeroBytes: MAX_EDITOR_TEST_BYTES + 4 } }],
  ['valid-flat', valid],
  ['authored-spawn', { ...valid, name: '  Authored map  ', bytesBase64: { binary: 'spawn' } }],
  ['javascript-trim', { ...valid, name: '\ufeff Named map \ufeff' }],
  ['preserve-non-js-whitespace', { ...valid, name: '\u0085Named map\u0085' }],
  ['maximum-name', { ...valid, name: '😀'.repeat(80) }],
  ['zero-bounds', { ...valid, bytesBase64: { binary: 'zeroBounds' } }],
  ['wide-bounds', { ...valid, bytesBase64: { binary: 'wideBounds' } }],
].map(([name, input]) => {
  try { return { name, input, accepted: true, entry: materializeEditorTestBoneyard(expand(input)) } }
  catch (error) { return { name, input, accepted: false, error: error.message } }
})
writeFixture('editor-test-goldens.json', 'editor-test-goldens.mjs',
  'current host/editor-test-boneyard.ts, actual native parser/authoring encoder and materializer', { binaries, cases })
