// Immutable definition identity and vocabulary, from the pinned Website.
import { loadKernel, writeFixture } from './oracle.mjs'

const identity = await loadKernel('modding/definition/web-lua-content-identity.ts')
const types = await loadKernel('modding/definition/web-lua-definition-types.ts')
const language = await loadKernel('modding/definition/web-lua-definition-language.ts')

const keys = [
  'a', '0', 'frost.bolt', 'frost_bolt', 'frost-bolt', 'a'.repeat(128),
  '', '.a', 'a.', '-a', 'a-', '_a', 'a_', 'A', 'a/b', 'a\0b', 'é', 'a'.repeat(129),
]
const ids = []
for (const [modId, key] of [
  ['example.mod', 'frost.bolt'],
  ['example.mod', 'frost_bolt'],
  ['example.mod', 'frost-bolt'],
  ['a', 'bc'],
  ['ab', 'c'],
  ['Mód.雷', 'frost.bolt'],
  ['', ''],
  ['example.mod', 'a'.repeat(128)],
]) ids.push({ modId, key, contentId: identity.stableWebLuaContentId(modId, key) })

writeFixture(
  'mod-definition-goldens.json',
  'mod-definition-goldens.mjs',
  'modding/definition/web-lua-{content-identity,definition-types,definition-language}.ts',
  {
    ids,
    keys: keys.map(key => ({ key, valid: identity.validWebLuaContentKey(key) })),
    contentKinds: types.WEB_LUA_CONTENT_KINDS,
    artSlots: types.WEB_LUA_CONTENT_ART_SLOTS,
    assetKinds: types.WEB_LUA_ASSET_KINDS,
    scopeKinds: types.WEB_LUA_SCOPE_KINDS,
    eventNames: types.WEB_LUA_RULE_EVENT_NAMES,
    ruleNames: language.WEB_LUA_RULE_NAMES,
    effectNames: language.WEB_LUA_EFFECT_NAMES,
    prefabNames: language.WEB_LUA_PREFAB_NAMES,
    schemaNames: language.WEB_LUA_SCHEMA_NAMES,
    artAliasNames: language.WEB_LUA_ART_ALIAS_NAMES,
    limits: types.DEFAULT_WEB_LUA_DEFINITION_LIMITS,
  },
)
