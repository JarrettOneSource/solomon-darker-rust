// Run the pinned immutable graph compiler, independently of VM admission.
import { loadKernel, writeFixture } from './oracle.mjs'

const { compileWebLuaDefinition } = await loadKernel('modding/definition/web-lua-definition-compiler.ts')
const { DEFAULT_WEB_LUA_DEFINITION_LIMITS } = await loadKernel('modding/definition/web-lua-definition-types.ts')
const identity = { id: 'compiler.test', name: 'Compiler Test', version: '1.0.0' }
const source = { column: 0, file: 'scripts/main.lua', line: 1 }
const asset = (assetKind, key, fields) => ({ assetKind, fields, key, kind: 'asset-definition', source })
const content = (contentKind, key, fields) => ({ contentKind, fields, key, kind: 'content-definition', source })
const reference = (targetKind, key, modId = null) => ({ key, kind: 'content-reference', modId, targetKind })
const art = key => ({ key, kind: 'asset-reference' })
const rule = (operation, fields) => ({ fields, kind: 'rule-definition', operation, source })
const schema = (schemaKind, fields) => ({ fields, kind: 'schema-definition', schemaKind, source })
const effect = rule('effect.damage', { amount: 5, target: 'enemy' })
const base = {
  api: '1.0.0',
  assets: [asset('sprite', 'icon', { path: 'art/icon.png' }), asset('sound', 'sound', { path: 'audio/cast.wav' }),
    asset('sheet', 'sheet', { image: 'art/sheet.png', frame: { width: 170, height: 170 } }),
    asset('music', 'music', { path: 'audio/music.ogg', loop: true }),
    asset('scene', 'scene', { path: 'scenes/a.json' }), asset('boneyard', 'yard', { path: 'levels/a.boneyard' })],
  content: [
    content('status', 'ward', { duration: '2m', modifiers: { incoming_damage: { multiply: 0.5 } } }),
    content('potion', 'tonic', { name: 'Tonic', duration: '2m', art: { icon: art('icon') },
      on_use: rule('effect.status', { status: reference('status', 'ward'), target: 'user' }), status: reference('status', 'ward') }),
    content('item', 'item', { name: 'Item', art: { icon: art('icon') }, use: rule('effect.resource', { gold: 1, target: 'user' }) }),
    content('affix', 'affix', { name: 'Affix', modifiers: { mana_spend: { set: 0 } } }),
    content('affix-pool', 'pool', { entries: [{ affix: reference('affix', 'affix'), weight: 1 }] }),
    content('powerup', 'orb', { name: 'Orb', effect: rule('effect.grant', { item: reference('potion', 'tonic'), target: 'user' }) }),
    content('skill', 'skill', { name: 'Skill', ranks: [{}], grants: [reference('spell', 'spell')] }),
    content('spell', 'spell', { name: 'Spell', slot: 'primary', behavior: rule('prefab.projectile', { duration: '1s', radius: 4, speed: 8, effects: [effect] }) }),
    content('enemy', 'enemy', { name: 'Enemy', stats: { health: 100 } }),
    content('boneyard', 'yard', { name: 'Yard', source: 'levels/a.boneyard', roster: [reference('enemy', 'enemy')] }),
    content('boast', 'boast', { name: 'Boast', instruction: 'Survive', response: 'Yes', statement: 'I will', stock_icon: 0 }),
    content('room', 'room', { geometry: { kind: 'inline', width: 640, height: 480 } }),
    content('scene', 'scene', { rooms: [reference('room', 'room')] }),
    content('scene-extension', 'extension', { scene: 'stock.boneyard', features: [rule('effect.state', { key: 'ready', value: true })] }),
    content('shop', 'shop', { name: 'Shop', stock: [{ item: reference('item', 'item') }] }),
    content('ui', 'ui', { mount: 'hud.top_right', view: rule('prefab.minimap', { range: 300, size: { width: 150, height: 100 } }) }),
  ],
  reducers: [{ key: 'progress', on: ['run.started'], schemaVersion: 1, scope: 'party-run', source,
    state: schema('object', { fields: { count: schema('integer', { default: 0, min: 0, max: 9 }) } }) }],
  rules: [rule('rules.on', { event: 'wave.completed', node: rule('effect.present', { sound: art('sound') }) })],
}
const cases = []
function record(name, definition, options = {}) {
  // The portable compiler boundary owns values; record the same detached graph
  // that the Rust definition runtime hands to its immutable compiler.
  definition = structuredClone(definition)
  let output = null
  let issues = null
  let error = null
  try { output = compileWebLuaDefinition(identity, definition, options) }
  catch (caught) { issues = caught.issues ?? null; error = caught.message }
  cases.push({ name, definition, options, output, issues, error })
}
function mutate(name, change, options = {}) {
  const definition = structuredClone(base)
  change(definition)
  record(name, definition, options)
}
function fields(key, name, change) {
  mutate(name, definition => change(definition.content.find(entry => entry.key === key).fields))
}
record('all-families-assets-rule-and-reducer', base)
record('empty-definition', { api: '1.0.0', assets: [], content: [], reducers: [], rules: [] })
mutate('declaration-order-independent-digest', definition => { definition.content.reverse(); definition.assets.reverse() })
mutate('external-and-stock-reference', definition => {
  definition.content.find(entry => entry.key === 'skill').fields.grants = [reference('spell', 'external', 'dependency'), reference('ui', 'map', 'stock')]
}, { dependencies: [{ id: 'dependency', content: [{ contentId: '42', contentKind: 'spell', key: 'external' }] }],
  stockContent: [{ contentId: '7', contentKind: 'ui', key: 'map' }] })
