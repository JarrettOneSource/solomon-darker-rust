#!/usr/bin/env node
// Full policy observations from real saved Game owners and subsequent public ticks.
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const sourceRevision=execFileSync('git',['rev-parse','HEAD'],{cwd:websiteDir,encoding:'utf8'}).trim()

const game = await loadKernel('core-server/game-simulation.ts')
const saves = await loadKernel('save/game-save-document.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const economy = await loadKernel('core-kernels/hub-economy.ts')
const skills = await loadKernel('core-kernels/player-skill-runtime.ts')
const belt = await loadKernel('core-kernels/native-belt.ts')
const secondary = await loadKernel('core-kernels/native-secondary-abilities.ts')
const { boneyardGeometrySha256 } = await loadKernel('host/project-boneyard.ts')
const { MlBotPolicyObserver } = await loadKernel('core-server/ml-bot-policy/observer.ts')
const { createEnemyWork, finishEnemyStore } = await loadKernel('core-server/enemies/work.ts')
const { materializeSpawnIntents } = await loadKernel('core-server/enemies/construction.ts')
const { spawnProjectile } = await loadKernel('core-server/enemies/projectile-emission.ts')
const { createNativeWorldManagerOrder } = await loadKernel('core-kernels/native-world-manager-order.ts')
const births = JSON.parse(readFileSync(resolve(fixturesDir, 'enemy-construction-goldens.json')))

const row = value => ({ currentHealth: value.currentHealth, headingDeg: value.headingDeg,
  id: value.id, maximumHealth: value.maximumHealth, position: value.position,
  radius: value.radius, species: value.species, targetPlayerId: value.targetPlayerId, velocity: value.velocity })
// JSON.stringify maps -0 to 0; retain the actual Float32 words separately so
// exact numeric replay does not accidentally demand a sign change from Source.
const frameValue = value => ({ values: Array.from(value.values),
  valueBits: Array.from(new Uint32Array(value.values.buffer, value.values.byteOffset, value.values.length)),
  enemyRows: value.enemyRows.map(row),
  targetId: value.targetId, player: { primaryAffordable: value.player.primaryAffordable,
    primaryRange: value.player.primaryRange, secondarySlots: value.player.secondarySlots },
  potions: value.inventory.potions, ownMinionTargetIds: [...value.minions.ownTargetIds].sort((a,b)=>a-b),
  secondaryMinionActive: value.minions.secondaryMinionActive,
})
const scene = { name: 'Policy Arena', environmentMode: 0,
  bounds: { x: 0, y: 0, w: 2000, h: 2000 }, spawn: { x: 1000, y: 1000, facingDeg: 0 },
  objects: [], sprites: [], roads: [], fences: [], terrain: [], solomonDig: null }
const cases = []
const specs = [
  { name: 'all-family-fronts', element: 'fire', rotation: 0, ticks: [0,1,1,5,20,80,160,240] },
  { name: 'all-family-rear-clocks', element: 'earth', rotation: 7, ticks: [0,1,4,30,100] },
  { name: 'rich-party', element: 'fire', rotation: 3, rich: true, team: true, ticks: [0,1,1,10] },
  ...['air','water','ether'].map(element=>({ name: `${element}-primary`, element, rotation: 5, ticks:[0,1,10] })),
]
for (const spec of specs) {
  const playerIds = spec.team ? ['local','a','A','ä','á','a-b','a_b'] : ['local']
  const configurations = Object.fromEntries(playerIds.map(id=>[id, {
    displayName: id, element: spec.element, discipline: spec.rich ? 'body' : 'arcane',
  }]))
  const loaded = { scene, choice: { id: 'policy-arena', name: scene.name, source: 'default' },
    sourceSha256: createHash('sha256').update(JSON.stringify(scene)).digest('hex'),
    geometrySha256: boneyardGeometrySha256(scene), seed: `policy:${spec.name}`, runId: spec.name }
  let state = game.enterBoneyardWorld(game.createGameSimulation(configurations, {
    gameRngSeed: 17, combatRngSeed: 91,
  }), loaded)
  for (const id of playerIds) {
    state.playerEntities = entities.replacePlayerCharacter(state.playerEntities, id, {
      ...game.getPlayerCharacter(state, id), position: { x: id === 'local' ? 1000 : 1100, y: 1000 },
    })
  }
  if (spec.rich) {
    const old = game.getPlayerEconomy(state, 'local')
    const stock = economy.FOMENTIUS_STOCK_DEFINITIONS.map((definition, index)=>economy.createFomentiusInventoryItem(definition, 100+index, index+1))
    const sack = stock.find(item=>item.kind==='sack')
    sack.contents = [economy.createFomentiusInventoryItem(economy.FOMENTIUS_STOCK_DEFINITIONS[0], 140, 13)]
    const equipment = { ...old.equipment,
      hat: economy.createEquipmentInventoryItem(economy.DOWSING_EQUIPMENT_RECIPES[0], 150),
      robe: economy.createEquipmentInventoryItem(economy.DOWSING_EQUIPMENT_RECIPES[1], 151),
      weapon: economy.createEquipmentInventoryItem(economy.DOWSING_EQUIPMENT_RECIPES[2], 152),
      rings: [3,4,5].map((recipe,index)=>economy.createEquipmentInventoryItem(economy.DOWSING_EQUIPMENT_RECIPES[recipe], 153+index)),
    }
    state.playerEntities = entities.replacePlayerEconomy(state.playerEntities, 'local', {
      ...old, backpack: stock, equipment,
    })
    const store = state.playerEntities
    const book = structuredClone(store.skillBooks[0])
    for (const id of [11,21,27,35,41,45,54,72]) {
      book.permanentRanks[id] = 1
      if (!book.learnedSkillOrder.includes(id)) book.learnedSkillOrder.push(id)
    }
    const refreshed = skills.refreshPlayerSkillRuntime(store.skillRuntimes[0], book, store.statBooks[0], store.economies[0])
    let bound = belt.createNativePlayerBelt(refreshed.skillBook)
    for (const [slot,id] of [11,21,27,35,41,45,54,72].entries()) bound = belt.bindNativeBeltSkill(bound, refreshed.skillBook, id, slot)
    state.playerEntities = { ...store, skillBooks: [refreshed.skillBook,...store.skillBooks.slice(1)],
      skillRuntimes: [refreshed.runtime,...store.skillRuntimes.slice(1)], belts: [bound,...store.belts.slice(1)] }
    const progression = { ...state.playerEntities.progressions[0], currentHealth: 47, currentMana: 18,
      poisonTicksRemaining: 75, poisonDamagePerTick: 0.025,
      damageX4TicksRemaining: 83, poisonImmunityTicksRemaining: 7, mindChugTicksRemaining: 14 }
    state.playerEntities = { ...state.playerEntities, progressions: [progression,...state.playerEntities.progressions.slice(1)] }
    const player = { ...secondary.createNativeSecondaryPlayerState(), magicShieldAbsorb: 5,
      magicShieldMaximum: 10, globalCooldownTicks: 7, heldSlot: 3, stoneskinTicksRemaining: 120,
      cooldownTicksBySkill: Array.from({length:83},(_,id)=>id===21?17:0),
      cooldownMaximumTicksBySkill: Array.from({length:83},(_,id)=>id===21?123:0), }
    state.secondaryAbilities = { ...state.secondaryAbilities, players: { local: player } }
  }
  const order = createNativeWorldManagerOrder(state.worldManagerOrder)
  const players = Object.fromEntries(playerIds.map(id=>[id, { alive:true,connected:true,eligible:true,
    collisionRadius:25,position:game.getPlayerCharacter(state,id).position,velocityPerTick:{x:0,y:0} }]))
  const context = { tick: state.tick, players, resolveMovement: request=>request.requestedPosition,
    registerWorldPainter: order.register }
  const work = createEnemyWork(state.world.enemies, context, true)
  work.actors = materializeSpawnIntents(work, context, births.cases[0].intents.map((intent,index)=>({
    ...intent, spawnTick:state.tick, positionPolicy:'direct', position: {
      x: 1050 + ((index+spec.rotation)%14)*17, y: 940+index%3*30,
    },
  })))
  work.actors = work.actors.map(actor=>actor.brain.family==='coffin' ? {
    ...actor, brain: { ...actor.brain, phase:'opening', phaseTicksRemaining:2 },
  } : actor)
  for (const [index,kind] of ['arrow','demon-bomb','firebolt','guided-missile','poison-pool'].entries()) {
    spawnProjectile(work, work.actors.find(actor=>actor.config.enemyToken==='ZOMBIE'), state.tick, kind, 8, {
      headingDeg: 30+index*45, position:{x:1000+index*35,y:900+index*20},
      payload: index%2===0?'cold':'poison', coldSlowTicks:index%2===0?250:0,
      poisonDamage:index%2===1?2:0, poisonDuration:10,
      ...(kind==='arrow'?{speed:6,lifetimeTicks:15,minimumSpeed:6}:{}),
    })
  }
  state = { ...state, worldManagerOrder:order.state(), world:{ ...state.world,enemies:finishEnemyStore(work,state.tick) } }
  const document = saves.createGameSaveDocument({ state, playerId:'local', loadedBoneyard:loaded,
    integrity:'global-clean',mods:[],modState:{} })
  // Compare the two public restores before observing. Source normalization is part
  // of the exact starting transaction, rather than a second hand-written adapter.
  state = saves.restoreGameSaveDocument(document).state
  const observer = new MlBotPolicyObserver('local')
  const activeInputs = Object.fromEntries(playerIds.map((id,index)=>[id, {
    aim:null,cast:{primary:false,quickbar:null},movement:{x:index%2?0.5:0,y:0},
    viewportWidth:1600,viewportHeight:900,
  }]))
  const controllers = Object.fromEntries(playerIds.map(id=>[id,id==='local'?'bot':'human']))
  const frames=[]
  for (const [index,tick] of spec.ticks.entries()) {
    while(state.tick<tick) state=game.stepGameSimulationTick(state,activeInputs)
    const frame = observer.observe(state,{activeInputs,controllers})
    const targetId = frame.enemyRows[index%Math.max(1,frame.enemyRows.length)]?.id??null
    const committed = { abilityAction:index%4,movement:{x:index%2,y:0},targetAction:index%9,targetSwitched:targetId!==frame.targetId }
    frames.push({tick,output:frameValue(frame),commit:{action:committed,targetId}})
    observer.commit(committed,targetId)
  }
  cases.push({name:spec.name,document:JSON.parse(document),activeInputs,controllers,frames})
}
writeFixture('ml-bot-observer-goldens.json','tools/website-oracle/ml-bot-observer-goldens.mjs',
  'Website core-server/ml-bot-policy/observer.ts + public Game save restore and ticks', {sourceRevision,cases})
console.log(JSON.stringify({cases:cases.length,frames:cases.reduce((sum,row)=>sum+row.frames.length,0)}))
