import {
  copyFile,
  mkdir,
  readFile,
  readdir,
  stat,
  writeFile,
} from 'node:fs/promises'
import { basename, dirname, join, relative, resolve, sep } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

import { writeDeterministicZip } from './zip.mjs'
import { runAssetCommand } from './assets.mjs'
import { renderSprite } from './render-sprite.mjs'
import { bundlePackage, checkPackage, readAuthoringInventory, testPackage } from './native.mjs'

// The generator reads the same immutable inventory the Rust compiler validates.
const {
  apiVersion: WEB_LUA_DEFINITION_API_VERSION,
  contentKinds: WEB_LUA_CONTENT_KINDS,
  contentSchemaFields: WEB_LUA_CONTENT_SCHEMA_FIELDS,
  contentArtSlots: WEB_LUA_CONTENT_ART_SLOTS,
  definitionErrorCodes: WEB_LUA_DEFINITION_ERROR_CODES,
  maximumIncludedScripts: WEB_LUA_MAX_INCLUDED_SCRIPTS,
  maximumScriptBytes: WEB_LUA_MAX_SCRIPT_BYTES,
  ruleEventNames: WEB_LUA_RULE_EVENT_NAMES,
  scopeKinds: WEB_LUA_SCOPE_KINDS,
  artAliases: ART_ALIASES,
  effects: EFFECTS,
  predicateFields: WEB_LUA_PREDICATE_FIELDS,
  prefabs: PREFABS,
  rules: RULES,
  schemas: SCHEMAS,
} = await readAuthoringInventory()

export async function runSdmod(argv, io = console) {
  const [command, ...args] = argv
  switch (command) {
    case 'new': return newMod(args, io)
    case 'assets': return runAssetCommand(args, io)
    case 'check': return checkMod(args, io)
    case 'test': return testMod(args, io)
    case 'pack': return packMod(args, io)
    case 'migrate': return migrateMod(args, io)
    case 'generate': return generateReference(args, io)
    case 'render-sprite': return renderSprite(args, io)
    default: throw new Error(usage())
  }
}

async function newMod(args, io) {
  const [target] = args
  if (!target || args.length !== 1) throw new Error(usage())
  const root = resolve(target)
  await requireEmptyDirectory(root)
  await mkdir(join(root, 'scripts'), { recursive: true })
  await mkdir(join(root, 'art'), { recursive: true })
  const id = canonicalId(basename(root))
  const name = displayName(id)
  await writeFile(join(root, 'manifest.json'), `${JSON.stringify({
    $schema: '/mod-manifest.schema.json',
    id,
    name,
    version: '1.0.0',
    runtime: {
      apiVersion: WEB_LUA_DEFINITION_API_VERSION,
      entryScript: 'scripts/main.lua',
    },
    requiredMods: [],
  }, null, 2)}\n`)
  await writeFile(join(root, 'scripts/main.lua'), starterModTemplate(name))
  await copyFile(
    fileURLToPath(new URL('./starter-icon.png', import.meta.url)),
    join(root, 'art/icon.png'),
  )
  await writeFile(join(root, 'THIRD_PARTY_ASSETS.md'), `# Third-party assets

- \`art/icon.png\`: Kenney Game Icons, CC0 1.0, https://kenney.nl/assets/game-icons
`)
  await checkPackage(root)
  io.log([
    `Created valid starter mod at ${root}`,
    `Next: npm run sdmod -- check ${JSON.stringify(root)}`,
  ].join('\n'))
}

async function checkMod(args, io) {
  const [target] = args
  if (!target) throw new Error(usage())
  const checked = await checkPackage(resolve(target))
  io.log(JSON.stringify(summary(checked), null, 2))
  return checked
}

async function testMod(args, io) {
  const [target] = args
  if (!target) throw new Error(usage())
  const { output, ...summary } = await testPackage(resolve(target))
  for (const line of output) console.log(line)
  io.log(JSON.stringify(summary, null, 2))
}

async function packMod(args, io) {
  const [target, output] = args
  if (!target || !output) throw new Error(usage())
  const checked = await bundlePackage(resolve(target))
  const files = await packageEntries(checked.root)
  if (checked.scripts.size > 0) {
    files.set(checked.entryScriptPath, Buffer.from(checked.bundledEntryScript))
  }
  files.set('compiled/graph.json', Buffer.from(`${checked.compiled.canonicalJson}\n`))
  files.set('compiled/graph.sha256', Buffer.from(`${checked.compiled.graphSha256}\n`))
  const outputPath = resolve(output)
  await mkdir(dirname(outputPath), { recursive: true })
  await writeDeterministicZip(outputPath, files)
  io.log(JSON.stringify({ ...summary(checked), output: outputPath }, null, 2))
}

