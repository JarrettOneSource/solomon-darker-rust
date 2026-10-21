#!/usr/bin/env node
// Private-room tables, collision lattice, ordinary walks and complete doorway lifecycles.
import { loadKernel, point, writeFixture } from './oracle.mjs'

const layout = await loadKernel('core-kernels/hub-private-room-layout.ts')
const regions = await loadKernel('core-kernels/hub-regions.ts')
const movement = await loadKernel('core-kernels/hub-participant-movement.ts')
const player = await loadKernel('core-kernels/player-character.ts')
const physics = await loadKernel('core-kernels/actor-physics.ts')
const roomPresentation = await loadKernel('renderer/hub-private-room-presentation.ts')
const math = await loadKernel('core-kernels/hub-math.ts')
const seed = 0x51a7c011
const phase = { 'college-intro': 0, 'college-loadout': 1, outgoing: 2, incoming: 3 }
const roomIndex = (id) => regions.HUB_REGION_IDS.indexOf(id)
const segment = ({ x1, y1, x2, y2 }) => [x1, y1, x2, y2]
const rooms = layout.HUB_PRIVATE_ROOM_IDS.map((id) => {
  const room = layout.HUB_PRIVATE_ROOM_LAYOUTS[id]
  const grid = []
  for (let y = 128; y <= room.height; y += 128) {
    for (let x = 128; x <= room.width; x += 128) {
      const position = { x: x + 0.5, y: y + 0.5 }
      const clear = regions.isHubRegionTraversable(id, position)
      const moves = []
      if (clear) {
        for (const [dx, dy] of [[0,-40],[40,-40],[40,0],[40,40],[0,40],[-40,40],[-40,0],[-40,-40]]) {
          const moved = regions.moveWithHubRegionCollisionState(id, position, { x: dx, y: dy }, 25, seed)
          moves.push([dx, dy, moved.position.x, moved.position.y, moved.rngState])
        }
      }
      grid.push({ position: point(position), clear, moves })
    }
  }
  return {
    id,
    offset: point(room.architecture.collider.tableToWorldOffset),
    segments: room.architecture.collider.segments.map(segment),
    actors: Object.entries(room.actors).map(([id, actor]) => ({
      id, position: point(actor.collider.position), radius: actor.collider.radius,
      visualPosition: point(actor.visual.position), painterY: actor.visual.painterY,
    })),
    props: room.props.map((prop) => ({
      id: prop.id, painterId: prop.painterId,
      position: point(prop.collider.position), radius: prop.collider.radius,
      visual: prop.visual === null ? null : {
        kind: prop.visual.kind, frameIndex: prop.visual.frameIndex ?? 0,
        position: point(prop.visual.position ?? { x: 0, y: 0 }), painterY: prop.visual.painterY,
        marker: prop.visual.marker ?? false,
      },
    })),
    grid,
    flames: roomPresentation.HUB_PRIVATE_ROOM_FLAME_ANCHORS[id].map(point),
    flameFrames: [0, 1, 100, 9999].map((tick) => ({ tick, samples:
      roomPresentation.HUB_PRIVATE_ROOM_FLAME_ANCHORS[id].map((_, index) => {
        const frame = roomPresentation.hubRoomFlameTransform(id, tick, index)
        return [frame.rotation, frame.scaleX, frame.scaleY]
      }),
    })),
    cameras: [[0,0], [512,512], [2000,2000]].map(([x,y]) => ({
      position: [x,y], origin: point(math.hubRegionCameraOrigin(id, { x,y })),
    })),
  }
})

