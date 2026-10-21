// Friendly declaration lowering runs through the pinned creation-registry owner.
import { loadKernel, writeFixture } from './oracle.mjs'

const { assembleWebLuaDefinition } = await loadKernel('modding/definition/web-lua-definition-assembly.ts')
const { WEB_LUA_STOCK_ENEMIES } = await loadKernel('host/lua/web-lua-contract.ts')
const identity = { id: 'assembly.test', name: 'Assembly Test', version: '1.0.0' }
const stockEnemies = new Map()
for (const enemy of WEB_LUA_STOCK_ENEMIES) {
  stockEnemies.set(`stock.${enemy.key}`, enemy.token)
  stockEnemies.set(enemy.key, enemy.token)
  stockEnemies.set(enemy.token, enemy.token)
}
const cases = []
function record(name, create) {
  let nextNode = 1
  const input = { assets: [], content: [], identity, rules: [] }
  const source = () => ({ column: 0, file: 'scripts/main.lua', line: nextNode, node: nextNode++ })
  const asset = (assetKind, fields, key = '') => {
    const definition = { assetKind, fields, key, kind: 'asset-definition', source: source() }
    input.assets.push(definition)
    return definition
  }
  const content = (contentKind, key, fields) => {
    const definition = { contentKind, fields, key, kind: 'content-definition', source: source() }
    input.content.push(definition)
    return definition
  }
  const rule = (operation, fields) => {
    const definition = { fields, kind: 'rule-definition', operation, source: source() }
    input.rules.push(definition)
    return definition
  }
  const explicit = create({ asset, content, rule }) ?? null
  const issues = []
  const output = assembleWebLuaDefinition({
    assets: new Map(input.assets.map(value => [value.source.node, value])),
    content: new Map(input.content.map(value => [value.source.node, value])),
    identity, issues,
    rules: new Map(input.rules.map(value => [value.source.node, value])),
    stockEnemies,
  }, explicit)
  cases.push({ name, input, explicit, output, issues })
}
record('nested-assets-references-list-and-potion-inference', ({ asset, content, rule }) => {
  const icon = asset('sprite', { path: 'art/tonic.png' })
  const ward = content('status', 'ward', { duration: '2m' })
  content('potion', 'tonic', { name: 'Tonic', icon, status: ward })
  const first = rule('effect.resource', { health: 1, target: 'user' })
  const second = rule('effect.state', { key: 'used', value: true })
  content('item', 'token', { name: 'Token', icon: 'art/token.png', use: [first, second],
    worn: 'art/worn.png', worn_trim: 'art/worn-trim.png' })
})
record('asset-shape-deduplication-and-name-collisions', ({ asset }) => {
  asset('sprite', { path: 'art/Icon.png', frame: { width: 20, height: 30 } })
  asset('sprite', { frame: { height: 30, width: 20 }, path: 'art/Icon.png' })
  asset('sound', { path: 'audio/icon.wav' })
  asset('sound', { path: 'audio/sub/icon.wav' })
  asset('sound', { path: 'audio/third/icon.ogg' })
  asset('music', { path: 'audio/icon.ogg' })
})
record('explicit-order-and-implicit-retention', ({ asset, content, rule }) => {
  const first = asset('sprite', { path: 'art/first.png' })
  const second = asset('sprite', { path: 'art/second.png' })
  const a = content('status', 'first', {})
  const b = content('status', 'second', {})
  const effect = rule('effect.state', { key: 'explicit', value: true })
  return { assets: [{ key: 'renamed', node: second.source.node }, { key: '', node: first.source.node }],
    content: [b.source.node, a.source.node, b.source.node], rules: [effect.source.node, effect.source.node], systems: [] }
})
record('explicit-different-assets-same-key', ({ asset }) => {
  const a = asset('sprite', { path: 'art/a.png' }, 'same')
  const b = asset('sprite', { path: 'art/b.png' }, 'same')
  return { assets: [{ key: 'same', node: a.source.node }, { key: 'same', node: b.source.node }], content: [], rules: [], systems: [] }
})
record('all-bare-reference-slots', ({ content, rule }) => {
  content('enemy', 'mote', { name: 'Mote' })
  content('status', 'ward', {})
  content('item', 'item', { name: 'Item' })
  content('potion', 'potion', { name: 'Potion', status: 'ward' })
  content('affix', 'affix', {})
  content('affix-pool', 'pool', { entries: [{ affix: 'affix', weight: 1 }] })
  content('spell', 'spell', {})
  content('ui', 'map', {})
  content('skill', 'parent', { ranks: [] })
  content('skill', 'skill', { parent: 'parent', prerequisites: ['parent'], grants: ['map', 'spell'],
    ranks: [{ grant: 'spell', grants: ['map'] }] })
  content('room', 'room', {})
  content('scene', 'scene', { rooms: ['room'] })
  content('shop', 'shop', { stock: [{ item: 'item' }, { item: 'potion' }], services: [{ pool: 'pool' }] })
  content('boneyard', 'yard', { source: 'levels/yard.boneyard', roster: ['mote', 'stock.zombie', 'SKELETON'],
    waves: [{ roster: ['mote', 'skeleton_archer'] }] })
  const effects = [rule('effect.grant', { item: 'potion', target: 'user' }),
    rule('effect.status', { status: 'ward', target: 'user' }),
    rule('effect.spawn', { enemy: 'stock.zombie' }), rule('effect.spawn', { enemy: 'mote' }),
    rule('effect.spawn', { content: 'powerup' }), rule('effect.spawn', { token: 'skeleton' }),
    rule('effect.present', { sound: 'audio/cast.wav' }),
    rule('prefab.portal', { destination: 'scene', selector: { object_kind: 'monument' } })]
  rule('rules.on', { event: 'run.started', node: effects })
})
record('orphan-and-implicit-event-root', ({ rule }) => {
  rule('effect.state', { key: 'unattached', value: true })
  const effect = rule('effect.state', { key: 'ready', value: true })
  rule('rules.on', { event: 'run.started', node: effect })
})
record('shorthand-art-conflict-and-atlas-rejection', ({ content }) => {
  content('enemy', 'enemy', { name: 'Enemy', art: { sound: 'audio/first.wav' }, sound: 'audio/second.wav', atlas: 'art/enemy.png' })
  content('item', 'item', { name: 'Item', art: false, icon: 'art/item.png' })
})
record('forged-asset-and-content-token', ({ content }) => {
  content('item', 'item', { name: 'Item', icon: { kind: 'asset-definition', key: 'missing', source: { node: 999 } },
    use: { kind: 'content-definition', key: 'bad', contentKind: 'unknown' } })
})
record('unknown-stock-enemies', ({ content, rule }) => {
  content('boneyard', 'yard', { roster: ['stock.zombei', 'UNKNOWN'] })
  const enemy = rule('effect.spawn', { enemy: 'stock.zombei' })
  const token = rule('effect.spawn', { token: 'zombei' })
  rule('rules.on', { event: 'run.started', node: [enemy, token] })
})
record('art-path-kinds-and-derived-unicode-key', ({ content }) => {
  content('scene', 'scene', { layout: 'scenes/İnside.json', music: 'audio/music.ogg', ambience: 'audio/ambient.ogg', loop: 'audio/loop.ogg' })
  content('powerup', 'orb', { world: 'art/---.png', sound: 'audio/pickup.wav' })
})
record('rule-without-source-and-list-wrap', ({ content }) => {
  content('item', 'item', { name: 'Item', use: [
    { kind: 'rule-definition', operation: 'effect.state', fields: { key: 'a', value: true } },
    { kind: 'rule-definition', operation: 'effect.state', fields: { key: 'b', value: true } },
  ] })
})
writeFixture('mod-assembly-goldens.json', 'mod-assembly-goldens.mjs', 'modding/definition/web-lua-definition-assembly.ts', { cases })