async function migrateMod(args, io) {
  const [sourceArg, targetArg] = args
  if (!sourceArg || !targetArg) throw new Error(usage())
  const source = resolve(sourceArg)
  const target = resolve(targetArg)
  const manifest = JSON.parse(await readFile(join(source, 'manifest.json'), 'utf8'))
  if (manifest.id !== 'canary.lua.invincibility_potion') {
    throw new Error('automatic migration currently supports only canary.lua.invincibility_potion')
  }
  await requireEmptyDirectory(target)
  await mkdir(join(target, 'scripts'), { recursive: true })
  await mkdir(join(target, 'art'), { recursive: true })
  await copyFile(
    join(source, 'sprites/invincibility_potion.png'),
    join(target, 'art/invincibility_potion.png'),
  )
  await copyFile(
    join(source, 'sprites/invincibility_potion.bundle'),
    join(target, 'art/invincibility_potion.bundle'),
  )
  await writeFile(join(target, 'manifest.json'), `${JSON.stringify({
    $schema: '/mod-manifest.schema.json',
    id: manifest.id,
    name: 'Invincibility Potion',
    version: manifest.version,
    runtime: {
      apiVersion: WEB_LUA_DEFINITION_API_VERSION,
      entryScript: 'scripts/main.lua',
    },
    requiredMods: [],
  }, null, 2)}\n`)
  await writeFile(join(target, 'scripts/main.lua'), invincibilityPotionTemplate())
  const checked = await checkPackage(target, false)
  io.log(JSON.stringify({ ...summary(checked), migratedFrom: source }, null, 2))
}

async function generateReference(args, io) {
  const [targetArg, mode] = args
  if (!targetArg) throw new Error(usage())
  const target = resolve(targetArg)
  const schema = {
    $schema: 'https://json-schema.org/draft/2020-12/schema',
    title: 'Solomon Dark Web Lua 1.0 definition inventory',
    apiVersion: WEB_LUA_DEFINITION_API_VERSION,
    contentIdentity: { oneOf: ['key', 'name'] },
    content: Object.fromEntries(WEB_LUA_CONTENT_KINDS.map(kind => [
      kind,
      {
        allowed: ['key', ...WEB_LUA_CONTENT_SCHEMA_FIELDS[kind].allowed],
        required: friendlyRequiredFields(kind),
      },
    ])),
    constructors: {
      effects: EFFECTS,
      events: WEB_LUA_RULE_EVENT_NAMES,
      intents: EFFECTS,
      prefabs: PREFABS,
      rules: RULES,
      schemas: SCHEMAS,
    },
    aliases: {
      art: ART_ALIASES,
      kit: WEB_LUA_CONTENT_KINDS.map(kind => kind.replaceAll('-', '_')),
      rules: RULES,
    },
    artSlots: WEB_LUA_CONTENT_ART_SLOTS,
    diagnostics: WEB_LUA_DEFINITION_ERROR_CODES,
    include: {
      maximumScripts: WEB_LUA_MAX_INCLUDED_SCRIPTS,
      maximumBytes: WEB_LUA_MAX_SCRIPT_BYTES,
    },
    predicateFields: WEB_LUA_PREDICATE_FIELDS,
    scopes: WEB_LUA_SCOPE_KINDS,
  }
  const generated = new Map([
    ['web-lua-definition.schema.json', `${JSON.stringify(schema, null, 2)}\n`],
    ['sd.lua', luaLsStub()],
    ['REFERENCE.md', referenceMarkdown()],
    ['DIAGNOSTICS.md', diagnosticsMarkdown()],
    ['STARTER.lua', starterLua()],
  ])
  if (mode === '--check') {
    for (const [path, expected] of generated) {
      const actual = await readFile(join(target, path), 'utf8')
      if (actual !== expected) throw new Error(`generated Web Lua author file is stale: ${path}`)
    }
    io.log(`Web Lua author reference is current at ${target}`)
    return
  }
  if (mode !== undefined) throw new Error(usage())
  await mkdir(target, { recursive: true })
  for (const [path, value] of generated) await writeFile(join(target, path), value)
  io.log(`Generated Web Lua author reference at ${target}`)
}

