// The actual provisioner compiles each admitted package in dependency order.
import { createHash } from 'node:crypto'
import { createRequire } from 'node:module'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const require = createRequire(resolve(websiteDir, 'frontend/package.json'))
const wasmPath = require.resolve('wasmoon/dist/glue.wasm')
const { materializeWebSessionContent, compileWebSessionContentDefinitions } = await loadKernel('host/web-mod-content.ts')
const bytes = readFileSync(resolve(websiteDir, 'tests/fixtures/flat_multiplayer_test.boneyard'))
const boneyardBase64 = bytes.toString('base64')
const sourceFile = { byteLength: bytes.length, bytesBase64: '$BONEYARD', contentType: 'application/vnd.solomon-dark.boneyard', kind: 'boneyard', path: 'levels/test.boneyard', sha256: createHash('sha256').update(bytes).digest('hex') }
const baseMod = { boneyards: [], contentSha256: 'a'.repeat(64), entryScript: null, files: [], id: 'provision.first', name: 'First', priority: 0, slug: 'first', version: '1.0.0' }
const cases = []
function expand(value) {
  if (value === '$BONEYARD') return boneyardBase64
  if (Array.isArray(value)) return value.map(expand)
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, child]) => [key, expand(child)]))
  return value
}
async function record(name, mods) {
  const input = { manifestSha256: 'f'.repeat(64), mods }
  let output = null
  let error = null
  try {
    const content = await compileWebSessionContentDefinitions(materializeWebSessionContent(expand(input)), wasmPath)
    output = { assets: content.assets, compiledMods: content.compiledMods, manifest: content.manifest, summary: content.summary,
      boneyards: content.boneyards.map(entry => ({ choice: entry.choice, geometrySha256: entry.geometrySha256, sourceSha256: entry.sourceSha256, webLuaContentId: entry.webLuaContentId ?? null, environmentMode: entry.scene.environmentMode })) }
  } catch (caught) { error = caught.message }
  cases.push({ name, input, output, error })
}
await record('no-enabled-lua', [baseMod])
await record('one-real-definition', [{ ...baseMod, entryScript: 'sd.item{key="token",name="Token"}' }])
const first = { ...baseMod, entryScript: 'sd.status{key="ward"}' }
const second = { ...baseMod, id: 'provision.second', name: 'Second', entryScript: 'sd.potion{key="tonic",name="Tonic",on_use=sd.effect.status{target="user",status=sd.ref("status","ward","provision.first")}}' }
await record('prior-mod-reference', [first, second])
await record('future-mod-reference-rejected', [second, first])
await record('authored-boneyard-name-environment-and-legacy-order', [{ ...baseMod, files: [sourceFile], boneyards: [{ bytesBase64: '$BONEYARD', target: 'data/levels/Legacy.boneyard' }], entryScript: 'sd.boneyard{key="yard",name="Authored Yard",source="levels/test.boneyard",environment={mode=2}}' }])
await record('missing-authored-boneyard-bytes', [{ ...baseMod, entryScript: 'sd.boneyard{key="yard",source="levels/missing.boneyard"}' }])
await record('included-definition', [{ ...baseMod, entryScript: `sd.include("scripts/items.lua")\n--@sd-bundle ${JSON.stringify({ 'scripts/items.lua': 'sd.item{key="included",name="Included"}' })}` }])
await record('definition-rejection-keeps-source-site', [{ ...baseMod, entryScript: 'sd.kit.itme{key="bad"}' }])
await record('graph-rejection-keeps-source-site', [{ ...baseMod, entryScript: 'sd.potion{key="bad",name="Bad",duration="forever",mystery=true}' }])
writeFixture('mod-provisioning-goldens.json', 'mod-provisioning-goldens.mjs', 'host/web-mod-content.ts::compileWebSessionContentDefinitions', { boneyardBase64, cases })
