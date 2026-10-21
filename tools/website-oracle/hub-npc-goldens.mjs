// Seeded Courtyard population, actor contacts, trader clocks and stock NPC presentation.
import { loadKernel, writeFixture } from './oracle.mjs'

const students = await loadKernel('core-server/hub-students.ts')
const splines = await loadKernel('core-server/hub-student-splines.ts')
const presentation = await loadKernel('hub-presentation.ts')
const worldKernel = await loadKernel('core-server/hub-world.ts')
const players = await loadKernel('core-kernels/player-character.ts')
const skorchaKernel = await loadKernel('core-server/hub-skorcha.ts')

const registration = () => {
  const ordinal = { actor: 49, scenery: 0, transient: 0 }
  return managerLane => ({ managerLane, registrationOrdinal: ordinal[managerLane]++ })
}
const populationSnapshot = population => ({
  nextId: population.nextId,
  rarePathDenominator: population.rarePathDenominator,
  rngState: population.rngState,
  spawnRequestPending: population.spawnRequestPending,
  spawnTickerCounter: population.spawnTickerCounter,
  students: population.students,
})
const splineSamples = splines.HUB_STUDENT_SPLINES.map(spline => ({
  pathId: spline.id,
  samples: [-1, 0, 0.01, 0.1, 0.5, 1, 1.25, spline.points.length - 1.01, spline.points.length]
    .map(cursor => ({ cursor, position: splines.evaluateHubStudentSpline(spline.id, cursor) })),
}))
const spawnCases = [0x51d07e57, 0, 1].flatMap(seed => splines.HUB_STUDENT_SPLINES.map(spline => (
  { seed, student: students.createHubStudentState(spline.id, spline.id, seed, 0.75) }
)))
const freeRegister = registration()
const population = students.createHubStudentPopulation(freeRegister)
const warmPopulation = populationSnapshot(population)
const populationTicks = []
for (let tick = 0; tick < 3000; tick += 1) {
  const current = population.students
  const committed = current.map(student => {
    const plan = students.planHubStudentRoute(student, current, 0.01)
    return students.commitHubStudentRoute(plan.state, {
      x: plan.state.position.x + plan.delta.x,
      y: plan.state.position.y + plan.delta.y,
    })
  })
  students.stepHubStudentPopulation(population, committed, freeRegister)
  if ((tick + 1) % 25 === 0) populationTicks.push({ tick: tick + 1, ...populationSnapshot(population) })
}

const worldRegister = registration()
let world = worldKernel.createHubWorld(['oracle'], {
  registerWorldPainter: worldRegister,
  skorcha: null,
  skorchaHiddenTicks: 60000,
})
let player = players.createPlayerCharacter(
  { discipline: 'arcane', displayName: 'oracle', element: 'ether' },
  { x: 950.64, y: 164.04 },
)
const worldTicks = []
for (let tick = 0; tick < 2400; tick += 1) {
  const movement = tick < 200 ? { x: 0, y: 1 }
    : tick < 550 ? { x: 1, y: 0 }
      : tick < 900 ? { x: 0, y: 1 }
        : tick < 1450 ? { x: -1, y: 0 }
          : tick < 1800 ? { x: 0, y: -1 } : { x: 0, y: 0 }
  const stepped = worldKernel.stepHubWorldTick(world, { oracle: player },
    { oracle: { movement } }, { oracle: 1 }, null, null, worldRegister)
  world = stepped.world
  player = stepped.players.oracle
  if ((tick + 1) % 20 === 0) worldTicks.push({
    tick: tick + 1, movement,
    player: { position: player.position, velocity: player.velocity,
      headingIndex: player.headingIndex, gaitDegrees: player.gaitDegrees,
      walkCyclePrimary: player.walkCyclePrimary },
    collisionRngState: world.collisionRngState,
    population: populationSnapshot(world.studentPopulation),
  })
}

const traderSeeds = [0x5eedc0de, 1]
const traderCases = traderSeeds.map(seed => {
  const common = presentation.createHubCommonTraderClock(seed ^ 5005, 0)
  const hagatha = presentation.createHubHagathaClock(seed ^ 5001, 0)
  const potion = presentation.createHubPotionTraderClock(0)
  let skorcha = skorchaKernel.createHubSkorchaSchedule(seed, { hiddenTicks: 70, visibleTicks: 90 })
  const ticks = []
  for (let tick = 0; tick <= 3000; tick += 1) {
    if (tick > 0) skorcha = skorchaKernel.stepHubSkorchaSchedule(skorcha, { hiddenTicks: 70, visibleTicks: 90 })
    const hagathaFrame = hagatha.advanceTo(tick)
    ticks.push({ tick, common: common.advanceTo(tick), potion: potion.advanceTo(tick),
      hagatha: { bodyFrame: hagathaFrame.bodyFrame,
        particles: tick % 59 === 0 ? hagathaFrame.particles : null },
      skorcha: skorcha.skorcha === null ? null : {
        variant: skorcha.skorcha.variant, position: skorcha.skorcha.position,
        gesture: skorcha.skorcha.gesture, hatFrame: skorchaKernel.hubSkorchaHatFrame(skorcha.skorcha),
        gestureTicksRemaining: skorcha.skorcha.gestureTicksRemaining,
      }, transitionTicksRemaining: skorcha.transitionTicksRemaining,
    })
  }
  return { seed, ticks }
})
const offsets = warmPopulation.students.map(student => ({
  student,
  head: presentation.hubStudentHeadOffset(student),
  props: student.props.map((prop, index) => presentation.hubStudentPropOffset(student.heading, prop, index)),
}))
writeFixture('hub-npc-goldens.json', 'tools/website-oracle/hub-npc-goldens.mjs',
  'Website: core-server/hub-students.ts, hub-student-splines.ts, hub-world.ts, hub-skorcha.ts; native-natural-spline.ts; hub-presentation.ts',
  { splineSamples, spawnCases, warmPopulation, populationTicks, worldTicks, traderCases, offsets })
console.log(`3000 population ticks (${populationTicks.length} snapshots); 2400 actor-world ticks (${worldTicks.length} snapshots); ${traderCases.length * traderCases[0].ticks.length} trader ticks`)