async function packageEntries(root) {
  const result = new Map()
  for (const path of await walk(root)) {
    if (path.startsWith('tests/') || path.startsWith('compiled/')) continue
    result.set(path, await readFile(join(root, path)))
  }
  return result
}

async function walk(root, directory = root) {
  const result = []
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (entry.name === '.git' || entry.name === 'node_modules') continue
    const path = join(directory, entry.name)
    if (entry.isDirectory()) result.push(...await walk(root, path))
    else if (entry.isFile()) result.push(relative(root, path).split(sep).join('/'))
  }
  return result.sort()
}

async function requireEmptyDirectory(root) {
  try {
    const info = await stat(root)
    if (!info.isDirectory() || (await readdir(root)).length > 0) {
      throw new Error(`target must be an absent or empty directory: ${root}`)
    }
  } catch (error) {
    if (error?.code !== 'ENOENT') throw error
    await mkdir(root, { recursive: true })
  }
}

function summary(checked) {
  return {
    apiVersion: checked.compiled.apiVersion,
    assets: checked.compiled.assets.length,
    capabilities: checked.compiled.capabilities,
    content: checked.compiled.content.length,
    graphSha256: checked.compiled.graphSha256,
    id: checked.manifest.id,
    reducers: checked.compiled.reducers.length,
    rules: checked.compiled.rules.length,
    version: checked.manifest.version,
  }
}

function canonicalId(value) {
  const id = value.toLowerCase().replace(/[^a-z0-9._-]+/g, '-').replace(/^[-._]+|[-._]+$/g, '')
  if (!id) throw new Error('target directory does not produce a valid mod id')
  return id
}

function displayName(value) {
  return value.split(/[._-]+/).map(word => word ? word[0].toUpperCase() + word.slice(1) : '').join(' ')
}

function starterModTemplate(name) {
  return `-- ${name}: a starter mod. Rename things, save, and run the check command printed by sdmod new.
-- Each sd.* call below tells the game about one thing.

sd.item({
  -- The key is this item's permanent id. Keep it once players have saves.
  key = "starter_item",
  name = ${JSON.stringify(`${name} Token`)},
  description = "A small item to rename and build on.",
  -- A path is enough for art. The game declares the sprite for you.
  icon = "art/icon.png",
})
`
}

function invincibilityPotionTemplate() {
  return `local icon = sd.art.sprite("art/invincibility_potion.png")

return sd.mod({
  api = "1.0.0",
  assets = {invincibility_potion = icon},
  content = {
    sd.kit.status({
      key = "invincible",
      duration = "3m",
      stacking = "refresh",
      modifiers = {incoming_damage = 0, mana_spend = 0},
    }),
    sd.kit.potion({
      key = "invincibility_potion",
      name = "Invincibility Potion",
      description = "Grants invincibility and unlimited mana for 3 minutes.",
      duration = "3m",
      status = sd.ref("status", "invincible"),
      on_use = sd.rules.all({
        sd.effect.resource({target = "user", mana = "full"}),
        sd.effect.status({target = "user", status = sd.ref("status", "invincible")}),
      }),
      loot = {ordinary = 0.5, boss = 1.0},
      art = {icon = sd.art.ref("invincibility_potion")},
    }),
  },
})
`
}

