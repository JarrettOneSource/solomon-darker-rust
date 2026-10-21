#!/usr/bin/env node
// The save boundary is independent of engine restore and accepts some wider values.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, writeFixture } from './oracle.mjs'
const { compileWebLuaDefinition } = await loadKernel('modding/definition/web-lua-definition-compiler.ts')
const codec = await loadKernel('host/prepared-mod-save.ts')
const directory = process.env.SOLOMON_MOD_FIXTURES_DIR ?? fixturesDir
const base = JSON.parse(readFileSync(resolve(directory, 'mod-compiler-goldens.json'))).cases[0].definition
const mods = ['save.first','save.second'].map((id,index)=>compileWebLuaDefinition(
  { id, name: `Save ${index}`, version: '1.0.0' }, structuredClone(base)))
const scope = { id: 'run:1', kind: 'party-run' }
const checkpoint = {
  enemies: { enemies: [{ contentId: mods[0].content.find(row=>row.contentKind==='enemy').contentId,
    currentHealth: 25.5, deathTick: null, headingIndex: 7, id: 1, lastAttackTick: 1,
    lastDamagedByPlayerId: 'local', lifeState: 'alive', maximumHealth: 100, moving: true,
    nextAttackTick: 4, spawnedTick: 0, targetPlayerId: null, x: -0.25, y: 17.5 }], nextId: 3, revision: 9 },
  powerups: { instances: [], nextId: 4, revision: 8 },
  scenes: { nextEpoch: 5, scenes: [{ epoch: 1, ownerId: 'local', parentContentId: null,
    roomIndex: 0, sceneContentId: mods[1].content.find(row=>row.contentKind==='scene').contentId }],
    stacks: [{ ownerId: 'local', roomIndexes: [0,1], sceneContentIds: ['old','current'] }] },
  semanticState: { revision: 7, values: [] },
  shops: { revision: 6, stock: [{ playerId: 'local', remaining: 4, restockTick: null, row: 0,
    shopContentId: mods[0].content.find(row=>row.contentKind==='shop').contentId }] },
  skills: { bindings: [{ contentId: mods[0].content.find(row=>row.contentKind==='skill').contentId, playerId: 'local', slot: 3 }],
    offers: [{ contentIds: ['skill.one','skill.two'], playerId: 'local', sequence: 2 }],
    ranks: [{ contentId: mods[0].content.find(row=>row.contentKind==='skill').contentId, playerId: 'local', rank: 1 }], revision: 5 },
  spells: { cooldowns: [], revision: 4 },
  spellEffects: { effects: [], nextId: 7, nextSequence: 1000000008, revision: 3 },
  session: { graphSha256: mods.map(mod=>mod.graphSha256), nextIntentSequence: 11, nextTimerId: 13,
    reducerHealth: [{ disabled: false, failures: 2, key: 'progress', modId: mods[0].identity.id }],
    state: { cells: [], revision: 2 }, timers: [] },
  statuses: { instances: [], nextInstanceId: 9, revision: 1 },
}
for (const [index,mod] of mods.entries()) {
  const content = kind => mod.content.find(row=>row.contentKind===kind).contentId
  checkpoint.powerups.instances.push({ contentId: content('powerup'), id: index+1, modId: mod.identity.id, spawnedTick: index, x: 2.5, y: -7 })
  checkpoint.spells.cooldowns.push({ contentId: content('spell'), playerId: 'local', readyTick: index+2 })
  checkpoint.spellEffects.effects.push({ contentId: content('spell'), effects: [{ fields: { amount: 3, target: 'enemy' }, kind: 'damage' }],
    expiresTick: 30, hitTargets: ['1','2'], id: index+1, intervalTicks: 3, kind: index===0?'area':'projectile',
    lastTick: 7, modId: mod.identity.id, nextPulseTick: 10, ownerPlayerId: 'local', radius: 4,
    scope, speedPerTick: 0.25, startedTick: 1, targetX: 9, targetY: 10, x: 5, y: 6 })
  checkpoint.semanticState.values.push({ key: 'value', modId: mod.identity.id, scope, value: { z: [null,true,3.25], '2':'second','1':'first', a:'last' } })
  checkpoint.session.state.cells.push({ key: 'progress', modId: mod.identity.id, schemaVersion: 1, scope, value: { count: index } })
  checkpoint.session.timers.push({ context: { player_id: 'local' }, dueTick: 12, event: 'timer.tick', id: index+1,
    intervalTicks: index===0?null:2, modId: mod.identity.id, nodeId: 'node.1', payload: [1,'a'], remaining: 3, scope })
  checkpoint.statuses.instances.push({ contentId: content('status'), expiresTick: 30, instanceId: index+1, modId: mod.identity.id, startedTick: 2, targetId: 'local' })
}
const clone = value=>structuredClone(value)
const cases=[]
function record(name, operation, value, ownerMods=mods) {
  try {
    const output = operation==='encode' ? codec.encodePreparedModSaveState(ownerMods,value)
      : codec.decodePreparedModSaveState(ownerMods,value)
    cases.push({ name, operation, value, mods: ownerMods, output,
      ...(operation==='encode' ? { outputJson: JSON.stringify(output) }
        : { reencodedJson: JSON.stringify(codec.encodePreparedModSaveState(ownerMods,output)) }),
    })
  } catch (error) { cases.push({ name, operation, value, mods: ownerMods, error: error.message }) }
}
record('all-ten-checkpoint-domains','encode',checkpoint)
const encoded = codec.encodePreparedModSaveState(mods,checkpoint)
record('all-ten-checkpoint-domains','decode',encoded)
record('source-mod-order-is-checkpoint-order','decode',codec.encodePreparedModSaveState([...mods].reverse(),{
  ...checkpoint,session:{...checkpoint.session,graphSha256:[...checkpoint.session.graphSha256].reverse()}
}),[...mods].reverse())
record('encode-empty-mod-list','encode',{...checkpoint,session:{...checkpoint.session,graphSha256:[]}},[])
record('decode-empty-mod-list','decode',{},[])
record('encode-wrong-graph-count','encode',{...checkpoint,session:{...checkpoint.session,graphSha256:[]}})
const changes = [
  ['unknown-top-mod-preserved-outside-codec',value=>{value.unknown={arbitrary:true}}],
  ['unknown-fields-ignored',value=>{value[mods[0].identity.id].unknown={arbitrary:true}}],
  ['missing-first-mod',value=>{delete value[mods[0].identity.id]}],
  ['missing-runtime',value=>{delete value[mods[0].identity.id].runtime}],
  ['runtime-duplicated',value=>{value[mods[1].identity.id].runtime=null}],
  ['wrong-api',value=>{value[mods[0].identity.id].api='2.0.0'}],
  ['wrong-graph',value=>{value[mods[1].identity.id].graph_sha256='bad'}],
  ['wrong-content-owner',value=>{value[mods[0].identity.id].statuses[0].content_id=mods[1].content[0].contentId}],
  ['missing-cell-value-becomes-null',value=>{delete value[mods[0].identity.id].state_cells[0].value}],
  ['missing-timer-payload-becomes-null',value=>{delete value[mods[0].identity.id].timers[0].payload}],
  ['missing-semantic-value-becomes-null',value=>{delete value[mods[0].identity.id].state_values[0].value}],
  ['bad-cell-scope',value=>{value[mods[0].identity.id].state_cells[0].scope_kind='future'}],
  ['bad-timer-scope',value=>{value[mods[0].identity.id].timers[0].scope_kind='future'}],
  ['bad-effect-scope',value=>{value[mods[0].identity.id].spell_effects[0].scope_kind='future'}],
  ['bad-semantic-scope',value=>{value[mods[0].identity.id].state_values[0].scope_kind='future'}],
  ['bad-effect-kind',value=>{value[mods[0].identity.id].spell_effects[0].kind='future'}],
  ['channel-effect',value=>{value[mods[0].identity.id].spell_effects[0].kind='channel'}],
  ['timer-interval-missing-is-invalid',value=>{delete value[mods[0].identity.id].timers[0].interval_ticks}],
  ['fractional-id-invalid',value=>{value[mods[0].identity.id].powerups[0].id=1.25}],
  ['unsafe-id-invalid',value=>{value[mods[0].identity.id].powerups[0].id=9007199254740992}],
  ['safe-binding-slot-beyond-u32',value=>{value[mods[0].identity.id].runtime.skill_bindings[0].slot=4294967297}],
  ['safe-maximum-counter',value=>{value[mods[0].identity.id].runtime.next_timer_id=Number.MAX_SAFE_INTEGER}],
  ['nonempty-unusual-life-state',value=>{value[mods[0].identity.id].runtime.enemies[0].life_state='future';value[mods[0].identity.id].runtime.enemies[0].current_health=0}],
  ['heading-beyond-u32-decoder-only',value=>{value[mods[0].identity.id].runtime.enemies[0].heading_index=4294967297}],
  ['rank-beyond-u32-decoder-only',value=>{value[mods[0].identity.id].runtime.skill_ranks[0].rank=4294967297}],
  ['remaining-beyond-u32-decoder-only',value=>{value[mods[0].identity.id].runtime.shop_stock[0].remaining=4294967297}],
  ['room-index-beyond-wasm-usize-decoder-only',value=>{value[mods[0].identity.id].runtime.scenes[0].room_index=4294967297}],
  ['shop-row-beyond-wasm-usize-decoder-only',value=>{value[mods[0].identity.id].runtime.shop_stock[0].row=4294967297}],
  ['utf16-text-maximum-accepted',value=>{value[mods[0].identity.id].timers[0].event='😀'.repeat(128)}],
  ['empty-text-invalid',value=>{value[mods[0].identity.id].timers[0].event=''}],
  ['utf16-text-limit',value=>{value[mods[0].identity.id].timers[0].event='😀'.repeat(129)}],
  ['wrong-bool',value=>{value[mods[0].identity.id].runtime.enemies[0].moving=1}],
  ['wrong-array',value=>{value[mods[0].identity.id].spell_cooldowns={}}],
  ['wrong-template-object',value=>{value[mods[0].identity.id].spell_effects[0].effects[0].fields=[]}],
]
for (const [name,change] of changes) { const value=clone(encoded);change(value);record(name,'decode',value) }
writeFixture('prepared-mod-save-goldens.json','tools/website-oracle/prepared-mod-save-goldens.mjs',
  'Website host/prepared-mod-save.ts; codec contracts are distinct from later engine restore', { cases })
console.log(JSON.stringify({cases:cases.length}))
