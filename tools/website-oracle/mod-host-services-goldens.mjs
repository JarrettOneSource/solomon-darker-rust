// Service actions and every runtime projection family use the real Source host.
import { createRequire } from 'node:module'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const require = createRequire(resolve(websiteDir, 'frontend/package.json'))
const wasmPath = require.resolve('wasmoon/dist/glue.wasm')
const { materializeWebSessionContent, compileWebSessionContentDefinitions } = await loadKernel('host/web-mod-content.ts')
const { prepareModHost } = await loadKernel('host/prepared-mod-host.ts')
const simulation = await loadKernel('core-server/game-simulation.ts')
const { replacePlayerEconomy } = await loadKernel('core-server/player-entity-store.ts')
const { unequipInventorySlot } = await loadKernel('core-kernels/hub-economy.ts')
const base = JSON.parse(readFileSync(new URL('../../crates/sim/tests/fixtures/website/mod-host-game-goldens.json', import.meta.url)))
const provision = JSON.parse(readFileSync(new URL('../../crates/sim/tests/fixtures/website/mod-provisioning-goldens.json', import.meta.url)))
const payload = structuredClone(base.payload)
function file(path, bytes, kind, contentType) {
  return { path, byteLength: bytes.length, bytesBase64: bytes.toString('base64'), kind, contentType, sha256: createHash('sha256').update(bytes).digest('hex') }
}
payload.mods[0].files.push(file('scenes/layout.json', Buffer.from(JSON.stringify({schemaVersion:1,name:'Scene',rooms:['room']})), 'scene', 'application/json'))
payload.mods[0].files.push(file('levels/test.boneyard', Buffer.from(provision.boneyardBase64, 'base64'), 'boneyard', 'application/vnd.solomon-dark.boneyard'))
payload.mods[0].files.push(file('audio/tone.wav', Buffer.from('RIFF0000WAVE'), 'audio', 'audio/wav'))
payload.mods[0].entryScript = payload.mods[0].entryScript
  .replace('return sd.mod({', `
local layout = sd.art.scene("scenes/layout.json")
local music = sd.art.music("audio/tone.wav")
local sound = sd.art.sound("audio/tone.wav", {volume = 0.5})
local room = sd.kit.room({key = "room", name = "Room", geometry = {kind = "inline", width = 200, height = 180, floor = "#102030"}, art = {ambience = sd.art.ref("music")}})
local scene = sd.kit.scene({key = "scene", name = "Scene", rooms = {sd.ref("room", "room"), sd.ref("room", "room")}, art = {layout = sd.art.ref("layout")}})
local extension = sd.kit.scene_extension({key = "portal", scene = "stock.boneyard", features = {sd.prefab.portal({selector = {object_kind = "monument"}, destination = sd.ref("scene", "scene")})}})
local ui = sd.kit.ui({key = "ui", name = "Survey", mount = "hud.top_right", actions = {"ready"}, visible = {state = {state = "lesson.ready", equals = true}}, bindings = {ready = {state = "lesson.ready"}}, view = sd.prefab.minimap({size = 220, range = 500})})
local affix = sd.kit.affix({key = "power", name = "Empowered", modifiers = {incoming_damage = {multiply = 0.5}}})
local pool = sd.kit.affix_pool({key = "pool", entries = {{affix = sd.ref("affix", "power"), weight = 1}}, rolls = 1})
local forge = sd.kit.shop({key = "forge", name = "Forge", stock = {{item = sd.ref("item", "ingredient"), price = 1, quantity = 1}}, services = {{type = "reforge", pool = sd.ref("affix-pool", "pool"), price = 5}}})
local boneyard = sd.kit.boneyard({key = "yard", name = "Yard", source = "levels/test.boneyard", roster = {sd.ref("enemy", "grave_tyrant"), "stock.skeleton_archer"}, waves = {{wave = 2, roster = {"stock.imp"}}}, art = {music = sd.art.ref("music")}})
local pickup = sd.rules.on("wave.started", sd.effect.spawn({content = sd.ref("powerup", "survey_orb")}))
local ui_rule = sd.rules.on("action.ui.action", sd.effect.state({key = "ui.used", value = true}))
return sd.mod({`)
  .replace('assets = {icon = icon}', 'assets = {icon = icon, layout = layout, music = music, sound = sound}')
  .replace('content = {status, potion, powerup, ingredient, shop, spell, skill, enemy, boast}', 'content = {status, potion, powerup, ingredient, shop, spell, skill, enemy, boast, room, scene, extension, ui, affix, pool, forge, boneyard}')
  .replace('rules = {spawn_content, probe, remember_purchase, grant_probe, rejected_probe, spawn_probe}', 'rules = {spawn_content, probe, remember_purchase, grant_probe, rejected_probe, spawn_probe, pickup, ui_rule}')
