// Actor-grid contact ordering, cone/polygon queries and Ether steering/fan boundaries.
import { loadKernel, point, writeFixture } from './oracle.mjs'
const targeting = await loadKernel('core-kernels/primary-spell-targeting.ts')
const target = (id, x, y, fields = {}) => ({ id, position: { x, y }, active: true,
  actorFlags: 2, attachment: { x: 1, y: -2 }, bodyRadius: 15, cellBindingOrder: 3,
  nativePriority: 2, pendingRemove: false, queryLane: 'grid', registrationOrder: 2,
  ...fields })
const targets = [
  target('near', 0, 80),
  target('priority', 10, 150, { nativePriority: 1, cellBindingOrder: 4, registrationOrder: 1 }),
  target('blocking', 0, 100, { actorFlags: 4, bodyRadius: 30, cellBindingOrder: 1, registrationOrder: 0, kind: 'scenery' }),
  target('legacy', -5, 65, { actorFlags: 0x182, queryLane: 'transient', registrationOrder: 8 }),
  target('dead', 0, 30, { active: false, nativePriority: 0, registrationOrder: 3 }),
  target('rebound', 300, 300, { gridPosition: { x: 0, y: 0 }, registrationOrder: 4 }),
  target('removing', 0, 81, { pendingRemove: true, cellBindingOrder: 0, nativePriority: 0, registrationOrder: 7 }),
]
const id = value => value?.id ?? null
const ids = values => values.map(value => value.id)
const points = []
for (const position of [{ x: 0, y: 99 }, { x: 0, y: 100 }, { x: -0.1, y: 80 }, { x: 0, y: 100.01 }, { x: 0, y: 109 }]) {
  const query = { actorMask: 6, position, queryRadius: 6, targets }
  points.push({ position: point(position), point: id(targeting.firstNativePrimaryPointContact(query)),
    fire: id(targeting.firstNativeFireballPointContact({ ...query, direction: { x: 0, y: 1 }, hostileCorridorLength: 400 })) })
}
const cones = []
const air = []
const roots = []
for (const reach of [75, 150, 205, 500]) {
  const origin = { x: 0, y: 0 }
  const hasLineOfSight = candidate => candidate.id !== 'priority'
  cones.push({ reach, ids: ids(targeting.nativePrimaryConeTargets({ actorMask: 6, aimDirection: { x: 0, y: 1 }, halfAngleDegrees: 15, hasLineOfSight, origin, reach, targets })) })
  air.push({ reach, id: id(targeting.selectAirPrimaryTarget({ aimDirection: { x: 0, y: 1 }, hasLineOfSight, maxRange: reach, origin, previousTargetId: 'priority', targets })) })
  roots.push({ reach, ids: ids(targeting.nativePrimaryRootTargets(origin, reach, 6, targets)) })
}
const polygon = [{ x: -20, y: 50 }, { x: 20, y: 50 }, { x: 20, y: 170 }, { x: -20, y: 170 }]
const fan = []
for (const heading of [7.5, 359]) {
  for (const quantity of [1, 2, 3, 4, 5, 6]) {
    for (let index = 0; index < quantity; index += 1) {
      fan.push([heading, quantity, index, targeting.nativeMissileFanHeading(heading, quantity, index), targeting.nativeMissileFanTurnScale(index)])
    }
  }
}
const homing = []
const homeCases = [
  [0, 0.01, { x: 70, y: -80 }, 1, 3], [0, 1, { x: 70, y: -80 }, 1, 3],
  [0, 1.01, { x: 70, y: -80 }, 1, 3], [0, 9.99, { x: 70, y: -80 }, 1, 3],
  [0, 10, { x: 70, y: -80 }, 1, 3], [359, 0.01, null, 1, 2.4],
  [359, 0.01, { x: 0, y: -80 }, 1, 2.4], [180, 0.5, { x: 0, y: -80 }, 1, 3],
  [90, 0.5, { x: 70, y: -80 }, 0, 3],
]
for (const [headingDegrees, turnAccumulator, targetPosition, movementScalar, speed] of homeCases) {
  const input = { headingDegrees, movementScalar, position: { x: 0, y: 0 }, speed, targetPosition, turnAccumulator, turnInput: 2 }
  const result = targeting.advanceEtherPrimaryHoming(input)
  homing.push({ input, result })
}
const tracking = []
for (const [targetId, reacquiresTarget] of [['near', true], ['dead', true], ['missing', true], ['missing', false], [null, true]]) {
  const input = { headingDegrees: 180, movementScalar: 1, position: { x: 0, y: 0 }, speed: 3, targetId, reacquiresTarget, targets, turnAccumulator: 0.01, turnInput: 2 }
  tracking.push({ targetId, reacquiresTarget, result: targeting.advanceEtherPrimaryTracking(input) })
}
writeFixture('spell-targeting-goldens.json', 'tools/website-oracle/spell-targeting-goldens.mjs',
  'Website/frontend/src/game/core-kernels/primary-spell-targeting.ts', {
    targets: targets.map(value => ({ ...value, kind: value.kind ?? 'enemy' })), points, cones, air, roots,
    polygon: { vertices: polygon.map(point), ids: ids(targeting.nativePrimaryPolygonTargets({ actorMask: 6, polygon, targets })) },
    ether: id(targeting.selectEtherPrimaryTarget({ origin: { x: 0, y: 0 }, aimDirection: { x: 0, y: 1 }, targets })),
    homing, tracking, fan,
  })
