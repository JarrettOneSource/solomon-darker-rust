// Actual current authoring CLI; all writable packages live outside the reference.
import { createHash } from 'node:crypto'
import { mkdtemp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { websiteDir, writeFixture } from './oracle.mjs'

const { runSdmod } = await import(pathToFileURL(resolve(websiteDir, 'frontend/tools/sdmod/cli.mjs')))
const { readZipEntries } = await import(pathToFileURL(resolve(websiteDir, 'frontend/tools/sdmod/zip-reader.mjs')))
const scratch = await mkdtemp(join(tmpdir(), 'sdmod-source-oracle-'))
const silent = { log() {} }
const digest = value => createHash('sha256').update(value).digest('hex')
try {
  const base = join(scratch, 'fixture-base')
  await runSdmod(['new', base], silent)
  const manifest = JSON.parse(await readFile(join(base, 'manifest.json'), 'utf8'))
  const entry = await readFile(join(base, 'scripts/main.lua'), 'utf8')
  const icon = (await readFile(join(base, 'art/icon.png'))).toString('base64')
  const cases = []
  async function record(name, changes = {}) {
    const root = join(scratch, name)
    const input = { manifest: structuredClone(manifest), entry, scripts: {},
      files: [{ path: 'art/icon.png', bytes: icon }], tests: {}, ...changes }
    await mkdir(join(root, 'scripts'), { recursive: true })
    await writeFile(join(root, 'manifest.json'), JSON.stringify(input.manifest))
    const entryPath = input.manifest.runtime?.entryScript
    if (typeof entryPath === 'string' && !entryPath.includes('..') && !entryPath.startsWith('/')) {
      await mkdir(join(root, entryPath, '..'), { recursive: true })
      await writeFile(join(root, entryPath), input.entry)
    }
    for (const [path, text] of Object.entries(input.scripts)) {
      await mkdir(join(root, path, '..'), { recursive: true })
      await writeFile(join(root, path), text)
    }
    for (const file of input.files) {
      await mkdir(join(root, file.path, '..'), { recursive: true })
      await writeFile(join(root, file.path), Buffer.from(file.bytes, 'base64'))
    }
    for (const [path, text] of Object.entries(input.tests)) {
      await mkdir(join(root, 'tests'), { recursive: true })
      await writeFile(join(root, 'tests', path), text)
    }
    const output = []
    try {
      await runSdmod([changes.command ?? 'check', root], { log: text => output.push(text) })
      cases.push({ name, input, expected: JSON.parse(output.at(-1)) })
    } catch (error) {
      cases.push({ name, input, error: error.message })
    }
  }
  await record('starter')
  await record('includes', {
    entry: 'local things = sd.include("scripts/items.lua")\nsd.potion({name="Stone Tonic",duration="2s",icon="art/icon.png",on_use=sd.effect.grant({target="user",item=things.stone})})\n',
    scripts: { 'scripts/items.lua': 'return {stone=sd.item({name="Moon Stone",icon="art/icon.png"})}\n' },
  })
  for (const [name, patch] of [
    ['unknown-manifest', { surprise: true }],
    ['invalid-id', { id: 'Bad Package' }],
    ['empty-name', { name: '' }],
    ['long-name', { name: '😀'.repeat(41) }],
    ['invalid-version', { version: '_broken' }],
    ['fractional-priority', { priority: 0.5 }],
    ['invalid-overlays', { overlays: {} }],
    ['duplicate-required-mods', { requiredMods: ['other', 'other'] }],
    ['old-api', { runtime: { apiVersion: '0.2.0', entryScript: 'scripts/main.lua' } }],
    ['unknown-runtime', { runtime: { ...manifest.runtime, surprise: true } }],
    ['entry-outside-scripts', { runtime: { ...manifest.runtime, entryScript: 'main.lua' } }],
    ['entry-traversal', { runtime: { ...manifest.runtime, entryScript: 'scripts/../main.lua' } }],
  ]) await record(name, { manifest: { ...manifest, ...patch } })
  await record('missing-art', { files: [] })
  await record('empty-art', { files: [{ path: 'art/icon.png', bytes: '' }] })
  await record('invalid-png', { files: [{ path: 'art/icon.png', bytes: Buffer.from('bad PNG').toString('base64') }] })
  await record('entry-byte-budget', { entry: `--${'a'.repeat(256 * 1024)}\n` })
  await record('bundled-byte-budget', { scripts: { 'scripts/extra.lua': `--${'a'.repeat(256 * 1024 - 34)}\n` } })
  await record('included-script-count', {
    scripts: Object.fromEntries(Array.from({ length: 65 }, (_, index) => [`scripts/part${index}.lua`, ''])),
  })
  await record('typed-asset-count', {
    files: [{ path: 'art/icon.png', bytes: icon },
      ...Array.from({ length: 256 }, (_, index) => ({ path: `art/extra${index}.png`, bytes: 'AA==' }))],
  })
  for (const [name, code] of [
    ['test-true', 'assert(mod.content[1].key == "starter_item"); return true'],
    ['test-nil', 'assert(io == nil and os == nil and require == nil and debug == nil)'],
    ['test-false', 'return false'],
    ['test-zero', 'return 0'],
    ['test-explicit-nil', 'return nil'],
    ['test-first-return', 'return nil, false'],
    ['test-first-true', 'return true, false'],
  ]) await record(name, { command: 'test', tests: { 'basic.lua': code } })
  await record('test-integer-literals', {
    command: 'test',
    entry: 'sd.item({name="Counting Stone",icon="art/icon.png",stack={maximum=2}})',
    tests: { 'basic.lua': 'assert(math.type(mod.content[1].fields.stack.maximum) == "integer"); return true' },
  })

  const generatedRoot = join(scratch, 'reference')
  await runSdmod(['generate', generatedRoot], silent)
  await runSdmod(['generate', generatedRoot, '--check'], silent)
  const reference = []
  for (const path of (await readdir(generatedRoot)).sort()) {
    reference.push({ path, contents: await readFile(join(generatedRoot, path), 'utf8') })
  }
  const packed = join(scratch, 'starter.zip')
  await runSdmod(['pack', base, packed], silent)
  const includedZip = join(scratch, 'includes.zip')
  await runSdmod(['pack', join(scratch, 'includes'), includedZip], silent)
  const includedBundleEntry = readZipEntries(await readFile(includedZip), ['scripts/main.lua'])
    .get('scripts/main.lua').toString('utf8')
  writeFixture('sdmod-authoring-goldens.json', 'tools/website-oracle/sdmod-authoring-goldens.mjs',
    'website-rust-current tools/sdmod/cli.mjs + modding/definition/web-lua-package-authoring.ts',
    { sourceRevision: 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83', cases, reference,
      starterArchiveSha256: digest(await readFile(packed)), includedBundleEntry })
} finally {
  await rm(scratch, { recursive: true, force: true })
}
