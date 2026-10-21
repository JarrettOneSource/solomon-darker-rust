// Source PreparedModHost owns one catalog/engine collection and atomic restore.
import { createRequire } from 'node:module'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const require = createRequire(resolve(websiteDir, 'frontend/package.json'))
const wasmPath = require.resolve('wasmoon/dist/glue.wasm')
const { materializeWebSessionContent, compileWebSessionContentDefinitions } = await loadKernel('host/web-mod-content.ts')
const { prepareModHost } = await loadKernel('host/prepared-mod-host.ts')
const { createGameSimulation, getPlayerEconomy } = await loadKernel('core-server/game-simulation.ts')
const { replacePlayerEconomy } = await loadKernel('core-server/player-entity-store.ts')
const test = readFileSync(resolve(websiteDir, 'frontend/src/game/host/prepared-mod-host.test.ts'), 'utf8')
const code = test.split('const SCRIPT = `')[1].split('`')[0]
const bytes = Buffer.alloc(24)
bytes.set([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])
bytes.writeUInt32BE(13, 8); bytes.write('IHDR', 12); bytes.writeUInt32BE(53, 16); bytes.writeUInt32BE(50, 20)
const payload = { manifestSha256: '1'.repeat(64), mods: [{ boneyards: [], contentSha256: '2'.repeat(64), entryScript: code, files: [{ byteLength: bytes.length, bytesBase64: bytes.toString('base64'), contentType: 'image/png', kind: 'image', path: 'art/icon.png', sha256: createHash('sha256').update(bytes).digest('hex') }], id: 'example.host-content', name: 'Host Content', priority: 0, slug: 'host-content', version: '1.0.0' }] }
const content = await compileWebSessionContentDefinitions(materializeWebSessionContent(payload), wasmPath)
let state = createGameSimulation({ 'player-1': { discipline: 'arcane', displayName: 'Tester', element: 'ether' } })
const host = await prepareModHost({ content, state: { read: () => state, write: value => { state = value } }, wasmPath, now: () => 0 })
const cases = [], clone = value => JSON.parse(JSON.stringify(value))
function record(input) {
  let output = null, error = null
  try {
    switch (input.kind) {
      case 'checkpoint': output = host.checkpoint(); break
      case 'restore': host.restore(input.checkpoint); break
      case 'damage': output = host.extensions.filterDamage(input.input); break
      case 'mana': output = host.extensions.filterMana(input.input); break
      case 'loot': output = host.extensions.createLootItems(input.input); break
      case 'has-consumable': output = host.extensions.hasConsumable(input.contentId); break
      case 'boast': {
        const value = host.extensions.resolveBoast(input.selection)
        output = value === null ? null : { failureProducers: value.failureProducers, label: value.label, randomSkillChoices: value.randomSkillChoices, scoreMultiplier: value.scoreMultiplier, successWave: value.successWave }
        break
      }
      case 'equipment': state = { ...state, playerEntities: replacePlayerEconomy(state.playerEntities, 'player-1', { ...getPlayerEconomy(state, 'player-1'), equipment: input.value }) }; break
    }
  } catch (caught) { error = caught.message }
  cases.push({ input, output, error, checkpoint: host.checkpoint() })
}
const initial = clone(host.checkpoint())
record({ kind: 'checkpoint' })
const status = host.content.all().find(value => value.contentKind === 'status')
const skill = host.content.skills()[0]
const potion = host.content.consumables()[0]
const boast = host.content.boasts()[0]
record({ kind: 'has-consumable', contentId: potion.content.contentId })
record({ kind: 'has-consumable', contentId: skill.contentId })
record({ kind: 'boast', selection: boast.selection })
record({ kind: 'boast', selection: 1 })
for (const seed of [0, 1, 123, 0x3fffffff]) record({ kind: 'loot', input: { actorSeed: seed, enemyToken: seed % 2 ? 'DEMON' : 'SKELETON' } })
const active = clone(initial)
active.statuses = { instances: [{ contentId: status.contentId, expiresTick: 10, instanceId: 1, modId: status.modId, startedTick: 0, targetId: 'player-1' }], nextInstanceId: 2, revision: 1 }
record({ kind: 'restore', checkpoint: active })
for (const tick of [0, 9, 10]) {
  record({ kind: 'damage', input: { amount: 20, damageKind: 'physical', sourceActorId: null, targetPlayerId: 'player-1', tick } })
  for (const delta of [-30, 5]) record({ kind: 'mana', input: { currentMana: 20, delta, maximumMana: 50, playerId: 'player-1', source: 'primary-cast', tick } })
}
const equipment = clone(getPlayerEconomy(state, 'player-1').equipment)
equipment.hat.modAffixes = [{ contentId: '1', modId: status.modId, modifiers: [{ key: 'incoming_damage', operation: 'add', value: 2 }, { key: 'mana_spend', operation: 'set', value: 3 }], name: 'First' }]
equipment.robe.modAffixes = [{ contentId: '2', modId: status.modId, modifiers: [{ key: 'incoming_damage', operation: 'multiply', value: 0.5 }, { key: 'mana_spend', operation: 'multiply', value: 0.5 }], name: 'Second' }]
record({ kind: 'equipment', value: equipment })
record({ kind: 'damage', input: { amount: 20, damageKind: 'physical', sourceActorId: null, targetPlayerId: 'player-1', tick: 10 } })
record({ kind: 'mana', input: { currentMana: 20, delta: -30, maximumMana: 50, playerId: 'player-1', source: 'primary-cast', tick: 10 } })
for (const change of [
  value => { value.powerups.instances = [{ contentId: '1', id: 1, modId: status.modId, spawnedTick: 0, x: 0, y: 0 }] },
  value => { value.statuses.instances[0].contentId = '1' },
  value => { value.session.graphSha256 = [] },
  value => { value.spellEffects.effects = [{ contentId: '1', effects: [], expiresTick: 10, hitTargets: [], id: 1, intervalTicks: 1, kind: 'area', lastTick: 0, modId: status.modId, nextPulseTick: 1, ownerPlayerId: 'player-1', radius: 1, scope: { kind: 'participant-run', id: 'player-1:run-1' }, speedPerTick: 0, startedTick: 0, targetX: 0, targetY: 0, x: 0, y: 0 }] },
]) {
  const checkpoint = clone(active); change(checkpoint)
  record({ kind: 'restore', checkpoint })
}
host.close()
writeFixture('mod-host-runtime-goldens.json', 'mod-host-runtime-goldens.mjs', 'host/prepared-mod-host.ts::prepareModHost,extensions,checkpoint,restore', { payload, cases })