function luaLsStub() {
  const specs = WEB_LUA_CONTENT_KINDS.map(kind => {
    const fields = WEB_LUA_CONTENT_SCHEMA_FIELDS[kind]
    const required = new Set(friendlyRequiredFields(kind))
    const shorthand = WEB_LUA_CONTENT_ART_SLOTS[kind]
      .filter(slot => !fields.allowed.includes(slot))
      .map(slot => `---@field ${slot}? string|table`)
    return [
      `---@class Sd${pascal(kind)}Spec`,
      '---@field key? string',
      ...fields.allowed.map(field => (
        `---@field ${field}${required.has(field) ? '' : '?'} ${luaFieldType(kind, field)}`
      )),
      ...shorthand,
    ].join('\n')
  }).join('\n\n')
  const kits = WEB_LUA_CONTENT_KINDS.map(kind => (
    `---@field ${kind.replaceAll('-', '_')} fun(spec: Sd${pascal(kind)}Spec): table`
  )).join('\n')
  const constructors = (name, values, type) => `---@class Sd${name}\n${values.map(value => (
    `---@field ${value} fun(spec: table): ${type}`
  )).join('\n')}`
  const ruleSignatures = [
    '---@field on fun(event: SdEventName, ...: SdRule): SdRule',
    '---@field all fun(...: SdRule|SdRule[]): SdRule',
    '---@field first fun(...: SdRule|SdRule[]): SdRule',
    '---@field when fun(predicate: boolean|SdPredicate, yes: SdRule|SdRule[], no?: SdRule|SdRule[]): SdRule',
    '---@field after fun(duration: SdDuration, ...: SdRule): SdRule',
    '---@field every fun(interval: SdDuration, node: SdRule|SdRule[], times?: integer|{times: integer}): SdRule',
  ].join('\n')
  const artSignatures = [
    '---@field music fun(path: string, options?: table): table',
    '---@field sheet fun(path_or_spec: string|table, options?: table): table',
    '---@field sound fun(path: string, options?: table): table',
    '---@field sprite fun(path: string, options?: table): table',
    '---@field wearable fun(path: string, options?: table): table',
  ].join('\n')
  return `---@meta

---@alias SdDuration integer|string
---@alias SdEventName ${WEB_LUA_RULE_EVENT_NAMES.map(value => JSON.stringify(value)).join('|')}
---@alias SdScope ${WEB_LUA_SCOPE_KINDS.map(value => JSON.stringify(value)).join('|')}
---@alias SdRule table
---@alias SdIntentValue table
---@alias SdSchemaDefinition table

---@class SdPredicate
---@field event? SdEventName
---@field context? string
---@field equals? boolean|number|string
---@field not_equals? boolean|number|string
---@field above? number
---@field below? number
---@field at_least? number
---@field at_most? number
---@field all? SdPredicate[]
---@field any? SdPredicate[]
---@field none? SdPredicate[]

---@class SdUiBinding
---@field state string

---@class SdUiVisibilityState
---@field state string
---@field equals? boolean|number|string

---@class SdUiVisibility
---@field scenes? ("hub"|"boneyard"|"room")[]
---@field state? SdUiVisibilityState

---@class SdReducerSpec
---@field key string
---@field scope SdScope
---@field schema_version integer
---@field state SdSchemaDefinition
---@field on SdEventName[]
---@field reduce function
---@field migrations? table<integer, function>

${specs}

---@class SdModSpec
---@field api? "1.0.0"
---@field assets? table<string, table>
---@field content? table[]
---@field rules? SdRule[]
---@field systems? table[]

---@class SdArt
---@field boneyard fun(spec: string|table): table
---@field ref fun(key: string): table
---@field scene fun(spec: string|table): table
${artSignatures}

---@class SdKit
${kits}

---@class SdRules
${ruleSignatures}

${constructors('Effect', EFFECTS, 'SdRule')}

${constructors('Intent', EFFECTS, 'SdIntentValue')}

${constructors('Prefab', PREFABS, 'SdRule')}

${constructors('Schema', SCHEMAS, 'SdSchemaDefinition')}

---@class SdAdvanced
---@field reducer fun(spec: SdReducerSpec): table

---@class Sd
---@field advanced SdAdvanced
---@field art SdArt
---@field effect SdEffect
---@field include fun(path: string): any
---@field intent SdIntent
---@field kit SdKit
---@field mod fun(spec: SdModSpec): table
---@field prefab SdPrefab
---@field ref fun(kind: string, key: string, mod_id?: string): table
---@field rules SdRules
---@field schema SdSchema
${kits}
${ruleSignatures}
${artSignatures}

---@type Sd
sd = {}
`
}

