#!/usr/bin/env node
// The complete ordered solver, including recursive pushes and non-pushing root moves.
import { loadKernel, writeFixture } from './oracle.mjs'

const { resolveActorMotion, resolveUnpushedMoverMotion } = await loadKernel('core-kernels/actor-physics.ts')
const collision = await loadKernel('core-server/boneyard-collision.ts')
const bounds = { x: 0, y: 0, w: 1200, h: 1200 }
const scene = objects => ({ name: 'actor physics', environmentMode: 0, bounds,
  spawn: { x: 200, y: 200, facingDeg: 180 }, objects,
  sprites: [], roads: [], fences: [], terrain: [], solomonDig: null })
const scenes = [scene([]), scene([{ eid: 'tree', typeId: 2001, variant: 1, pos: { x: 490, y: 500 } }])]
const cases = []
const unpushed = []
const body = (id, x, y, radius, resistance, strength, driven = false, pushEnabled = true) => ({
  id, position: { x, y }, delta: { x: 0, y: 0 }, radius,
  pushResistance: resistance, pushStrength: strength, driven, pushEnabled,
})

for (const [sceneIndex, input] of scenes.entries()) {
  const collisionWorld = collision.createBoneyardCollisionWorldAllPairsOracle(input)
  const world = {
    canPlace: (_id, position, radius) => collision.canPlaceBoneyardBody(position, bounds, collisionWorld, radius),
    move: (_id, position, delta, radius) => collision.resolveBoneyardMovement(position,
      { x: position.x + delta.x, y: position.y + delta.y }, bounds, collisionWorld, radius),
  }
  for (const strength of [0, 6, 12]) {
    for (const resistance of [0, 4, 90]) {
      for (const pushEnabled of [false, true]) {
        for (const delta of [{ x: 0, y: 0 }, { x: 32, y: 0 }, { x: 12, y: -19 }]) {
          const bodies = [
            { ...body('player', 400, 500, 25, 10, strength, true, pushEnabled), delta },
            body('enemy-a', 435, 500, 25, resistance, 0),
            body('enemy-b', 465, 505, 25, 1, 0),
            body('lantern', 440, 525, 8, 1, 0),
            body('fixed', 500, 535, 30, 90, 0),
            body('ghost', 405, 495, 25, 0, 0),
          ]
          const ticks = []
          let state = bodies
          for (let tick = 0; tick < 4; tick++) {
            const contacts = []
            state = resolveActorMotion(state, world, (a, b) => a.id !== 'ghost' && b.id !== 'ghost',
              undefined, (moverId, otherId) => contacts.push([moverId, otherId]))
            ticks.push({ positions: state.map(body => body.position), contacts })
          }
          cases.push({ sceneIndex, bodies, ticks })
          // This path deliberately collides with every body, as its source contract requires.
          const moverIndex = 1
          const crowd = bodies.map((body, index) => ({ ...body, driven: index === moverIndex,
            delta: index === moverIndex ? delta : { x: 0, y: 0 }, pushEnabled: index === moverIndex ? false : body.pushEnabled }))
          unpushed.push({ sceneIndex, bodies: crowd, moverIndex, delta,
            expected: resolveUnpushedMoverMotion(crowd, moverIndex, delta, world) })
        }
      }
    }
  }
}
// Coincident centers have no separation normal and remain unchanged. A boundary can also
// reject a whole correction, without projecting that correction along the world wall.
for (const position of [{ x: 25, y: 25 }, { x: 400, y: 400 }]) {
  const bodies = [body('player', position.x, position.y, 25, 10, 12, true),
    body('fixed', position.x, position.y, 25, 90, 0)]
  const world = { canPlace: () => true,
    move: (_id, point, delta) => ({ x: point.x + delta.x, y: point.y + delta.y }) }
  cases.push({ sceneIndex: 0, bodies, ticks: [{ positions: resolveActorMotion(bodies, world, () => true).map(body => body.position), contacts: [] }] })
}
writeFixture('actor-physics-goldens.json', 'tools/website-oracle/actor-physics-goldens.mjs',
  'Website/frontend/src/game/core-kernels/actor-physics.ts with source Boneyard collision callbacks',
  { scenes, cases, unpushed })
