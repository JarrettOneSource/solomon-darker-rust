// Copy the actual pinned Lua sandbox; Rust only supplies its native host bridge.
import { writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, root } from './oracle.mjs'
const { WEB_LUA_DEFINITION_SANDBOX_BOOTSTRAP } = await loadKernel('modding/definition/web-lua-definition-sandbox.ts')
writeFileSync(resolve(root, 'crates/modding/src/definition/bootstrap.lua'), WEB_LUA_DEFINITION_SANDBOX_BOOTSTRAP)
