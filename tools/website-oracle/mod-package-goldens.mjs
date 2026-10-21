// The actual Website provisioner validates these packages before Lua execution.
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const { materializeWebSessionContent } = await loadKernel('host/web-mod-content.ts')
const boneyardBytes = readFileSync(resolve(websiteDir, 'tests/fixtures/flat_multiplayer_test.boneyard'))
const boneyardBase64 = boneyardBytes.toString('base64')
const baseMod = {
  boneyards: [], contentSha256: 'A'.repeat(64), entryScript: null, files: [],
  id: 'tests.first', name: 'First', priority: 10, slug: 'first', version: '1.0.0',
}
const base = { manifestSha256: 'F'.repeat(64), mods: [baseMod] }
const cases = []

function expand(value) {
  if (value === '$BONEYARD') return boneyardBase64
  if (value === '$FILE_LIMIT') return Buffer.alloc(16 * 1024 * 1024, 97).toString('base64')
  if (value === '$FILE_OVERFLOW') return Buffer.alloc(16 * 1024 * 1024 + 1, 97).toString('base64')
  if (Array.isArray(value)) return value.map(expand)
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([key, child]) => [key, expand(child)]))
  }
  return value
}

function project(content) {
  return {
    ...content,
    modSources: content.modSources.map(source => ({
      ...source,
      files: Object.fromEntries(Object.entries(source.files).map(([path, bytes]) => [path, [...bytes]])),
    })),
  }
}

function record(name, input) {
  let output = null
  let error = null
  try { output = project(materializeWebSessionContent(expand(input))) }
  catch (caught) { error = caught.message }
  cases.push({ name, input, output, error })
}

function mutate(name, change) {
  const input = structuredClone(base)
  change(input, input.mods[0])
  record(name, input)
}

function file(path, kind, contentType, bytes = Buffer.from('package-data')) {
  return {
    byteLength: bytes.length, bytesBase64: bytes.toString('base64'), contentType, kind, path,
    sha256: createHash('sha256').update(bytes).digest('hex').toUpperCase(),
  }
}

record('empty', { manifestSha256: '0'.repeat(64), mods: [] })
record('one-package', base)
mutate('all-typed-package-paths', (_, mod) => {
  mod.entryScript = 'return sd.mod({api="1.0.0"})'
  mod.files = [
    file('art/sprite.png', 'image', 'image/png'),
    file('sprites/hero.bundle', 'sprite-bundle', 'application/vnd.solomon-dark.sprite-bundle'),
    file('art/meta.json', 'art-metadata', 'application/json'),
    file('audio/a.ogg', 'audio', 'audio/ogg'), file('audio/b.wav', 'audio', 'audio/wav'),
    file('audio/c.mp3', 'audio', 'audio/mpeg'),
    file('levels/a.boneyard', 'boneyard', 'application/vnd.solomon-dark.boneyard'),
    file('levels/a.json', 'level-metadata', 'application/json'),
    file('scenes/a.json', 'scene', 'application/json'),
    // The source does not reject a dot segment at this admission stage.
    file('art/../retained.png', 'image', 'image/png'),
  ]
})
const target = 'sandbox/DarkCloud/mylevels/Contract.boneyard'
mutate('legacy-overlay-last-owner-first-position', (input, mod) => {
  mod.boneyards = [{ target, bytesBase64: '$BONEYARD' },
    { target: 'data/levels/Other.boneyard.boneyard', bytesBase64: '$BONEYARD' }]
  input.mods.push({ ...structuredClone(baseMod), id: 'tests.second', name: 'Second', priority: -30,
    boneyards: [{ target: target.toUpperCase().replace('SANDBOX/DARKCLOUD/MYLEVELS', 'sandbox/DarkCloud/mylevels').replace('.BONEYARD', '.boneyard'), bytesBase64: '$BONEYARD' }] })
})
mutate('target-final-line-ending', (_, mod) => {
  mod.boneyards = [{ target: `${target}\n`, bytesBase64: '$BONEYARD' }]
})
record('not-an-object', [])
mutate('extra-root-field', input => { input.extra = true })
mutate('missing-root-field', input => { delete input.mods })
mutate('bad-manifest', input => { input.manifestSha256 = 'x'.repeat(64) })
mutate('too-many-mods', input => { input.mods = Array.from({ length: 129 }, () => baseMod) })
mutate('duplicate-owner-case-insensitive', input => { input.mods.push({ ...baseMod, id: 'TESTS.FIRST' }) })
mutate('extra-mod-field', (_, mod) => { mod.extra = true })
mutate('missing-nullable-entrypoint', (_, mod) => { delete mod.entryScript })
mutate('bad-id', (_, mod) => { mod.id = '-first' })
mutate('bad-priority', (_, mod) => { mod.priority = 0.25 })
mutate('empty-script', (_, mod) => { mod.entryScript = '' })
mutate('script-byte-limit', (_, mod) => { mod.entryScript = 'é'.repeat(131073) })
mutate('too-many-lua-mods', input => {
  input.mods = Array.from({ length: 9 }, (_, index) => ({ ...baseMod, id: `mod.${index}`, entryScript: '-- accepted' }))
})
mutate('too-many-files', (_, mod) => { mod.files = Array.from({ length: 257 }, () => ({})) })
for (const [name, change] of [
  ['missing-file-field', value => { delete value.kind }],
  ['bad-file-path', value => { value.path = 'private/a.png' }],
  ['wrong-file-mime', value => { value.contentType = 'audio/ogg' }],
  ['wrong-file-kind', value => { value.kind = 'audio' }],
  ['noncanonical-base64', value => { value.bytesBase64 = 'YQ' }],
  ['empty-bytes', value => { value.bytesBase64 = '' }],
  ['wrong-byte-length', value => { value.byteLength = 5 }],
  ['wrong-digest', value => { value.sha256 = '0'.repeat(64) }],
  ['invalid-digest', value => { value.sha256 = 'z'.repeat(64) }],
]) {
  mutate(name, (_, mod) => { const value = file('art/a.png', 'image', 'image/png'); change(value); mod.files = [value] })
}
mutate('duplicate-portable-path', (_, mod) => {
  mod.files = [file('art/A.png', 'image', 'image/png'), file('art/a.png', 'image', 'image/png')]
})
mutate('bad-boneyard-target', (_, mod) => { mod.boneyards = [{ target: 'images/a.png', bytesBase64: 'YQ==' }] })
mutate('bad-boneyard-base64', (_, mod) => { mod.boneyards = [{ target, bytesBase64: 'YQ' }] })
mutate('bad-boneyard-bytes', (_, mod) => { mod.boneyards = [{ target, bytesBase64: 'YQ==' }] })
mutate('aggregate-byte-limit', (_, mod) => {
  mod.entryScript = '--'
  const bytes = Buffer.alloc(16 * 1024 * 1024, 97)
  mod.files = [file('art/a.png', 'image', 'image/png', bytes), file('art/b.png', 'image', 'image/png', bytes)]
  for (const value of mod.files) value.bytesBase64 = '$FILE_LIMIT'
})
mutate('individual-byte-limit', (_, mod) => {
  const value = file('art/a.png', 'image', 'image/png', Buffer.alloc(16 * 1024 * 1024 + 1, 97))
  value.bytesBase64 = '$FILE_OVERFLOW'
  mod.files = [value]
})
writeFixture('mod-package-goldens.json', 'mod-package-goldens.mjs', 'host/web-mod-content.ts and host/boneyard-catalog.ts', { boneyardBase64, cases })
