// Actual PreparedModHost transactions over the pinned canonical game owner.
import { createRequire } from 'node:module'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const require = createRequire(resolve(websiteDir, 'frontend/package.json'))
const wasmPath = require.resolve('wasmoon/dist/glue.wasm')
const { materializeWebSessionContent, compileWebSessionContentDefinitions } = await loadKernel('host/web-mod-content.ts')
const { prepareModHost } = await loadKernel('host/prepared-mod-host.ts')
const simulation = await loadKernel('core-server/game-simulation.ts')
const { setPlayerEntityMana } = await loadKernel('core-server/player-entity-store.ts')
const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const fixture = JSON.parse(readFileSync(new URL('../../crates/sim/tests/fixtures/website/mod-host-runtime-goldens.json', import.meta.url)))
const payload = fixture.payload
payload.mods[0].entryScript = payload.mods[0].entryScript.replace('return sd.mod({', `
local grant_probe = sd.rules.on("wave.completed", sd.effect.grant({ target = "user", item = sd.ref("item", "ingredient"), quantity = 2 }))
local rejected_probe = sd.rules.on("enemy.death", sd.rules.all({
  sd.effect.grant({ target = "user", item = sd.ref("item", "ingredient"), quantity = 3 }),
  sd.effect.spawn({ enemy = sd.ref("enemy", "grave_tyrant") }),
}))
local spawn_probe = sd.rules.on("enemy.spawned", sd.effect.spawn({ enemy = sd.ref("enemy", "grave_tyrant") }))
return sd.mod({`).replace('rules = {spawn_content, probe, remember_purchase}', 'rules = {spawn_content, probe, remember_purchase, grant_probe, rejected_probe, spawn_probe}')
const content = await compileWebSessionContentDefinitions(materializeWebSessionContent(payload), wasmPath)
let state = simulation.createGameSimulation({ 'player-1': { discipline: 'arcane', displayName: 'Tester', element: 'ether' } })
const loaded = { ...NATIVE_GENERATED_BONEYARDS[0], seed: 'host-game-seed', runId: 'host-game-run', choice: { id: 'mod-host-game', source: 'mod', modId: 'example.host-content', modName: 'Host Content', name: 'Host Game' } }
const host = await prepareModHost({ content, state: { read: () => state, write: value => { state = value } }, wasmPath, now: () => 0 })
const cases = [], clone = value => JSON.parse(JSON.stringify(value))
function snapshot() {
  const progression = simulation.getPlayerProgression(state, 'player-1'), economy = simulation.getPlayerEconomy(state, 'player-1')
  return { health: progression.currentHealth, mana: progression.currentMana, gold: economy.gold, experience: progression.experience, level: progression.level, backpack: economy.backpack }
}
function record(input) {
  let output = null, error = null
  try {
    switch (input.kind) {
      case 'step': output = host.step(input.events, input.tick, input.scopeId ?? 'run-1', input.context ?? { participant_id: 'player-1' }); break
      case 'tick': output = host.tick(input.tick); break
      case 'mana': state = { ...state, playerEntities: setPlayerEntityMana(state.playerEntities, 'player-1', input.amount) }; break
      case 'consume': output = host.consume(input.consumption); break
      case 'enter': state = simulation.enterBoneyardWorld(state, loaded); break
      case 'activate': host.activateBoneyard(input.contentId, input.restoreExisting); break
      case 'cast': output = host.cast(input.input); break
      case 'restore': host.restore(input.checkpoint); break
      case 'drain': output = { spawns: host.drainEnemySpawns(), presentation: host.drainPresentation() }; break
    }
  } catch (caught) { error = caught.message }
  cases.push({ input, output, error, game: snapshot(), checkpoint: host.checkpoint() })
}
record({ kind: 'mana', amount: 0 })
record({ kind: 'consume', consumption: { content: host.content.consumables()[0].content, playerId: 'player-1', tick: 10, useId: 1 } })
record({ kind: 'step', events: [{ name: 'gold.changed', payload: {} }], tick: 13 })
record({ kind: 'tick', tick: 19 })
record({ kind: 'tick', tick: 20 })
record({ kind: 'step', events: [{ name: 'gold.changed', payload: {} }], tick: 21 })
record({ kind: 'step', events: [{ name: 'wave.completed', payload: {} }], tick: 22 })
record({ kind: 'step', events: [{ name: 'enemy.death', payload: {} }], tick: 23 })
record({ kind: 'activate', contentId: 'missing', restoreExisting: true })
record({ kind: 'enter' })
record({ kind: 'step', events: [{ name: 'run.started', payload: { run_id: 'host-game-run' } }], tick: 24 })
record({ kind: 'step', events: [{ name: 'enemy.spawned', payload: {} }], tick: 25 })
for (const tick of [25, 26, 34, 50]) record({ kind: 'tick', tick })
record({ kind: 'cast', input: { contentId: host.content.spells()[0].contentId, playerId: 'player-1', requestId: 3 } })
const checkpoint = clone(host.checkpoint())
checkpoint.skills.ranks = [{ contentId: host.content.skills()[0].contentId, playerId: 'player-1', rank: 1 }]
record({ kind: 'restore', checkpoint })
record({ kind: 'cast', input: { contentId: host.content.spells()[0].contentId, playerId: 'player-1', requestId: 4 } })
record({ kind: 'cast', input: { contentId: host.content.spells()[0].contentId, playerId: 'player-1', requestId: 5 } })
for (const tick of [1, 9, 10, 100, 125, 225]) record({ kind: 'tick', tick })
record({ kind: 'drain' })
record({ kind: 'step', events: [{ name: 'run.ended', payload: { run_id: 'host-game-run' } }], tick: 226, scopeId: 'host-game-run' })
host.close()
writeFixture('mod-host-game-goldens.json', 'mod-host-game-goldens.mjs', 'host/prepared-mod-host.ts::prepareModHost,applyIntent,step,tick,consume,cast', { payload, loaded, cases })
