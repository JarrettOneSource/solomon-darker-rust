// The actual public Game tick, with controlled initial arenas and real native stores.
import { loadKernel, writeFixture } from './oracle.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const skills = await loadKernel('core-kernels/player-skill-runtime.ts')
const belt = await loadKernel('core-kernels/native-belt.ts')
const loot = await loadKernel('core-server/boneyard-loot-store.ts')
const { createNativeWorldManagerOrder } = await loadKernel('core-kernels/native-world-manager-order.ts')
const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const { STOCK_TUTORIAL_BONEYARD } = await loadKernel('host/stock-tutorial-boneyard.ts')

import { sessionTickSnapshot as snapshot } from './session-tick-rows.mjs'

const specs = [
  { name: 'college-intro', element: 'ether', discipline: 'arcane', hub: true, college: true, ticks: 2600 },
  { name: 'hub-walk', element: 'ether', discipline: 'arcane', hub: true, ticks: 250 },
  ...['air', 'earth', 'ether', 'fire', 'water'].map(element => ({
    name: `${element}-primary`, element, discipline: 'arcane', ticks: 360,
  })),
  { name: 'rewards-pickups', element: 'fire', discipline: 'arcane', ticks: 600, ranks: [[16, 10]], gold: 7 },
  { name: 'staff-contact', element: 'air', discipline: 'body', ticks: 350 },
  { name: 'secondary-acid-rain', element: 'water', discipline: 'arcane', ticks: 450,
    ranks: [[72, 1]], secondary: true },
  { name: 'death-return', element: 'ether', discipline: 'arcane', ticks: 545, lethal: true },
  { name: 'skill-barrier', element: 'fire', discipline: 'mind', ticks: 12, experience: 95 },
  { name: 'tutorial-entrance', element: 'ether', discipline: 'arcane', tutorial: true, ticks: 300 },
]
const cases = []
for (const spec of specs) {
  let state = game.createGameSimulation({ local: { displayName: 'Oracle', element: spec.element,
    discipline: spec.discipline } }, { gameRngSeed: 17, combatRngSeed: 91 })
  if (!spec.hub) {
    const template = spec.tutorial ? STOCK_TUTORIAL_BONEYARD : NATIVE_GENERATED_BONEYARDS[0]
    state = game.enterBoneyardWorld(state, { ...template,
      choice: { id: spec.tutorial ? 'stock-tutorial' : 'default-random', source: 'default' },
      seed: `session-${spec.name}`, runId: `session-${spec.name}` })
    if (!spec.tutorial) {
      // A retained, combat-ready arena is the input to these phase-order journeys.
      // All actors and damage below still run through public Game and native constructors.
      state.world = { ...state.world, encounter: null, waves: null, arenaTransition: null }
      state.playerEntities = entities.replacePlayerCharacter(state.playerEntities, 'local', {
        ...game.getPlayerCharacter(state, 'local'), position: { x: 1000, y: 1000 },
      })
    }
  }
  if (spec.ranks) {
    const store = state.playerEntities
    const book = structuredClone(store.skillBooks[0])
    for (const [id, rank] of spec.ranks) {
      book.permanentRanks[id] = rank
      if (!book.learnedSkillOrder.includes(id)) book.learnedSkillOrder.push(id)
    }
    const refreshed = skills.refreshPlayerSkillRuntime(store.skillRuntimes[0], book,
      store.statBooks[0], store.economies[0])
    state.playerEntities = { ...store, skillBooks: [refreshed.skillBook],
      skillRuntimes: [refreshed.runtime], belts: [belt.createNativePlayerBelt(refreshed.skillBook)] }
  }
  if (spec.secondary) {
    state.playerEntities = { ...state.playerEntities, belts: [belt.bindNativeBeltSkill(
      state.playerEntities.belts[0], state.playerEntities.skillBooks[0], 72, 2)] }
  }
  if (spec.gold) {
    // A real retained Gold actor exercises world pickup -> economy -> retained event
    // interleaving independently of the enemy's probabilistic loot category.
    const order = createNativeWorldManagerOrder(state.worldManagerOrder)
    const spawned = loot.spawnBoneyardLootSpecs(state.world.loot, [{
      activationDelayTicks: 0, amount: spec.gold, id: 1, kind: 'gold', nativeTypeId: 2012,
      phase: 0, position: { x: 1000, y: 1000 }, scatterSeed: 17, source: 'script', tier: 2,
    }], 0, order.register)
    state = { ...state, worldManagerOrder: order.state(), world: { ...state.world, loot: spawned.store } }
  }
  if (spec.lethal) {
    state.playerEntities = entities.damagePlayerEntityWithResult(state.playerEntities,
      'local', 1000, 0, true, true, 1).store
  }
  if (spec.experience) state = game.grantGameSimulationPlayerExperience(state, 'local', spec.experience)
  if (spec.college) state = game.armGameSimulationCollegeIntro(state, 'local')
  const frames = [{ frame: 0, expected: snapshot(state) }]
  const choices = []
  const commands = []
  const inputs = []
  const officeExitPath = [{ x: 350, y: 600 }, { x: 350, y: 800 },
    { x: 512, y: 850 }, { x: 512, y: 1100 }]
  let officeExitWaypoint = 0
  for (let frame = 1; frame <= spec.ticks; frame += 1) {
    if (spec.lethal && frame === 520) {
      state = game.continueGameSimulationOver(state, state.run.runId, state.run.gameOverEventId)
      if (state === null) throw new Error('Recorded Game Over continue must be admitted')
      commands.push({ frame, action: 'continue' })
    }
    if ((spec.experience && frame === 5) || spec.name === 'rewards-pickups') {
      while (state.levelUpBarrier !== null) {
        const offer = state.playerEntities.progressions[0].pendingOffer
        const option = offer.options[0]
        const choice = { choiceIndex: 0, offerSequence: offer.sequence, skillId: option.skillId }
        state = game.selectGameSimulationPlayerSkill(state, 'local', choice)
        if (state === null) throw new Error('Recorded first skill offer must be admitted')
        choices.push({ frame, choice })
      }
    }
    if (spec.college && state.world.participants.local.collegeIntro?.phase === 'arch-dialogue') {
      const result = game.applyGameSimulationHubAction(state, 'local', { type: 'acknowledge-college-intro-dialogue' })
      if (!result.accepted) throw new Error('College dialogue acknowledgement must be admitted')
      state = result.state
      commands.push({ frame, action: 'ack-college' })
    }
    if (spec.college && state.world.participants.local.transition?.phase === 'college-loadout') {
      state = game.confirmGameSimulationLoadout(state, 'local', { displayName: 'Oracle', element: 'water', discipline: 'mind' })
      if (state === null) throw new Error('College loadout must be admitted')
      commands.push({ frame, action: 'loadout' })
    }
    let movement = { x: 0, y: 0 }
    if (spec.college && state.world.participants.local.collegeIntro === null
      && state.world.participants.local.region === 'office'
      && state.world.participants.local.transition === null) {
      const character = game.getPlayerCharacter(state, 'local')
      while (officeExitWaypoint < officeExitPath.length - 1
        && Math.hypot(officeExitPath[officeExitWaypoint].x - character.position.x,
          officeExitPath[officeExitWaypoint].y - character.position.y) < 10) officeExitWaypoint += 1
      const target = officeExitPath[officeExitWaypoint]
      movement = { x: target.x - character.position.x, y: target.y - character.position.y }
    }
    if (spec.hub && !spec.college) movement = { x: frame < 125 ? 1 : 0, y: frame >= 125 ? 1 : 0 }
    if (spec.name === 'staff-contact' || (spec.name === 'rewards-pickups' && frame > 200)) movement = { x: 1, y: 0 }
    const target = state.world.kind === 'boneyard' ? state.world.enemies.actors.find(actor => actor.lifeState === 'alive') : null
    const input = { aim: spec.hub ? null : target?.position ?? { x: 1170, y: 1000 },
      cast: { primary: !spec.hub && !spec.lethal && spec.name !== 'staff-contact' && frame < 200,
        quickbar: spec.secondary && frame < 100 ? state.playerEntities.belts[0].findIndex(
          entry => entry?.kind === 'skill' && entry.skillId === 72) : null },
      movement, viewportWidth: 1600, viewportHeight: 900 }
    const enemySpawnIntents = frame === 1 && !spec.hub && !spec.tutorial && !spec.lethal && !spec.experience
      ? [{ enemyToken: 'ZOMBIE', flags: [], id: 1, locationPolicy: 'near-player',
          nativeTypeId: 1006, pathfindingMode: 1, position: { x: 1070, y: 1000 },
          positionPolicy: 'direct', spawnTick: state.tick + 1, waveOrdinal: 0 }]
      : []
    inputs.push({ input, enemySpawnIntents, collegeIntroReady: spec.college === true && frame > 2 })
    state = game.stepGameSimulationTick(state, { local: input }, { enemySpawnIntents,
      ...(spec.college ? { collegeIntroReadyPlayerIds: new Set(frame > 2 ? ['local'] : []) } : {}) })
    if (frame <= 5 || frame % 25 === 0 || frame >= spec.ticks - 2
      || (spec.experience && frame <= 7)) frames.push({ frame, expected: snapshot(state) })
  }
  cases.push({ spec, choices, commands, inputs, frames })
}
writeFixture('session-tick-goldens.json', 'tools/website-oracle/session-tick-goldens.mjs',
  'Website@fa08ae249ff5563570bdb2a653e2ef9358ce7b83/frontend/src/game/core-server/game-simulation.ts::stepGameSimulationTick (current public Game)', { cases })
