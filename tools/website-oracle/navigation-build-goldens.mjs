#!/usr/bin/env node
// Record the actual runtime triangulator, rather than another retained mesh bank.
import assert from 'node:assert/strict'
import { loadKernel, writeFixture } from './oracle.mjs'

const collision = await loadKernel('core-server/boneyard-collision.ts')
const navigation = await loadKernel('core-server/boneyard-enemy-navigation.ts')
const gates = await loadKernel('core-kernels/boneyard-gate.ts')
const construction = await loadKernel('core-server/boneyard-world-construction.ts')
const catalogKernel = await loadKernel('host/boneyard-catalog.ts')
const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const { STOCK_TUTORIAL_BONEYARD } = await loadKernel('host/stock-tutorial-boneyard.ts')

const scene = (name, objects = [], fences = [], bounds = { x: 0, y: 0, w: 1200, h: 1200 }) => ({
  name, environmentMode: 0, bounds,
  spawn: { x: bounds.x + 150, y: bounds.y + 150, facingDeg: 180 },
  objects, fences, sprites: [], roads: [], terrain: [], solomonDig: null,
})
const object = (eid, typeId, x, y, variant = 0) => ({ eid, typeId, variant, pos: { x, y } })
const fence = (eid, segmentCode, start, end) => ({
  eid, typeId: 2030, segmentCode, points: [start, end],
})
const inputs = [
  scene('offset empty imported scene', [], [], { x: -157.25, y: -95.5, w: 1400, h: 1250 }),
  scene('overlapping authored footprints', [
    object('tree', 2001, 440, 440, 1),
    object('monument', 2009, 458, 465, 19),
    object('building', 2040, 790, 620, 2),
    object('grave', 2029, 410, 750),
    object('goodie', 2061, 750, 920),
  ], [
    fence('stone', 3, { x: 175, y: 300 }, { x: 730, y: 360 }),
    fence('wood', 0, { x: 520, y: 120 }, { x: 520, y: 850 }),
    fence('gap', 1, { x: 350, y: 1000 }, { x: 900, y: 1000 }),
  ]),
  scene('clearance dependent corridor', [], [
    fence('left', 0, { x: 400, y: 100 }, { x: 400, y: 1100 }),
    fence('right', 0, { x: 475, y: 100 }, { x: 475, y: 1100 }),
    fence('zero length', 3, { x: 800, y: 700 }, { x: 800, y: 700 }),
  ]),
]
const cases = []
for (const input of inputs) {
  for (const clearance of [25, 50]) {
    const world = collision.createBoneyardCollisionWorldAllPairsOracle(input)
    const prepared = navigation.buildPreparedBoneyardNavigationMesh(input.bounds, world, clearance)
    const triangles = prepared.mesh.triangles
    assert(triangles.length > 0)
    const queries = [
      { start: triangles[0].center, end: triangles.at(-1).center, bodyRadius: 25 },
      { start: { x: input.spawn.x, y: input.spawn.y }, end: triangles[Math.floor(triangles.length / 2)].center, bodyRadius: 25 },
      { start: { x: 437.5, y: 175 }, end: { x: 437.5, y: 1025 }, bodyRadius: 8, endBodyRadius: 0 },
      { start: { x: input.bounds.x - 1, y: input.bounds.y - 1 }, end: { x: input.spawn.x, y: input.spawn.y }, bodyRadius: 25 },
    ].map(query => ({
      ...query,
      route: navigation.findBoneyardEnemyRoute({ ...query, bounds: input.bounds, clearance, world }),
    }))
    cases.push({ scene: input, clearance, mesh: prepared.mesh, queries })
  }
}

// Mask 0x80 removes a primitive from construction, while the live route query
// still consults the complete collision world. Keep each shape lane observable.
const maskScene = scene('native construction exclusions')
const maskWorld = {
  circles: [{ center: { x: 430, y: 390 }, radius: 80, nativeLineMask: 0x80 }],
  polygons: [{ points: [{ x: 600, y: 300 }, { x: 800, y: 350 }, { x: 700, y: 550 }], nativeLineMask: 0x80 }],
  segments: [{ start: { x: 400, y: 800 }, end: { x: 800, y: 800 }, radius: 15, nativeLineMask: 0x80 }],
}
for (const clearance of [25, 50]) {
  const prepared = navigation.buildPreparedBoneyardNavigationMesh(maskScene.bounds, maskWorld, clearance)
  cases.push({ scene: maskScene, world: maskWorld, clearance, mesh: prepared.mesh, queries: [] })
}

