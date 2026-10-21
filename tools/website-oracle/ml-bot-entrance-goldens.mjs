#!/usr/bin/env node
// Record the public navigator at Source-selected path points and lifecycle boundaries.
// The fixture supplies positions; it does not claim to advance an entire game journey.
import { loadKernel, writeFixture } from './oracle.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const { BoneyardEntranceNavigator } = await loadKernel('host/boneyard-entrance-navigation.ts')
const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const { STOCK_TUTORIAL_BONEYARD } = await loadKernel('host/stock-tutorial-boneyard.ts')
const cases = []
for (let index = 0; index <= NATIVE_GENERATED_BONEYARDS.length; index += 1) {
  const tutorial = index === NATIVE_GENERATED_BONEYARDS.length
  const template = tutorial ? STOCK_TUTORIAL_BONEYARD : NATIVE_GENERATED_BONEYARDS[index]
  const seed = `policy-entrance-${index}`
  const runId = `policy-${index}`
  let state = game.createGameSimulation({ local: {
    displayName: 'Policy Bot', element: 'fire', discipline: 'arcane',
  } })
  state = game.enterBoneyardWorld(state, { ...template, seed, runId,
    choice: { id: tutorial ? 'stock-tutorial' : 'default-random', source: 'default' },
  })
  const navigator = new BoneyardEntranceNavigator()
  const frames = []
  function capture(position, tick, phase = 'digging', runEventId = 0, playerId = 'local', hub = false) {
    const input = { position, tick, phase, runEventId, playerId, hub }
    state = { ...state, tick }
    state.playerEntities = entities.replacePlayerCharacter(state.playerEntities, 'local', {
      ...game.getPlayerCharacter(state, 'local'), position,
    })
    const savedWorld = state.world
    state.world = hub ? { kind: 'hub' } : { ...state.world,
      encounter: state.world.encounter === null ? null : { ...state.world.encounter, phase, runEventId },
    }
    try { frames.push({ input, output: navigator.input(state, playerId) }) }
    catch (error) { frames.push({ input, error: error.message }) }
    state.world = savedWorld
  }
  const spawn = game.getPlayerCharacter(state, 'local').position
  capture(spawn, 0)
  // Sampling the real Source route makes subsequent inputs distinguish waypoint
  // skipping, route simplification and the 100-tick replan boundary.
  const route = navigator.route
  for (let point = 1; point < route.length; point += 1) {
    const previous = route[point - 1]
    const next = route[point]
    capture({ x: (previous.x + next.x) / 2, y: (previous.y + next.y) / 2 }, point * 10)
    capture(next, point * 10 + 1)
  }
  capture(spawn, 99)
  capture(spawn, 100)
  const groups = new Map()
  for (const leaf of state.world.gateLeaves) {
    groups.set(leaf.fenceEid, [...(groups.get(leaf.fenceEid) ?? []), leaf])
  }
  const centers = [...groups.values()].filter(leaves => leaves.length === 2).map(leaves => ({
    x: (leaves[0].hinge.x + leaves[1].hinge.x) / 2,
    y: (leaves[0].hinge.y + leaves[1].hinge.y) / 2,
  })).sort((left, right) => Math.hypot(left.x - spawn.x, left.y - spawn.y)
    - Math.hypot(right.x - spawn.x, right.y - spawn.y))
  const direction = state.world.arenaTransition?.entrySide === 'south' ? -1 : 1
  const gate = centers[0]
  if (gate) {
    for (const [offsetX, offsetY] of [[40, -70], [40.0001, -70], [0, -60], [0, 60], [0, 60.0001], [0, 61]]) {
      capture({ x: gate.x + offsetX, y: gate.y + direction * offsetY }, 110 + frames.length)
    }
    const crossed = { x: gate.x, y: gate.y + direction * 61 }
    capture(crossed, 300)
    const solomonRoute = navigator.route
    for (let point = 1; point < solomonRoute.length; point += 1) {
      capture(solomonRoute[point], 300 + point * 10)
    }
  }
  if (state.world.encounter) {
    capture(state.world.encounter.position, 500)
    capture(spawn, 501, 'speaking')
    capture(spawn, 502, 'digging', 1)
    capture(spawn, 503)
    capture(spawn, 504, 'digging', 0, 'missing')
  }
  capture(spawn, 505, 'digging', 0, 'local', true)
  capture(spawn, 506)
  cases.push({ index, tutorial, seed, runId, frames })
}
writeFixture('ml-bot-entrance-goldens.json', 'tools/website-oracle/ml-bot-entrance-goldens.mjs',
  'Website host/boneyard-entrance-navigation.ts', { cases })
console.log(JSON.stringify({ cases: cases.length, frames: cases.reduce((sum, row) => sum + row.frames.length, 0) }))
