#!/usr/bin/env node
// Actual encounter entry into escape, then the world's route and collision phases.
import { loadKernel, writeFixture } from './oracle.mjs'

const encounter = await loadKernel('core-kernels/boneyard-encounter.ts')
const placement = await loadKernel('core-server/boneyard-world-placement.ts')
const navigation = await loadKernel('core-server/boneyard-enemy-navigation.ts')
const collision = await loadKernel('core-server/boneyard-collision.ts')
const gates = await loadKernel('core-kernels/boneyard-gate.ts')
const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const { STOCK_TUTORIAL_BONEYARD } = await loadKernel('host/stock-tutorial-boneyard.ts')
const { nativeTutorialDialogueTicks } = await loadKernel('core-kernels/native-tutorial.ts')

const snapshot = state => ({ position: state.position, headingDeg: state.headingDeg,
  escapeTarget: state.escapeTarget, escapeCollisionSourceIds: state.escapeCollisionSourceIds,
  phase: state.phase, lifetimeTicksRemaining: state.lifetimeTicksRemaining,
  acceleration: state.acceleration, escapeSpeed: state.escapeSpeed, transitionOffsetY: state.transitionOffsetY,
  rngState: state.rngState, voiceTicksRemaining: state.voiceTicksRemaining, voiceEvents: state.voiceEvents })
const cases = []
for (const [templateIndex, template] of [...NATIVE_GENERATED_BONEYARDS, STOCK_TUTORIAL_BONEYARD].entries()) {
  const scene = template.scene
  const seed = `escape-${templateIndex}`
  const world = collision.createBoneyardCollisionWorldAllPairsOracle(scene)
  navigation.prepareBoneyardNavigationMesh(placement.solomonEscapeTraversalBounds(scene.bounds), world, 25)
  const tutorial = templateIndex === NATIVE_GENERATED_BONEYARDS.length
  const options = tutorial ? { dialogueMode: 'tutorial', tutorialDialogueTicks: nativeTutorialDialogueTicks() } : undefined
  let state = encounter.createSolomonEncounter(scene.solomonDig, seed, options)
  const players = { local: { position: { x: scene.solomonDig.position.x + 30, y: scene.solomonDig.position.y + 50 } } }
  let tick = 0
  let leaves = gates.createBoneyardGateLeaves(scene.fences, seed)
  while (state.phase !== 'escaping' && tick < 6000) {
    leaves = leaves.map(gates.stepBoneyardGateLeaf)
    state = encounter.stepSolomonEncounter(state, players, ++tick)
  }
  if (state.phase !== 'escaping') throw new Error('oracle encounter never reached escape')
  const initialTick = tick
  const records = [{ tick, state: snapshot(state) }]
  while (state.phase !== 'gone' && tick < initialTick + 600) {
    leaves = leaves.map(gates.stepBoneyardGateLeaf)
    const dynamic = collision.withBoneyardGateCollision(world, leaves)
    const source = placement.prepareSolomonEscapeNavigation(state, scene.bounds, dynamic, world)
    const next = encounter.stepSolomonEncounter(source, players, ++tick)
    state = placement.resolveSolomonEscapeMovement(source, next, scene.bounds, dynamic)
    records.push({ tick, state: snapshot(state) })
  }
  cases.push({ templateIndex, seed, tutorialDialogueTicks: options?.tutorialDialogueTicks ?? 0,
    initialTick, players, records })
}
writeFixture('solomon-escape-goldens.json', 'tools/website-oracle/solomon-escape-goldens.mjs',
  'Website/frontend/src/game/core-server/boneyard-world-placement.ts::prepareSolomonEscapeNavigation and resolveSolomonEscapeMovement',
  { cases })
