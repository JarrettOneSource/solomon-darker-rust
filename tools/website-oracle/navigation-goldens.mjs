#!/usr/bin/env node
// Exact immutable NavMeshes for the shipped native catalog, plus runtime route-query goldens.
import { mkdirSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { loadKernel, root, writeFixture } from './oracle.mjs'

const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const { STOCK_TUTORIAL_BONEYARD } = await loadKernel('host/stock-tutorial-boneyard.ts')
const collision = await loadKernel('core-server/boneyard-collision.ts')
const navigation = await loadKernel('core-server/boneyard-enemy-navigation.ts')
const placement = await loadKernel('core-server/boneyard-world-placement.ts')
const arena = await loadKernel('core-kernels/boneyard-arena-transition.ts')

const directory = resolve(root, 'web/assets/navigation')
mkdirSync(directory, { recursive: true })
const cases = []
const files = []
let totalBytes = 0
for (const [templateIndex, template] of [...NATIVE_GENERATED_BONEYARDS, STOCK_TUTORIAL_BONEYARD].entries()) {
  const scene = template.scene
  const world = collision.createBoneyardCollisionWorldAllPairsOracle(scene)
  const requests = [{ bounds: scene.bounds, clearance: 25 }, { bounds: scene.bounds, clearance: 50 }]
  if (templateIndex < NATIVE_GENERATED_BONEYARDS.length && scene.solomonDig !== null) {
    const transition = arena.createBoneyardArenaTransition(scene.bounds, scene.spawn)
    requests.push({ bounds: transition.combatBounds, clearance: 25 }, { bounds: transition.combatBounds, clearance: 50 })
  }
  if (scene.solomonDig !== null) {
    requests.push({ bounds: placement.solomonEscapeTraversalBounds(scene.bounds), clearance: 25 })
  }
  const meshes = []
  for (const { bounds, clearance } of requests) {
    const prepared = navigation.buildPreparedBoneyardNavigationMesh(bounds, world, clearance)
    navigation.installPreparedBoneyardNavigationMesh(world, prepared)
    meshes.push({ bounds, clearance, mesh: prepared.mesh })
    const triangles = prepared.mesh.triangles
    if (triangles.length === 0) throw new Error(`${scene.name} has an empty prepared NavMesh`)
    const first = triangles[0].center
    const middle = triangles[Math.floor(triangles.length / 2)].center
    const last = triangles.at(-1).center
    const queryInputs = [
      { start: first, end: last, bodyRadius: 25 },
      { start: middle, end: first, bodyRadius: 25 },
      { start: scene.spawn, end: last, bodyRadius: 25 },
      { start: first, end: first, bodyRadius: 25 },
      { start: { x: bounds.x - 1, y: bounds.y - 1 }, end: middle, bodyRadius: 25 },
      { start: first, end: last, bodyRadius: 8, endBodyRadius: 0 },
    ]
    if (scene.solomonDig !== null) {
      queryInputs.push({
        start: scene.solomonDig.position, end: scene.spawn, bodyRadius: 30, endBodyRadius: 25,
        ignoredSourceIds: collision.boneyardBodyCollisionSourceIds(scene.solomonDig.position, world, 30),
      })
    }
    const queries = queryInputs.map(input => {
      const route = navigation.findBoneyardEnemyRoute({
        ...input, bounds, clearance, world,
        ...(input.ignoredSourceIds ? { ignoredSourceIds: new Set(input.ignoredSourceIds) } : {}),
      })
      return { ...input, route }
    })
    cases.push({ templateIndex, sourceSha256: template.sourceSha256, geometrySha256: template.geometrySha256,
      bounds, clearance, queries })
  }
  const body = `${JSON.stringify({ sourceSha256: template.sourceSha256, geometrySha256: template.geometrySha256, meshes })}\n`
  const name = `${template.sourceSha256}.json`
  writeFileSync(resolve(directory, name), body)
  files.push({ name, meshes: meshes.length, bytes: Buffer.byteLength(body) })
  totalBytes += Buffer.byteLength(body)
  console.log(`prepared ${templateIndex + 1}/13: ${meshes.length} meshes, ${Buffer.byteLength(body)} bytes`)
}
writeFixture('navigation-goldens.json', 'tools/website-oracle/navigation-goldens.mjs',
  'Website/frontend/src/game/core-server/boneyard-enemy-navigation.ts and boneyard-world-placement.ts',
  { cases })
console.log(JSON.stringify({ files: files.length, meshes: cases.length, bytes: totalBytes }))