function referenceMarkdown() {
  const rows = WEB_LUA_CONTENT_KINDS.map(kind => {
    const value = WEB_LUA_CONTENT_SCHEMA_FIELDS[kind]
    const required = friendlyRequiredFields(kind)
    const shorthand = WEB_LUA_CONTENT_ART_SLOTS[kind].filter(slot => !value.allowed.includes(slot))
    return `| \`${kind}\` | ${required.map(field => `\`${field}\``).join(', ') || 'none'} | ${['key', ...value.allowed].map(field => `\`${field}\``).join(', ')} | ${shorthand.map(field => `\`${field}\``).join(', ') || 'none'} |`
  }).join('\n')
  const list = (root, values) => values.map(value => `- \`${root}.${value}(spec)\``).join('\n')
  return `# Web Lua 1.0 generated reference

API: \`${WEB_LUA_DEFINITION_API_VERSION}\`

## Quick start

A mod is a script that creates things. Each \`sd.*\` call tells the game about
one thing, and the game collects everything the script created when it ends.

From \`frontend/\`, scaffold and check a mod with:

\`\`\`sh
npm run sdmod -- new path/to/my-mod
npm run sdmod -- check path/to/my-mod
\`\`\`

\`\`\`lua
local tough = sd.status({key = "tough", duration = "5s", modifiers = {incoming_damage = {multiply = 0.8}}})

sd.potion({
  name = "Tough Tonic",
  status = tough,
  icon = "art/tonic.png",
})

sd.on("wave.completed", sd.when({context = "wave", at_least = 5}, sd.effect.resource({target = "user", mana = 5})))
\`\`\`

- \`sd.item\`, \`sd.potion\`, \`sd.status\`, \`sd.enemy\`, and every other content
  kind is a short name for \`sd.kit.<kind>\`. \`sd.on\`, \`sd.all\`, \`sd.first\`,
  \`sd.when\`, \`sd.after\`, and \`sd.every\` are short names for \`sd.rules.*\`.
  \`sd.sprite\`, \`sd.sheet\`, \`sd.wearable\`, \`sd.sound\`, and \`sd.music\` are
  short names for \`sd.art.*\`.
- A content \`key\` is optional when the \`name\` can become one: "Tough Tonic"
  becomes \`tough_tonic\`. Write the key yourself once players have saves, because
  the key is the permanent id of that content.
- Any field that expects content accepts the created value, or its key as a
  string: \`status = tough\` and \`status = "tough"\` mean the same thing.
- Art fields accept a path. \`icon = "art/tonic.png"\` declares the sprite and
  references it. Sounds, music, wearables, and boneyard layouts work the same way.
  Enemy atlases need \`sd.sheet\` so the game knows the frame grid.
- A potion with a \`status\` and no \`on_use\` applies that status to the user, and
  takes its \`duration\` from the status.
- \`sd.on(event, ...)\` attaches rules on its own. Effects created outside a rule
  or content field are an error, so nothing is silently dropped.
- \`sd.mod({...})\` is still available for explicit ordering and for \`systems\`
  (advanced reducers). It may be called once, and everything created outside its
  lists is still collected.
- Errors name the file and line that created the value, and suggest close names.

## Splitting a mod across files

\`sd.include("scripts/items.lua")\` runs another script from the package once
and returns whatever it returns, so a large mod can keep items, enemies, and
scenes in separate files. Included scripts see the same \`sd\` and the same
strict globals. Packing folds every \`scripts/*.lua\` file into the entry script,
verifies the folded script compiles to the identical graph, and keeps the
sources in the package. At most ${WEB_LUA_MAX_INCLUDED_SCRIPTS} extra scripts
and ${WEB_LUA_MAX_SCRIPT_BYTES} bytes of Lua in total are allowed.

## Art

- \`sd.art.sprite(path, options)\` declares one PNG sprite.
- \`sd.art.sheet(spec)\` declares an explicit PNG frame grid, with optional \`headings\`.
- \`sd.art.wearable(path, options)\` declares a 170 px actor sheet for an existing hat, robe, or staff slot.
- \`sd.art.sound(path, options)\` and \`sd.art.music(path, options)\` declare audio.
- \`sd.art.scene(spec)\` and \`sd.art.boneyard(spec)\` declare document assets.
- \`sd.art.ref(key)\` references a named asset from content.
- Every art constructor accepts \`key = "name"\` in its options. Without a key
  the file name becomes the key, so \`art/tonic.png\` is \`tonic\`.

## Content

| Kind | Required fields | Allowed fields | Art shorthand fields |
| --- | --- | --- | --- |
${rows}

A required \`name\` may stand in for the \`key\`. Art shorthand fields take a
path or an \`sd.art\` value and move into \`art\`.

## Rules

- \`sd.on(event, ...)\`: run the rules when the event fires. Several rules run in order.
- \`sd.all(...)\`: run every rule.
- \`sd.first(...)\`: run the first rule that produces an effect.
- \`sd.when(predicate, yes, no)\`: choose a branch.
- \`sd.after(duration, ...)\`: run later.
- \`sd.every(interval, rule, times)\`: repeat a bounded number of times.

Lists of effects are accepted wherever one rule is expected, so
\`on_use = {a, b}\` means \`on_use = sd.all(a, b)\`.

## Predicates

\`sd.when\` takes \`true\`, \`false\`, or a table with exactly one subject:

- \`{event = "wave.completed"}\` is true while that event is being handled.
- \`{context = "wave"}\` is true when the context value is set and truthy.
- \`{context = "wave", equals = 5}\` compares with one of ${WEB_LUA_PREDICATE_FIELDS.filter(field => !['all', 'any', 'context', 'event', 'none'].includes(field)).map(field => `\`${field}\``).join(', ')}.
  The numeric comparisons are false unless both sides are numbers.
