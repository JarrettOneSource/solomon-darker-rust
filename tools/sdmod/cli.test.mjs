import { createHash } from 'node:crypto'
import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import {
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { promisify } from 'node:util'
import test from 'node:test'

import { runSdmod } from './cli.mjs'
import { fetchAssetSource } from './assets.mjs'
import { writeDeterministicZip } from './zip.mjs'
import { readZipEntries } from './zip-reader.mjs'
import { checkPackage, readAuthoringInventory } from './native.mjs'

const { apiVersion: WEB_LUA_DEFINITION_API_VERSION, maximumScriptBytes: WEB_LUA_MAX_SCRIPT_BYTES } = await readAuthoringInventory()
const fixture = JSON.parse(await readFile(new URL('../../crates/sim/tests/fixtures/website/sdmod-authoring-goldens.json', import.meta.url), 'utf8'))

const execFileAsync = promisify(execFile)

test('sdmod creates, checks, tests, packs, and generates one deterministic v1 mod', async (context) => {
  const root = await mkdtemp(join(tmpdir(), 'sdmod-v1-'))
  context.after(() => rm(root, { force: true, recursive: true }))
  const mod = join(root, 'example-item')
  const output = join(root, 'example-item.zip')
  const secondOutput = join(root, 'example-item-second.zip')
  const generated = join(root, 'generated')
  const messages = []
  const io = { log: value => messages.push(String(value)) }

  await runSdmod(['new', mod], io)
  await mkdir(join(mod, 'tests'))
  await writeFile(join(mod, 'tests/basic.lua'), 'assert(mod.content[1].key == "starter_item")\nreturn true\n')
  const checked = await runSdmod(['check', mod], io)
  await runSdmod(['test', mod], io)
  await runSdmod(['pack', mod, output], io)
  await runSdmod(['pack', mod, secondOutput], io)
  await runSdmod(['generate', generated], io)
  await runSdmod(['generate', generated, '--check'], io)
  for (const file of fixture.reference) assert.equal(await readFile(join(generated, file.path), 'utf8'), file.contents, file.path)

  assert.equal(checked.compiled.content[0]?.contentKind, 'item')
  assert.deepEqual(await readFile(output), await readFile(secondOutput))
  const { stdout } = await execFileAsync('unzip', ['-Z1', output])
  assert.deepEqual(stdout.trim().split('\n'), [
    'art/icon.png',
    'compiled/graph.json',
    'compiled/graph.sha256',
    'manifest.json',
    'scripts/main.lua',
    'THIRD_PARTY_ASSETS.md',
  ])
  assert.match(await readFile(join(generated, 'sd.lua'), 'utf8'), /---@field potion/)
  const luaLs = await readFile(join(generated, 'sd.lua'), 'utf8')
  assert.match(luaLs, /wearable fun\(path: string, options\?: table\)/)
  assert.match(luaLs, /sheet fun\(path_or_spec: string\|table, options\?: table\)/)
  const potionSpec = luaLs.slice(luaLs.indexOf('---@class SdPotionSpec'), luaLs.indexOf('---@class SdPowerupSpec'))
  assert.match(potionSpec, /---@field duration\? SdDuration/)
  assert.match(potionSpec, /---@field on_use\? SdRule\|SdRule\[\]/)
  assert.match(await readFile(join(generated, 'sd.lua'), 'utf8'), /---@class SdPredicate/)
  const reference = await readFile(join(generated, 'REFERENCE.md'), 'utf8')
  assert.match(reference, /sd\.include/)
  assert.match(reference, /Reducers are collected automatically/)
  assert.match(reference, /npm run sdmod -- check/)
  assert.match(await readFile(join(generated, 'DIAGNOSTICS.md'), 'utf8'), /E_SCRIPT/)
  assert.match(await readFile(join(generated, 'REFERENCE.md'), 'utf8'), /scene-extension/)
  assert.match(await readFile(join(generated, 'REFERENCE.md'), 'utf8'), /sd\.art\.wearable/)
  assert.match(await readFile(join(generated, 'STARTER.lua'), 'utf8'), /npm run sdmod -- check/)
  const inventory = JSON.parse(await readFile(join(generated, 'web-lua-definition.schema.json'), 'utf8'))
  assert.deepEqual(inventory.content.potion.required, ['name'])
  assert.deepEqual(inventory.contentIdentity, { oneOf: ['key', 'name'] })
  assert.ok(messages.some(message => message.includes('graphSha256')))
  assert.ok(messages.some(message => message.includes('Next: npm run sdmod -- check')))
})

test('sdmod migrates the retained Invincibility Potion to the v1 kit', async (context) => {
  const root = await mkdtemp(join(tmpdir(), 'sdmod-migrate-'))
  context.after(() => rm(root, { force: true, recursive: true }))
  const source = join(root, 'old')
  const target = join(root, 'new')
  await mkdir(join(source, 'sprites'), { recursive: true })
  await writeFile(join(source, 'manifest.json'), JSON.stringify({
    id: 'canary.lua.invincibility_potion',
    name: 'Invincibility Potion Canary',
    version: '0.3.0',
    minimumLoaderVersion: '0.1.0-beta.29',
    runtime: {
      apiVersion: '0.2.0',
      entryScript: 'scripts/main.lua',
      requiredCapabilities: ['items.consumables.register'],
    },
  }))
  await writeFile(join(source, 'sprites/invincibility_potion.png'), Buffer.from('png'))
  await writeFile(join(source, 'sprites/invincibility_potion.bundle'), Buffer.from('bundle'))

  await runSdmod(['migrate', source, target], { log: () => {} })
  const manifest = JSON.parse(await readFile(join(target, 'manifest.json'), 'utf8'))
  const script = await readFile(join(target, 'scripts/main.lua'), 'utf8')

  assert.equal(manifest.runtime.apiVersion, WEB_LUA_DEFINITION_API_VERSION)
  assert.equal(manifest.minimumLoaderVersion, undefined)
  assert.equal(manifest.runtime.requiredCapabilities, undefined)
  assert.match(script, /sd\.kit\.potion/)
  assert.match(script, /stacking = "refresh"/)
  assert.match(script, /ordinary = 0\.5, boss = 1\.0/)
})

test('sdmod rejects targets and packages outside the v1 contract', async (context) => {
  const root = await mkdtemp(join(tmpdir(), 'sdmod-reject-'))
  context.after(() => rm(root, { force: true, recursive: true }))
  const mod = join(root, 'bad')
  await mkdir(join(mod, 'scripts'), { recursive: true })
  await writeFile(join(mod, 'manifest.json'), JSON.stringify({
    id: 'Bad Package',
    name: 'Bad',
    version: '1.0.0',
    runtime: { apiVersion: '0.2.0', entryScript: 'scripts/main.lua' },
  }))
  await writeFile(join(mod, 'scripts/main.lua'), 'return true\n')

  await assert.rejects(() => runSdmod(['check', mod]), /canonical lowercase package id/)
  await assert.rejects(() => runSdmod(['new', 'unknown-kind', join(root, 'new')]), /usage:/)
})

test('asset tooling verifies downloads and reads only requested ZIP entries', async (context) => {
  const root = await mkdtemp(join(tmpdir(), 'sdmod-assets-'))
  context.after(() => rm(root, { force: true, recursive: true }))
  const archive = join(root, 'sample.zip')
  await writeDeterministicZip(archive, new Map([
    ['keep.txt', Buffer.from('kept')],
    ['skip.txt', Buffer.from('skipped')],
  ]))
  assert.deepEqual(
    Object.fromEntries(readZipEntries(await readFile(archive), ['keep.txt'])),
    { 'keep.txt': Buffer.from('kept') },
  )

  const sourceBytes = Buffer.from('blend-source')
  const originalFetch = globalThis.fetch
  globalThis.fetch = async () => new Response(sourceBytes)
  context.after(() => { globalThis.fetch = originalFetch })
  const previousCache = process.env.SDR_MOD_ASSET_CACHE
  process.env.SDR_MOD_ASSET_CACHE = join(root, 'cache')
  context.after(() => {
    if (previousCache === undefined) delete process.env.SDR_MOD_ASSET_CACHE
    else process.env.SDR_MOD_ASSET_CACHE = previousCache
  })
  await fetchAssetSource({
    archiveUrl: 'https://assets.example/model.blend',
    downloadKind: 'file',
    fileName: 'model.blend',
    id: 'example-model-1.0',
    license: 'CC0-1.0',
    licenseFile: null,
    selectedFiles: ['model.blend'],
    sha256: createHash('sha256').update(sourceBytes).digest('hex'),
  })
  assert.deepEqual(
    await readFile(join(root, 'cache/example-model-1.0/model.blend')),
    sourceBytes,
  )
})

test('sdmod folds included scripts into the packed entry script and keeps the generated starter valid', async (context) => {
  const root = await mkdtemp(join(tmpdir(), 'sdmod-include-'))
  context.after(() => rm(root, { force: true, recursive: true }))
  const io = { log: () => {} }
  const mod = join(root, 'split-mod')
  const output = join(root, 'split-mod.zip')

  await runSdmod(['new', mod], io)
  await writeFile(join(mod, 'scripts/items.lua'), [
    'local items = {}',
    'items.stone = sd.item({name = "Moon Stone", icon = "art/icon.png"})',
    'return items',
    '',
  ].join('\n'))
  await writeFile(join(mod, 'scripts/main.lua'), [
    'local items = sd.include("scripts/items.lua")',
    'sd.potion({',
    '  name = "Stone Tonic",',
    '  duration = "2s",',
    '  icon = "art/icon.png",',
    '  on_use = sd.effect.grant({target = "user", item = items.stone}),',
    '})',
    '',
  ].join('\n'))
  const checked = await runSdmod(['check', mod], io)
  assert.deepEqual(checked.compiled.content.map(entry => entry.key), ['moon_stone', 'stone_tonic'])
  await runSdmod(['pack', mod, output], io)
  const entries = Object.fromEntries(readZipEntries(
    await readFile(output),
    ['scripts/main.lua', 'scripts/items.lua', 'compiled/graph.sha256'],
  ))
  const packedEntry = entries['scripts/main.lua'].toString('utf8')
  assert.match(packedEntry, /^--@sd-bundle \{"scripts\/items\.lua":/m)
  assert.equal(entries['scripts/items.lua'].toString('utf8').split('\n')[0], 'local items = {}')
  const packedRoot = join(root, 'packed')
  await mkdir(join(packedRoot, 'scripts'), { recursive: true })
  await mkdir(join(packedRoot, 'art'), { recursive: true })
  await writeFile(join(packedRoot, 'manifest.json'), JSON.stringify(checked.manifest))
  await writeFile(join(packedRoot, 'art/icon.png'), checked.files.get('art/icon.png'))
  await writeFile(join(packedRoot, 'scripts/main.lua'), packedEntry)
  const packed = await checkPackage(packedRoot)
  assert.equal(packed.compiled.graphSha256, entries['compiled/graph.sha256'].toString('utf8').trim())
  assert.equal(packed.compiled.graphSha256, checked.compiled.graphSha256)

  const generated = join(root, 'generated')
  await runSdmod(['generate', generated], io)
  const starter = join(root, 'starter')
  await runSdmod(['new', starter], io)
  await copyFile(join(generated, 'STARTER.lua'), join(starter, 'scripts/main.lua'))
  const starterChecked = await runSdmod(['check', starter], io)
  assert.deepEqual(
    starterChecked.compiled.content.map(entry => `${entry.contentKind}:${entry.key}`),
    ['item:example_item', 'potion:example_potion', 'status:example_tough'],
  )
  assert.equal(starterChecked.compiled.rules.length, 1)
})

test('sdmod check rejects a source tree that cannot fit in the packed script budget', async (context) => {
  const root = await mkdtemp(join(tmpdir(), 'sdmod-packed-budget-'))
  context.after(() => rm(root, { force: true, recursive: true }))
  const mod = join(root, 'too-large-when-bundled')
  const io = { log: () => {} }
  await runSdmod(['new', mod], io)

  const half = Math.floor(WEB_LUA_MAX_SCRIPT_BYTES / 2)
  await writeFile(join(mod, 'scripts/main.lua'), `--${'a'.repeat(half)}\nsd.item({name = "Thing", icon = "art/icon.png"})\n`)
  await writeFile(join(mod, 'scripts/extra.lua'), `--${'b'.repeat(half)}\nreturn {}\n`)

  await assert.rejects(
    () => runSdmod(['check', mod], io),
    new RegExp(`entry script and its included scripts total \\d+ bytes, above the ${WEB_LUA_MAX_SCRIPT_BYTES} byte limit`),
  )
})


test('sdmod starter ZIP matches the current Source bytes', async context => {
  const root = await mkdtemp(join(tmpdir(), 'sdmod-source-zip-'))
  context.after(() => rm(root, { force: true, recursive: true }))
  const mod = join(root, 'fixture-base')
  const output = join(root, 'starter.zip')
  const io = { log() {} }
  await runSdmod(['new', mod], io)
  await runSdmod(['pack', mod, output], io)
  const hash = createHash('sha256').update(await readFile(output)).digest('hex')
  assert.equal(hash, fixture.starterArchiveSha256)
})