mutate('wrong-api', definition => { definition.api = '2.0.0' })
mutate('duplicate-asset', definition => { definition.assets.push(structuredClone(definition.assets[0])) })
mutate('duplicate-cross-family-key', definition => { definition.content.push(content('status', 'item', {})) })
mutate('invalid-key', definition => { definition.content[0].key = '_invalid' })
mutate('duplicate-dependency', () => {}, { dependencies: [{ id: 'same', content: [] }, { id: 'same', content: [] }] })
fields('item', 'unknown-content-field-suggestion', value => { value.descriptino = 'Typo' })
fields('item', 'missing-name', value => { delete value.name })
mutate('unknown-asset-field', definition => { definition.assets[0].fields.pth = 'typo' })
mutate('missing-asset-path', definition => { delete definition.assets[0].fields.path })
fields('ward', 'invalid-duration', value => { value.duration = '01s' })
fields('ward', 'invalid-stacking', value => { value.stacking = 'merge' })
fields('tonic', 'invalid-probabilities', value => { value.loot = { ordinary: -1, boss: 2 } })
fields('boast', 'invalid-boast-fields', value => {
  delete value.stock_icon; value.instruction = ''; value.fail_on = ['potion-use', 'potion-use']
  value.random_skill_choices = 'yes'; value.success_wave = 0; value.score_multiplier = 11
})
fields('boast', 'boast-art-conflict', value => { value.art = { icon: art('icon'), portrait: art('icon') } })
fields('boast', 'boast-icon-outside-range', value => { value.stock_icon = 8 })
fields('tonic', 'unknown-local-reference', value => { value.status = reference('status', 'wrad') })
fields('tonic', 'wrong-family-reference', value => { value.status = reference('status', 'item') })
fields('skill', 'missing-dependency-reference', value => { value.grants = [reference('spell', 'external', 'absent')] })
fields('skill', 'unknown-stock-reference', value => { value.grants = [reference('spell', 'absent', 'stock')] })
fields('item', 'unknown-asset-reference', value => { value.art.icon = art('icno') })
mutate('local-content-cycle', definition => {
  definition.content.find(entry => entry.key === 'ward').fields.next = reference('status', 'other')
  definition.content.push(content('status', 'other', { next: reference('status', 'ward') }))
})
mutate('exclusive-mount-conflict', definition => {
  definition.content.find(entry => entry.key === 'shop').fields.mount = { scene: 'hub.courtyard', anchor: 'east' }
  definition.content.push(content('shop', 'second', { name: 'Second', stock: [], mount: { scene: 'hub.courtyard', anchor: 'east' } }))
})
for (const [name, operation, values] of [
  ['when-subject', 'rules.when', { predicate: { contest: 'wave' }, yes: effect }],
  ['when-comparisons', 'rules.when', { predicate: { event: 'run.started', above: 'x', equals: {} }, yes: effect }],
  ['when-groups', 'rules.when', { predicate: { all: [] }, yes: effect }],
  ['when-numeric', 'rules.when', { predicate: { context: 'wave', below: 'x' }, yes: effect }],
  ['on-unknown-event', 'rules.on', { event: 'run.start', node: effect }],
  ['on-non-text', 'rules.on', { event: 3, node: false }],
  ['all-empty', 'rules.all', { nodes: [] }],
  ['first-invalid', 'rules.first', { nodes: [true] }],
  ['after-missing', 'rules.after', {}],
  ['every-unbounded', 'rules.every', { interval: '1s', node: effect, times: 1025 }],
  ['prefab-numeric', 'prefab.area', { duration: 'x', every: -1, radius: 0, effects: [true] }],
  ['minimap-layout', 'prefab.minimap', { layers: ['hidden'], range: 0, size: { width: -1, height: 1, extra: 2 } }],
  ['portal-fields', 'prefab.portal', { destination: reference('scene', 'scene'), selector: { object_kind: 'other', extra: true }, policy: 'solo', prompt: '' }],
  ['resource-empty', 'effect.resource', { target: 'user' }],
  ['spawn-empty', 'effect.spawn', {}],
  ['spawn-invalid', 'effect.spawn', { content: reference('item', 'item'), enemy: true }],
  ['state-ambiguous', 'effect.state', { key: 'ready', value: null, clear: true }],
  ['damage-zero', 'effect.damage', { target: 'enemy', amount: 0 }],
  ['status-invalid', 'effect.status', { target: 'user', status: 'ward' }],
  ['grant-invalid', 'effect.grant', { target: 'user', item: reference('spell', 'spell') }],
  ['present-asset-kind', 'effect.present', { sound: art('icon') }],
  ['present-invalid', 'effect.present', { sound: 'sound' }],
  ['unsupported-operation', 'effect.heal', { amount: 1 }],
]) {
  mutate(name, definition => { definition.rules = [rule(operation, values)] })
}
mutate('schema-array-item', definition => { definition.reducers[0].state = schema('array', { item: true, extra: 1 }) })
mutate('schema-object-fields', definition => { definition.reducers[0].state = schema('object', { fields: [] }) })
mutate('schema-empty-enum', definition => { definition.reducers[0].state = schema('enum', { values: [] }) })
mutate('schema-unsupported', definition => { definition.reducers[0].state = schema('unknown', {}) })
mutate('nested-intent-validation', definition => {
  definition.reducers[0].state.fields.fields.command = schema('object', { fields: {
    embedded: { fields: { amount: 0, target: 'enemy' }, kind: 'intent-definition', intentKind: 'damage', source },
  } })
})
for (const [name, limits] of [
  ['content-count-budget', { maximumContent: 1 }],
  ['node-budget', { maximumNodes: 20 }],
  ['depth-budget', { maximumDepth: 3 }],
  ['string-budget', { maximumStringBytes: 4 }],
]) {
  record(name, base, { limits: { ...DEFAULT_WEB_LUA_DEFINITION_LIMITS, ...limits } })
}
writeFixture('mod-compiler-goldens.json', 'mod-compiler-goldens.mjs', 'modding/definition/web-lua-definition-{compiler,schemas}.ts', { identity, cases })
