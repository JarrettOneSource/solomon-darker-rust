// Diagnostic: Lua hash-map iteration is private VM state, while the actual
// Source compiler canonicalizes admitted assets before gameplay consumes them.
import { createRequire } from 'node:module'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, websiteDir } from './oracle.mjs'
const require = createRequire(resolve(websiteDir, 'frontend/package.json'))
const { WebLuaDefinitionRuntime } = await loadKernel('modding/definition/web-lua-definition-runtime.ts')
const { compileWebLuaDefinition } = await loadKernel('modding/definition/web-lua-definition-compiler.ts')
const source = readFileSync(resolve(websiteDir, 'frontend/src/game/modding/definition/web-lua-definition-runtime.test.ts'), 'utf8')
const code = source.split('const definition = runtime.run(`')[1].split('`)')[0]
const fixture = JSON.parse(readFileSync(new URL('../../crates/sim/tests/fixtures/website/mod-runtime-goldens.json', import.meta.url)))
const identity = fixture.identity
const recorded = fixture.cases[0].definition
process.stdout.write(JSON.stringify({ kind: 'recorded-source', assets: recorded.assets.map(asset => asset.key) }) + '\n')
for (let index = 0; index < 12; index += 1) {
  const runtime = await WebLuaDefinitionRuntime.create({ entryScript: 'scripts/main.lua', identity, wasmPath: require.resolve('wasmoon/dist/glue.wasm') })
  try {
    const definition = runtime.run(code)
    const compiled = compileWebLuaDefinition(identity, definition)
    process.stdout.write(JSON.stringify({ kind: 'fresh-source', index, assets: definition.assets.map(asset => asset.key), graphSha256: compiled.graphSha256 }) + '\n')
  } finally { runtime.close() }
}