function runWalk(name, id, start, direction, ticks, initialTransition = null) {
  let participant = { collegeIntro: null, region: id, transition: initialTransition }
  let state = player.createPlayerCharacter({ discipline: 'arcane', displayName: 'oracle', element: 'ether' }, { x: start[0], y: start[1] })
  let rngState = seed
  const samples = []
  for (let tick = 0; tick < ticks; tick += 1) {
    const { plan: forced } = movement.planHubParticipantMovement(state, participant, false)
    const plan = forced ?? player.planPlayerCharacterTick(state, { movement: { x: direction[0], y: direction[1] } }, 1)
    const bypass = participant.transition !== null
    const resolved = physics.resolveActorMotion([
      { id: 'wizard', position: state.position, delta: plan.delta, ...player.PLAYER_CHARACTER_PHYSICS },
      ...movement.HUB_FIXED_ACTOR_COLLISION_LAYOUT.filter((body) => body.region === participant.region && body.region !== 'courtyard'),
    ], {
      canPlace: (_, position, radius) => bypass || regions.isHubRegionTraversable(participant.region, position, radius),
      move: (_, position, delta, radius) => {
        if (bypass) return { x: position.x + delta.x, y: position.y + delta.y }
        const moved = regions.moveWithHubRegionCollisionState(participant.region, position, delta, radius, rngState)
        rngState = moved.rngState
        return moved.position
      },
    }, (a,b) => a.id === 'wizard' || b.id === 'wizard')
    state = player.commitPlayerCharacterTick(state, plan, resolved[0].position)
    const stepped = movement.stepHubParticipantMovement(participant, state, null, {
      collegeIntroPending: false, collegeIntroWaiting: false,
    })
    state = stepped.player
    participant = stepped.participant
    const tr = participant.transition
    samples.push([
      state.position.x, state.position.y, state.velocity.x, state.velocity.y,
      state.headingIndex, state.gaitDegrees, state.walkCyclePrimary, rngState,
      roomIndex(participant.region), tr ? phase[tr.phase] : -1,
      tr?.alpha ?? 0, tr?.scriptedSpeed ?? 0,
      tr?.scriptedTarget.x ?? 0, tr?.scriptedTarget.y ?? 0,
    ])
  }
  return { name, region: id, start, direction, initialTransition, samples }
}

const walks = [
  runWalk('enter-mortuary', 'courtyard', [170,500], [-1,0], 360),
  runWalk('enter-library', 'courtyard', [1890,560], [1,0], 360),
  runWalk('enter-storeroom', 'courtyard', [627.5,210], [0,-1], 360),
  runWalk('enter-office', 'courtyard', [952.5,180], [0,-1], 360),
  ...layout.HUB_PRIVATE_ROOM_IDS.map((id) => {
    const target = regions.hubIncomingPlacement('courtyard', id).scriptedTarget
    return runWalk(`leave-${id}`, id, point(target), [0,1], 360)
  }),
  runWalk('mortuary-off-center-return', 'mortuary', [300,910], [0,1], 360),
  runWalk('mortuary-portrait-collision', 'mortuary', [512,850], [0,-1], 150),
  runWalk('office-desk-collision', 'office', [512,820], [0,-1], 150),
  runWalk('library-shelf-collision', 'library', [512,820], [-1,-1], 150),
  runWalk('storeroom-shelf-collision', 'storeroom', [537.5,650], [0,-1], 150),
  runWalk('intro-fade', 'courtyard', [950.64,164.04], [1,0], 120, {
    alpha: 1, destination: 'courtyard', phase: 'college-intro', scriptedSpeed: 1,
    scriptedTarget: { x: 950.64, y: 164.04 }, sourceRegion: 'courtyard',
  }),
]
const memoratorHeadings = [[628,770], [628,0], [0,770], [1024,770], [628,1024]]
for (let degrees = 0; degrees < 360; degrees += 11.25) {
  const radians = degrees * Math.PI / 180
  memoratorHeadings.push([628 + Math.sin(radians) * 100, 770 - Math.cos(radians) * 100])
}
writeFixture('hub-rooms-goldens.json', 'tools/website-oracle/hub-rooms-goldens.mjs',
  'Website/frontend/src/game: core-kernels/hub-private-room-layout.ts, hub-participant-movement.ts, actor-physics.ts, renderer/hub-private-room-presentation.ts',
  { rooms, walks, memoratorHeadings: memoratorHeadings.map(position =>
    [position, roomPresentation.hubMemoratorHeadingIndex({ x:position[0], y:position[1] })]) })
console.log(`rooms: ${rooms.length} layouts, ${rooms.reduce((n,r)=>n+r.grid.length,0)} grid points, ${walks.length} walks, ${walks.reduce((n,w)=>n+w.samples.length,0)} ticks`)
