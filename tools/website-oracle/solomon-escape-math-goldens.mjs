#!/usr/bin/env node
// Operand witness for the first independently observed template-nine heading difference.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, writeFixture } from './oracle.mjs'

const collision = await loadKernel('core-server/boneyard-collision.ts')
const navigation = await loadKernel('core-server/boneyard-enemy-navigation.ts')
const placement = await loadKernel('core-server/boneyard-world-placement.ts')
const encounter = await loadKernel('core-kernels/boneyard-encounter.ts')
const gates = await loadKernel('core-kernels/boneyard-gate.ts')
const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const fixture = JSON.parse(readFileSync(resolve(fixturesDir, 'solomon-escape-goldens.json'), 'utf8'))
const sourceCase = fixture.cases[9]
const scene = NATIVE_GENERATED_BONEYARDS[9].scene
const world = collision.createBoneyardCollisionWorldAllPairsOracle(scene)
const bounds = placement.solomonEscapeTraversalBounds(scene.bounds)
navigation.prepareBoneyardNavigationMesh(bounds, world, encounter.NATIVE_SOLOMON_NAVIGATION_CLEARANCE)
const cases = []
for (const tick of [940, 1094]) {
  const input = sourceCase.records.find(record => record.tick === tick - 1).state
  let leaves = gates.createBoneyardGateLeaves(scene.fences, sourceCase.seed)
  for (let index = 0; index < tick; index++) leaves = leaves.map(gates.stepBoneyardGateLeaf)
  const dynamic = collision.withBoneyardGateCollision(world, leaves)
  const pathTarget = encounter.nativeSolomonEscapePathTarget(input.position, input.escapeTarget, scene.bounds)
  const ignored = new Set(input.escapeCollisionSourceIds)
  const blocked = collision.firstBoneyardPathBlockProgress(input.position, pathTarget, bounds,
    dynamic, encounter.NATIVE_SOLOMON_COLLISION_RADIUS, ignored) !== null
  const route = blocked ? navigation.findBoneyardEnemyRoute({
    bodyRadius: encounter.NATIVE_SOLOMON_COLLISION_RADIUS, bounds,
    clearance: encounter.NATIVE_SOLOMON_NAVIGATION_CLEARANCE, end: pathTarget,
    ignoredSourceIds: ignored, start: input.position, world,
  }) : null
  let goal = pathTarget
  if (route !== null && route.length >= 2) {
    goal = route[1]
    for (const candidate of route.slice(2)) {
      if (collision.firstBoneyardLineObstruction(input.position, candidate, bounds,
        dynamic, undefined, 0, ignored) !== null) break
      goal = candidate
    }
  }
  const x = goal.x - input.position.x
  const y = goal.y - input.position.y
  const angle = Math.atan2(x, -y)
  const product = angle * 180
  const degrees = product / Math.PI
  const shifted = degrees + 360
  const heading = shifted % 360
  const prepared = placement.prepareSolomonEscapeNavigation(input, scene.bounds, dynamic, world)
  if (heading !== prepared.headingDeg) throw new Error('steering witness differs from actual source preparation')
  cases.push({ tick, position: input.position, goal, x, y, angle, product, degrees, shifted, heading })
}
writeFixture('solomon-escape-math-goldens.json', 'tools/website-oracle/solomon-escape-math-goldens.mjs',
  'Website core-server/boneyard-world-placement.ts and core-kernels/actor-heading.ts; exact first-divergence operands',
  { cases })