const content = await compileWebSessionContentDefinitions(materializeWebSessionContent(payload), wasmPath)
let state = simulation.createGameSimulation({ 'player-1': { discipline: 'arcane', displayName: 'Tester', element: 'ether' } })
const host = await prepareModHost({ content, state: { read: () => state, write: value => { state = value } }, wasmPath, now: () => 0 })
const cases = []
const id = key => host.content.all().find(value => value.key === key).contentId
function snapshot() {
  const player = simulation.getPlayerProgression(state, 'player-1'), economy = simulation.getPlayerEconomy(state, 'player-1')
  return { health: player.currentHealth, mana: player.currentMana, gold: economy.gold, experience: player.experience, level: player.level, backpack: economy.backpack }
}
function record(input) {
  let output = null, error = null
  try {
    switch (input.kind) {
      case 'purchase': host.purchaseShop('player-1', input.contentId, input.row); break
      case 'xp': state = simulation.grantGameSimulationPlayerExperience(state, 'player-1', input.amount); break
      case 'step': output = host.step(input.events, input.tick, 'host-game-run', { participant_id: 'player-1' }); break
      case 'choose': host.chooseSkill('player-1', input.contentId, input.modSequence, input.nativeSequence); break
      case 'bind': host.bindModQuickbar('player-1', input.slot, input.contentId); break
      case 'portal': host.enterPortal(input.input); break
      case 'room': output = host.selectSceneRoom('host-game-run', input.room); break
      case 'return': output = host.returnScene('host-game-run'); break
      case 'ui': output = host.uiAction(input.input); break
      case 'unequip': { const result = unequipInventorySlot(simulation.getPlayerEconomy(state, 'player-1'), 'weapon'); if (!result.accepted) throw new Error(result.reason); state = {...state, playerEntities: replacePlayerEconomy(state.playerEntities, 'player-1', result.state)}; break }
      case 'reforge': host.reforgeShop('player-1', input.contentId, input.service, input.itemId); break
      case 'enter': state = simulation.enterBoneyardWorld(state, base.loaded); break
      case 'activate': host.activateBoneyard(input.contentId); break
      case 'tick': output = host.tick(input.tick); break
      case 'drain': output = {spawns:host.drainEnemySpawns(),presentation:host.drainPresentation()}; break
    }
  } catch (caught) { error = caught.message }
  cases.push({ input, output, error, game: snapshot(), checkpoint: host.checkpoint(), content:host.project(), runtime:host.runtimeProjection('player-1') })
}
record({kind:'ui',input:{action:'ready',arguments:{},contentId:id('ui'),playerId:'player-1',requestId:1}})
record({kind:'purchase',contentId:id('test_shop'),row:0})
record({kind:'purchase',contentId:id('test_shop'),row:0})
record({kind:'purchase',contentId:id('test_shop'),row:0})
record({kind:'xp',amount:100})
record({kind:'step',events:[{name:'level.up',payload:{player_id:'player-1',level:2}}],tick:0})
const nativeSequence=simulation.getPlayerProgression(state,'player-1').pendingOffer.sequence
const modSequence=host.checkpoint().skills.offers[0].sequence
record({kind:'choose',contentId:id('gravity_student'),modSequence,nativeSequence:nativeSequence+1})
record({kind:'choose',contentId:id('gravity_student'),modSequence,nativeSequence})
record({kind:'bind',slot:0,contentId:id('gravity_well')})
record({kind:'bind',slot:8,contentId:id('gravity_well')})
record({kind:'step',events:[{name:'gold.changed',payload:{}}],tick:1})
record({kind:'ui',input:{action:'ready',arguments:{accepted:true},contentId:id('ui'),playerId:'player-1',requestId:2}})
record({kind:'ui',input:{action:'wrong',arguments:{},contentId:id('ui'),playerId:'player-1',requestId:3}})
record({kind:'unequip'})
const staff=simulation.getPlayerEconomy(state,'player-1').backpack.find(item=>item.equipmentType==='staff')
record({kind:'reforge',contentId:id('forge'),service:0,itemId:staff.id})
record({kind:'reforge',contentId:id('forge'),service:1,itemId:staff.id})
record({kind:'enter'})
record({kind:'activate',contentId:id('yard')})
record({kind:'step',events:[{name:'wave.started',payload:{wave:2}}],tick:2})
record({kind:'tick',tick:2})
const portal={actorKind:'monument',confirmedByLeader:false,ownerId:'host-game-run',playerId:'player-1',portalId:host.runtimeProjection().projection.portals[0].id,scene:'stock.boneyard'}
record({kind:'portal',input:portal})
record({kind:'portal',input:{...portal,confirmedByLeader:true}})
record({kind:'room',room:1})
record({kind:'room',room:2})
record({kind:'return'})
record({kind:'drain'})
host.close()
writeFixture('mod-host-services-goldens.json','mod-host-services-goldens.mjs','host/prepared-mod-host.ts::services,project,runtimeProjection', {payload,loaded:base.loaded,cases})
