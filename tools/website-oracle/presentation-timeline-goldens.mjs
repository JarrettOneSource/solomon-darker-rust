// The actual client timelines, using real protocol snapshots as their input.
import { loadKernel, writeFixture } from './oracle.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const { createGameSnapshot } = await loadKernel('host/game-snapshot.ts')
const { createHubPresentationTimeline } = await loadKernel('client/hub-presentation-timeline.ts')
const { createBoneyardPresentationTimeline } = await loadKernel('client/boneyard-presentation-timeline.ts')
const { predictPlayerCharacterInHub } = await loadKernel('client/hub-prediction.ts')
const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')

function initial(world) {
  let state = game.createGameSimulation({
    local: { displayName: 'Local', element: 'ether', discipline: 'arcane' },
    remote: { displayName: 'Remote', element: 'fire', discipline: 'body' },
  }, { gameRngSeed: 17, combatRngSeed: 91 })
  if (world === 'boneyard') state = game.enterBoneyardWorld(state, {
    ...NATIVE_GENERATED_BONEYARDS[0], choice: { id: 'default-random', source: 'default' },
    runId: 'presentation-oracle', seed: 'presentation-oracle',
  })
  return createGameSnapshot(state, 'local')
}

function endpoint(base, tick, shift = 0) {
  const result = structuredClone(base)
  result.tick = tick
  Object.entries(result.players).forEach(([id, player], index) => {
    player.position = { x: 100 + index * 200 + tick * 2 + shift, y: 300 + index * 100 + tick * 3 }
    player.velocity = { x: 7 + tick, y: -3 - tick }
    player.gaitDegrees = (350 + tick * 5) % 360
    player.headingIndex = (23 + Math.floor(tick / 5)) % 24
    player.walkCyclePrimary = (4.5 + tick * .4) % 5
    player.primaryCast.weaponPulse = 1 - tick * .02
    player.primaryCast.etherBlastCharge = tick * .125
    player.lighting.overlayEffectPhase = tick * .25
  })
  const registration = { managerLane: 'actor', registrationOrdinal: 77 }
  const common = { ownerId: 'local', worldKey: 'presentation', painterRegistrations: [registration] }
  result.primarySpells = { nextId: 104, projectiles: [], transients: [
    ...(tick >= 5 && tick < 10 ? [{ ...common, kind: 'fire', id: 101,
      ageTicks: tick - 5, lightRegistration: null, direction: { x: .6, y: -.8 },
      origin: { x: tick * 2, y: tick * 3 }, variant: 1 }] : []),
    { ...common, kind: 'fire-ember', id: 102, ageTicks: tick, lightRegistration: registration,
      burnDamage: 0, damage: 0, height: -tick * 1.5, horizontalVelocity: { x: tick * .2, y: -tick * .3 },
      life: 2 - tick * .01, phase: (3.8 + tick * .4) % 4,
      position: { x: tick * 3, y: tick * 4 }, verticalVelocity: tick * .1,
      contactCadence: 1, contactDue: false, spentEmber: { kind: 'none' } },
    { ...common, kind: 'water-hail', id: 103, ageTicks: tick, birthTick: 0,
      bounceProgress: 0, bounceSoundIndex: null, bounceSoundPitch: null, bounceSoundSequence: 0,
      height: -tick * .75, horizontalVelocity: { x: Math.fround(tick * .3), y: Math.fround(-tick * .4) }, life: .5,
      position: { x: tick * 5, y: tick * 6 }, rotationDegrees: 350 + tick * 4,
      rotationStepDegrees: 1, savedBounceVelocity: -2, scale: .5, verticalVelocity: Math.fround(tick * .2) },
  ] }
  return result
}

function row(frame) {
  return {
    tick: frame.tick,
    spells: frame.primarySpells.transients.map(effect => {
      if (effect.kind === 'fire') return [0, effect.id, effect.ageTicks, effect.origin.x, effect.origin.y]
      if (effect.kind === 'fire-ember') return [1, effect.id, effect.ageTicks, effect.position.x,
        effect.position.y, effect.height, effect.horizontalVelocity.x, effect.horizontalVelocity.y,
        effect.life, effect.phase, effect.verticalVelocity]
      if (effect.kind === 'water-hail') return [2, effect.id, effect.ageTicks, effect.position.x,
        effect.position.y, effect.height, effect.horizontalVelocity.x, effect.horizontalVelocity.y,
        effect.life, effect.rotationDegrees, effect.verticalVelocity]
      throw new Error(`unexpected fixture actor ${effect.kind}`)
    }),
    players: Object.entries(frame.players).map(([id, player]) => [id,
      player.position.x, player.position.y, player.velocity.x, player.velocity.y,
      player.gaitDegrees, player.headingIndex, player.walkCyclePrimary,
      player.primaryCast.weaponPulse, player.primaryCast.etherBlastCharge,
      player.lighting.overlayEffectPhase,
    ]),
  }
}

const operations = [
  { sample: 0 }, { sample: 25 },
  { push: 5, received: 50 }, { sample: 50 }, { sample: 66.66666666666667 },
  { sample: 83.33333333333333 }, { sample: 100 },
  { push: 10, received: 130 }, { sample: 130 }, { sample: 145 },
  { push: 10, received: 155, shift: 3 }, { sample: 160 },
  { push: 8, received: 165 }, { sample: 175 }, { sample: 500 },
  ...Array.from({ length: 10 }, (_, index) => ({ push: 15 + index * 5, received: 550 + index * 50 })),
  { sample: 1000 }, { sample: 1016.6666666666666 }, { sample: 1100 },
]
const cases = []
for (const world of ['hub', 'boneyard']) {
  const base = initial(world)
  const options = { initialSnapshot: endpoint(base, 0), initialReceivedAtMs: 0,
    serverTickRate: 100, snapshotRate: 20, localPlayerId: 'local' }
  const timeline = world === 'hub' ? createHubPresentationTimeline(options) : createBoneyardPresentationTimeline(options)
  const samples = []
  for (let index = 0; index < operations.length; index += 1) {
    const operation = operations[index]
    if ('push' in operation) timeline.push(endpoint(base, operation.push, operation.shift), operation.received)
    else samples.push({ operation: index, expected: row(timeline.sample(operation.sample)) })
  }
  cases.push({ world, samples })
}
const prediction = []
for (const movement of [{ x: 1, y: 0 }, { x: -.6, y: .8 }]) {
  const snapshot = endpoint(initial('hub'), 0)
  let player = snapshot.players.local
  let participant = snapshot.world.participants.local
  let collision = snapshot.world.collisionRngState
  const samples = []
  for (let tick = 0; tick <= 5; tick += 1) {
    samples.push([player.position.x,player.position.y,player.velocity.x,player.velocity.y,
      player.gaitDegrees,player.headingIndex,player.walkCyclePrimary,collision])
    const next = predictPlayerCharacterInHub(player, { movement }, collision,
      snapshot.players.local.movementScale, participant,
      { collegeIntroPending: player.economy.collegeIntroPending, collegeIntroWaiting: false })
    player = { ...player, ...next.player }
    participant = next.participant
    collision = next.collisionRngState
  }
  prediction.push({ movement, samples })
}
writeFixture('presentation-timeline-goldens.json' , 'tools/website-oracle/presentation-timeline-goldens.mjs',
  'Website/frontend/src/game/client/{hub,boneyard}-presentation-timeline.ts', { operations, cases, prediction })
