import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, writeFixture } from './oracle.mjs'

const { compileModContentCatalog } = await loadKernel('modding/content/mod-content-catalog.ts')
const { PreparedModAssetCatalog } = await loadKernel('modding/assets/mod-asset-compiler.ts')
const { stableWebLuaContentId } = await loadKernel('modding/definition/web-lua-content-identity.ts')
const { ModStatusEngine } = await loadKernel('modding/content/mod-status-engine.ts')
const { ModAffixEngine } = await loadKernel('modding/content/mod-affix-engine.ts')
const { ModSpellEngine } = await loadKernel('modding/content/mod-spell-engine.ts')
const source = JSON.parse(readFileSync(resolve(fixturesDir, 'mod-content-goldens.json'))).cases[0]
const mods = structuredClone(source.mods)
const id = key => stableWebLuaContentId(mods[0].identity.id, key)
for (const [key, stacking, modifiers] of [
  ['ignore', 'ignore', { incoming_damage: { set: 7 } }],
  ['refresh', 'refresh', { incoming_damage: { add: -3 }, mana_spend: 0.5 }],
  ['replace', 'replace', { incoming_damage: 2 }],
]) mods[0].content.push({ contentId: id(key), contentKind: 'status', key, fields: { duration: '20ms', stacking, modifiers } })
mods[0].content.push(
 { contentId:id('strong'),contentKind:'affix',key:'strong',fields:{name:'Strong',applies_to:['robe'],modifiers:{incoming_damage:{multiply:0.75}}} },
 { contentId:id('swift'),contentKind:'affix',key:'swift',fields:{name:'Swift',applies_to:['wand'],modifiers:{mana_spend:{multiply:0.75}}} },
)
const pool = mods[0].content.find(row => row.key === 'pool')
pool.fields.applies_to = ['robe','wand']
pool.fields.entries.push(...['strong','swift'].map((key,index) => ({affix:{kind:'resolved-content-reference',contentId:id(key),key,modId:mods[0].identity.id,targetKind:'affix'},weight:index+1})))
const catalog = compileModContentCatalog(mods, new PreparedModAssetCatalog(source.assets))
const statuses = new ModStatusEngine(catalog, 100)
const spells = new ModSpellEngine(catalog, 100)
const affixes = new ModAffixEngine(catalog)
const cases = []
function record(input) {
  let output = null, error = null
  try {
    switch (input.kind) {
      case 'apply': output = statuses.apply(input.contentId, input.targetId, input.tick, input.durationMs); break
      case 'damage': output = statuses.filterDamage(input.targetId, input.amount, input.tick); break
      case 'mana': output = statuses.filterMana(input.targetId, input.delta, input.tick); break
      case 'status-tick': output = statuses.tick(input.tick); break
      case 'status-restore': statuses.restore(input.checkpoint); break
      case 'admit': { const result = spells.admit(input.playerId, input.contentId, input.tick, input.currentMana, input.manaCost); output = { manaCost: result.manaCost, readyTick: result.readyTick, contentId: result.spell.contentId }; break }
      case 'spell-restore': spells.restore(input.checkpoint); break
      case 'roll': output = affixes.roll(input.poolId, input.equipmentType, input.seed); break
    }
  } catch (caught) { error = caught.message }
  cases.push({ input, output, error, status: statuses.checkpoint(), spells: spells.checkpoint() })
}
record({ kind: 'apply', contentId: id('ward'), targetId: 'player:a', tick: 10 })
record({ kind: 'damage', targetId: 'player:a', amount: 40, tick: 11 })
for (let i = 0; i < 8; i += 1) record({ kind: 'apply', contentId: id('ward'), targetId: 'player:a', tick: 11 })
record({ kind: 'mana', targetId: 'player:a', delta: -10, tick: 12 })
record({ kind: 'mana', targetId: 'player:a', delta: 3, tick: 12 })
record({ kind: 'damage', targetId: 'player:a', amount: 40, tick: 136 })
record({ kind: 'status-tick', tick: 136 })
for (const key of ['ignore', 'refresh', 'replace']) {
 record({ kind: 'apply', contentId: id(key), targetId: 'player:a', tick: 140 })
 record({ kind: 'apply', contentId: id(key), targetId: 'player:a', tick: 141 })
 record({ kind: 'damage', targetId: 'player:a', amount: 30, tick: 141 })
}
record({ kind: 'apply', contentId: id('ignore'), targetId: 'player:a', tick: 142, durationMs: 0 })
record({ kind: 'status-tick', tick: 143 })
record({ kind: 'apply', contentId: 'missing', targetId: 'player:a', tick: 144 })
record({ kind: 'apply', contentId: id('ward'), targetId: '', tick: 144 })
record({ kind: 'damage', targetId: 'player:a', amount: -1, tick: 144 })
record({ kind: 'status-restore', checkpoint: { ...statuses.checkpoint(), nextInstanceId: 0 } })
record({ kind: 'status-restore', checkpoint: { instances: [], nextInstanceId: 1, revision: 0 } })
record({ kind: 'admit', playerId: 'player:a', contentId: id('bolt'), tick: 1, currentMana: 10 })
record({ kind: 'admit', playerId: 'player:a', contentId: id('bolt'), tick: 1, currentMana: 10 })
record({ kind: 'admit', playerId: 'player:a', contentId: id('bolt'), tick: 2, currentMana: 9 })
record({ kind: 'admit', playerId: 'player:a', contentId: id('bolt'), tick: 2, currentMana: 9, manaCost: 3 })
record({ kind: 'admit', playerId: 'player:b', contentId: id('bolt'), tick: 2, currentMana: 100 })
record({ kind: 'admit', playerId: '', contentId: id('bolt'), tick: 3, currentMana: 100 })
record({ kind: 'admit', playerId: 'player:a', contentId: 'missing', tick: 3, currentMana: 100 })
record({ kind: 'spell-restore', checkpoint: { cooldowns: [{ playerId: 'player:a', contentId: 'missing', readyTick: 0 }], revision: 1 } })
record({ kind: 'spell-restore', checkpoint: { cooldowns: [], revision: 0 } })
for (const seed of [0, 1, 12345, 4294967296, 9007199254740991]) {
 for (const equipmentType of ['robe', 'wand']) record({ kind: 'roll', poolId: id('pool'), equipmentType, seed })
}
record({ kind: 'roll', poolId: 'missing', equipmentType: 'robe', seed: 1 })
writeFixture('mod-engines-goldens.json', 'mod-engines-goldens.mjs', 'modding/content/mod-status-engine.ts,mod-affix-engine.ts,mod-spell-engine.ts', { catalog: catalog.all(), cases })
