// Owned session construction and world entry/return through the actual public Game API.
import { loadKernel, writeFixture } from './oracle.mjs'
import { playerStateSnapshot } from './player-state-snapshot.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const { STOCK_TUTORIAL_BONEYARD } = await loadKernel('host/stock-tutorial-boneyard.ts')

function tutorialProfile(economy) {
  if (economy === null) return null
  return {
    gold: economy.gold, revision: economy.revision, collegeIntroPending: economy.collegeIntroPending,
    inventory: economy.backpack.map(item => [item.id, item.nativeTypeId, item.quantity]),
    equippedTints: [economy.equipment.hat?.iconTints ?? null, economy.equipment.robe?.iconTints ?? null],
    rng: economy.rng,
  }
}

function snapshot(state) {
  const character = game.getPlayerCharacter(state, 'local')
  const world = state.world
  return {
    tick: state.tick,
    player: playerStateSnapshot(state),
    character,
    lighting: state.playerEntities.lightings[0],
    combatRng: state.combatRng,
    secondary: {
      actors: state.secondaryAbilities.actors,
      nextActorId: state.secondaryAbilities.nextActorId,
      nextEventId: state.secondaryAbilities.nextEventId,
      players: state.secondaryAbilities.players,
      rng: state.secondaryAbilities.rng,
    },
    run: state.run,
    order: state.worldManagerOrder,
    world: world.kind === 'hub'
      ? { kind: world.kind, traderAnimationSeed: world.traderAnimationSeed,
          collisionRngState: world.collisionRngState, participant: world.participants.local,
          studentIds: world.studentPopulation.students.map(student => student.id),
          studentRegistrations: world.studentPopulation.students.map(student => student.painterRegistration.registrationOrdinal) }
      : { kind: world.kind, runId: world.runId,
          lanternLightRegistration: world.lanternLightRegistration,
          solomonPainterRegistration: world.solomonPainterRegistration,
          tutorial: world.tutorial,
          tutorialProfileEconomy: tutorialProfile(world.tutorialProfileEconomy),
          score: world.hallOfFameRuns.local },
  }
}

const cases = []
for (const [seed, element, discipline] of [[17, 'ether', 'arcane'], [73, 'fire', 'body']]) {
  const config = { displayName: 'Oracle', element, discipline }
  let state = game.createGameSimulation({ local: config }, { gameRngSeed: seed, combatRngSeed: 91 })
  const steps = [{ action: 'create', expected: snapshot(state) }]
  for (const choice of ['default-random', 'stock-tutorial']) {
    const loaded = { ...(choice === 'stock-tutorial' ? STOCK_TUTORIAL_BONEYARD : NATIVE_GENERATED_BONEYARDS[0]),
      choice: { id: choice, source: 'default' }, runId: `session-${seed}-${choice}`, seed: `session-${seed}-${choice}` }
    state = game.enterBoneyardWorld(state, loaded)
    steps.push({ action: 'enter', choice, expected: snapshot(state) })
    state = game.returnGameSimulationToHub(state)
    steps.push({ action: 'return', expected: snapshot(state) })
  }
  state = game.grantGameSimulationPlayerExperience(state, 'local', 95)
  const beforeRejected = snapshot(state)
  let rejection = null
  try {
    game.enterBoneyardWorld(state, { ...NATIVE_GENERATED_BONEYARDS[0],
      choice: { id: 'default-random', source: 'default' }, runId: 'rejected', seed: 'rejected' })
  } catch (error) { rejection = error.message }
  if (rejection !== 'cannot enter a Boneyard during a level-up barrier') throw new Error(`Unexpected admission: ${rejection}`)
  steps.push({ action: 'pending-offer', experience: 95, rejected: true, expected: beforeRejected })
  cases.push({ seed, config, combatSeed: 91, steps })
}
writeFixture('session-lifecycle-goldens.json', 'tools/website-oracle/session-lifecycle-goldens.mjs',
  'Website/frontend/src/game/core-server/game-simulation.ts public create, enterBoneyardWorld and returnGameSimulationToHub', { cases })