const gateScene = scene('first query moving gate cache', [], [
  fence('gate', 2, { x: 600, y: 200 }, { x: 600, y: 1000 }),
])
const staticWorld = collision.createBoneyardCollisionWorldAllPairsOracle(gateScene)
const seed = '00112233445566778899aabbccddeeff'
let leaves = gates.createBoneyardGateLeaves(gateScene.fences, seed)
const firstWorld = collision.withBoneyardGateCollision(staticWorld, leaves)
const clearance = 25
const directQuery = { start: { x: 100, y: 100 }, end: { x: 400, y: 100 }, bodyRadius: 25 }
const blockedQuery = { start: { x: 300, y: 350 }, end: { x: 900, y: 350 }, bodyRadius: 25 }
const find = (world, query) => navigation.findBoneyardEnemyRoute({
  ...query, bounds: gateScene.bounds, clearance, world,
})
const prepared = world => navigation.boneyardNavigationMeshIsPrepared(gateScene.bounds, world, clearance)
assert.equal(prepared(firstWorld), false)
const directRoute = find(firstWorld, directQuery)
assert.equal(prepared(firstWorld), false)
const firstRoute = find(firstWorld, blockedQuery)
assert.equal(prepared(firstWorld), true)
const firstMesh = navigation.buildPreparedBoneyardNavigationMesh(gateScene.bounds, firstWorld, clearance).mesh
const contact = { x: 1, y: 0 }
for (let tick = 0; tick < 100; tick += 1) {
  leaves = leaves.map(leaf => gates.stepBoneyardGateLeaf(gates.applyBoneyardGateContact(leaf, contact)))
}
const movedWorld = collision.withBoneyardGateCollision(staticWorld, leaves)
assert.equal(prepared(movedWorld), true)
const retainedRoute = find(movedWorld, blockedQuery)
// Collision geometry identity is the immutable scene object, not the wrapper.
// A new accepted scene has its own identity even when its contents are equal.
const freshStaticWorld = collision.createBoneyardCollisionWorldAllPairsOracle(structuredClone(gateScene))
const freshWorld = collision.withBoneyardGateCollision(freshStaticWorld, leaves)
assert.equal(prepared(freshWorld), false)
const freshRoute = find(freshWorld, blockedQuery)
assert.equal(prepared(freshWorld), true)
const movedMesh = navigation.buildPreparedBoneyardNavigationMesh(gateScene.bounds, freshWorld, clearance).mesh
assert.notDeepEqual(firstMesh, movedMesh)

const preparations = []
for (const tutorial of [false, true]) {
  for (const source of ['default', 'mod']) {
    const template = tutorial ? STOCK_TUTORIAL_BONEYARD : NATIVE_GENERATED_BONEYARDS[0]
    const choiceId = tutorial ? 'stock-tutorial' : 'default-random'
    const world = construction.createBoneyardWorld({
      ...template, choice: { id: choiceId, source }, seed, runId: 'source preparations',
    })
    preparations.push({ tutorial, source, choiceId,
      expected: construction.boneyardWorldNavigationPreparations(world) })
  }
}

// Catalog materialization preserves the scene object, including lazy meshes built
// in a previous run. Equal bytes decoded into a new scene do not share that owner.
const stockCatalog = catalogKernel.createBoneyardCatalog()
const stockRun = catalogKernel.materializeBoneyard(stockCatalog, 'default-random', Buffer.alloc(16))
const stockWorld = construction.createBoneyardWorld(stockRun)
const stockBounds = stockRun.scene.bounds
const isPrepared = (bounds, world) => navigation.boneyardNavigationMeshIsPrepared(bounds, world.collision, 25)
const catalogIdentity = { stockBefore: isPrepared(stockBounds, stockWorld) }
navigation.prepareBoneyardNavigationMesh(stockBounds, stockWorld.collision, 25)
const repeatedStock = catalogKernel.materializeBoneyard(catalogKernel.createBoneyardCatalog(), 'default-random', Buffer.alloc(16))
catalogIdentity.stockSameScene = stockRun.scene === repeatedStock.scene
catalogIdentity.stockRepeated = isPrepared(stockBounds, construction.createBoneyardWorld(repeatedStock))
catalogIdentity.stockDecoded = isPrepared(stockBounds, construction.createBoneyardWorld(structuredClone(stockRun)))

const modEntry = {
  choice: { id: 'mod:fixture:gate', name: gateScene.name, source: 'mod', modId: 'fixture', modName: 'Fixture' },
  scene: structuredClone(gateScene), sourceSha256: 'fixture source', geometrySha256: 'fixture geometry',
}
const modCatalog = catalogKernel.createBoneyardCatalog([modEntry])
const modRun = catalogKernel.materializeBoneyard(modCatalog, modEntry.choice.id, Buffer.alloc(16))
const modWorld = construction.createBoneyardWorld(modRun)
catalogIdentity.modBefore = isPrepared(modRun.scene.bounds, modWorld)
navigation.prepareBoneyardNavigationMesh(modRun.scene.bounds, modWorld.collision, 25)
const repeatedMod = catalogKernel.materializeBoneyard(modCatalog, modEntry.choice.id, Buffer.alloc(16))
catalogIdentity.modSameScene = modRun.scene === repeatedMod.scene
catalogIdentity.modRepeated = isPrepared(modRun.scene.bounds, construction.createBoneyardWorld(repeatedMod))
const replacedCatalog = catalogKernel.createBoneyardCatalog([structuredClone(modEntry)])
const replacedMod = catalogKernel.materializeBoneyard(replacedCatalog, modEntry.choice.id, Buffer.alloc(16))
catalogIdentity.modReplaced = isPrepared(modRun.scene.bounds, construction.createBoneyardWorld(replacedMod))

writeFixture('navigation-build-goldens.json', 'tools/website-oracle/navigation-build-goldens.mjs',
  'Website/frontend/src/game/core-server/boneyard-enemy-navigation.ts and boneyard-collision.ts', {
    cases, preparations, catalogIdentity,
    cache: { scene: gateScene, seed, contact, ticks: 100, clearance,
      firstWorld, movedWorld, firstMesh, movedMesh,
      directQuery, directRoute, blockedQuery, firstRoute, retainedRoute, freshRoute },
  })