- \`{all = {...}}\`, \`{any = {...}}\`, and \`{none = {...}}\` combine predicates.

## Events

${WEB_LUA_RULE_EVENT_NAMES.map(value => `- \`${value}\``).join('\n')}

Unknown event names fail admission. UI rules receive the declared UI action in
the \`action\` context field and the framework action family in \`action_kind\`.

## Effects

${list('sd.effect', EFFECTS)}

\`sd.effect.grant\` and \`sd.effect.status\` accept content keys as strings.
\`sd.effect.present\` accepts a sound path.
\`sd.effect.spawn\` accepts a local enemy key or a stock name such as
\`stock.skeleton\`; both forms are validated during \`check\`.

## Prefabs

${list('sd.prefab', PREFABS)}

## Advanced reducers

\`sd.advanced.reducer(spec)\` declares a scoped reducer. Versions above 1 require a pure migration for every prior version in \`migrations\`. Reducers are collected automatically; list them under \`systems\` only when using \`sd.mod\` to make their order explicit.

## UI state shapes

- \`bindings = {label = {state = "state.key"}}\`
- \`visible = {scenes = {"hub", "boneyard", "room"}}\`
- \`visible = {state = {state = "state.key", equals = value}}\`

## Schemas and intents

${list('sd.schema', SCHEMAS)}

${list('sd.intent', EFFECTS)}

## Scopes

${WEB_LUA_SCOPE_KINDS.map(value => `- \`${value}\``).join('\n')}

## Sandbox

Definition scripts run once, in a small Lua VM with a 250 ms budget. \`require\`,
\`dofile\`, \`load\`, \`io\`, \`os\`, \`debug\`, and \`coroutine\` are not available;
use \`sd.include\` to split files. Reading an unknown \`sd\` name or an undefined
global is an error with a suggestion, and \`sd\` names are read-only.
`
}

function diagnosticsMarkdown() {
  const descriptions = {
    E_API_VERSION: 'Use api = "1.0.0" or leave api out.',
    E_ASSET: 'Fix the declared path, type, bytes, dimensions, or audio header.',
    E_BUDGET: 'Reduce definitions, nodes, depth, files, state, or definition-time loops rather than raising a limit.',
    E_CONTENT_KEY: 'Use a stable lowercase content key, or a name that can become one.',
    E_CYCLE: 'Remove the reported reference cycle.',
    E_DUPLICATE: 'Rename the duplicate key or mount owner.',
    E_GRAPH: 'Attach every effect to a rule or content field, call sd.mod at most once, and repack from the current source when a compiled graph does not match.',
    E_MOUNT_CONFLICT: 'Give exclusive UI or shop mounts one owner.',
    E_REFERENCE: 'Create the target before packing and use the correct content kind; the message suggests close names.',
    E_SCHEMA: 'Make the value match its declared bounded schema.',
    E_SCRIPT: 'Fix the Lua error at the reported file and line; common slips come with a hint.',
    E_UNKNOWN_FIELD: 'Remove the field or use one of the allowed fields the message lists.',
  }
  for (const code of WEB_LUA_DEFINITION_ERROR_CODES) {
    if (!(code in descriptions)) throw new Error(`diagnostics description missing for ${code}`)
  }
  return `# Web Lua diagnostics

From \`frontend/\`, run \`npm run sdmod -- check <directory>\`. Definition issues include a stable code, a graph path, and the script file and line that created the value. Prepared package errors name the content or asset field that failed admission.

${WEB_LUA_DEFINITION_ERROR_CODES.map(code => `- \`${code}\`: ${descriptions[code]}`).join('\n')}
`
}

function starterLua() {
  return `-- Web Lua starter. Each sd.* call tells the game about one thing.
-- Save this as scripts/main.lua, put a PNG at art/icon.png, then from frontend run:
-- npm run sdmod -- check path/to/my-mod

-- A status is a temporary effect on a character.
local tough = sd.status({
  key = "example_tough",
  duration = "5s",
  modifiers = {incoming_damage = {multiply = 0.8}},
})

-- An item can be carried, sold, and granted. The icon path declares the art.
sd.item({
  key = "example_item",
  name = "Example Item",
  icon = "art/icon.png",
})

-- A potion with a status applies that status when it is used.
sd.potion({
  key = "example_potion",
  name = "Example Potion",
  status = tough,
  icon = "art/icon.png",
})

-- Rules react to game events. This one hands the potion out when a run starts.
sd.on("run.started", sd.effect.grant({target = "user", item = "example_potion", quantity = 1}))
`
}

function pascal(value) {
  return value.split('-').map(part => part[0].toUpperCase() + part.slice(1)).join('')
}

function friendlyRequiredFields(kind) {
  return WEB_LUA_CONTENT_SCHEMA_FIELDS[kind].required.filter(field => (
    kind !== 'potion' || (field !== 'duration' && field !== 'on_use')
  ))
}

function luaFieldType(kind, field) {
  if (['accessible_name', 'description', 'name', 'source'].includes(field)) return 'string'
  if (kind === 'boast' && ['instruction', 'response', 'statement'].includes(field)) return 'string'
  if (kind === 'boast' && field === 'fail_on') {
    return '("magical-equipment"|"mana-underflow"|"potion-use"|"secondary-cast")[]'
  }
  if (kind === 'boast' && field === 'random_skill_choices') return 'boolean'
  if (kind === 'boast' && field === 'score_multiplier') return 'number'
  if (kind === 'boast' && ['stock_icon', 'success_wave'].includes(field)) return 'integer'
  if (['cooldown', 'duration', 'restock'].includes(field)) return 'SdDuration'
  if (['maximum_rank', 'rolls'].includes(field)) return 'integer'
  if (field === 'mana') return 'number'
  if (field === 'slot') return '"primary"|"secondary"'
  if (field === 'stacking') return '"ignore"|"refresh"|"replace"|"stack"'
  if (field === 'stock_scope') return '"party"|"player"|"session"'
  if (field === 'rng_domain') return 'string'
  if (field === 'scene' && kind === 'scene-extension') return '"stock.boneyard"'
  if (field === 'mount' && kind === 'ui') {
    return '"hud.bottom_left"|"hud.bottom_right"|"hud.overlay"|"hud.top_left"|"hud.top_right"'
  }
  if (field === 'bindings' && kind === 'ui') return 'table<string, SdUiBinding>'
  if (field === 'visible' && kind === 'ui') return 'SdUiVisibility'
  if (field === 'behavior' || field === 'effect' || field === 'on_use' || field === 'use' || field === 'view') {
    return 'SdRule|SdRule[]'
  }
  if (field === 'status' || field === 'parent') return 'string|table'
  if (['grants', 'prerequisites', 'rooms', 'roster'].includes(field)) return '(string|table)[]'
  if (field === 'features' || field === 'triggers') return 'SdRule[]'
  if (['actions', 'applies_to'].includes(field)) return 'string[]'
  if (['entries', 'prerequisites', 'ranks', 'rooms', 'roster', 'services', 'stock', 'waves'].includes(field)) {
    return 'table[]'
  }
  return 'table'
}

function usage() {
  return [
    'usage:',
    '  sdmod new <directory>',
    '  sdmod assets fetch [source-id ...]',
    '  sdmod check <mod-directory>',
    '  sdmod test <mod-directory>',
    '  sdmod pack <mod-directory> <output.zip>',
    '  sdmod migrate <0.2-mod-directory> <1.0-directory>',
    '  sdmod generate <output-directory> [--check]',
    '  sdmod render-sprite <recipe.json>',
  ].join('\n')
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  runSdmod(process.argv.slice(2)).catch((error) => {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`)
    process.exitCode = 1
  })
}
