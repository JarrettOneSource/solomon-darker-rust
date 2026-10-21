// Accepted shared Game ticks. The pinned Website remains untouched.
import { loadKernel, writeFixture } from './oracle.mjs'
import { multiplayerSessionSnapshot as snapshot } from './session-tick-rows.mjs'
const game = await loadKernel('core-server/game-simulation.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')
const skills = await loadKernel('core-kernels/player-skill-runtime.ts')
const belt = await loadKernel('core-kernels/native-belt.ts')
const enemies = await loadKernel('core-server/enemies/damage.ts')
const { createNativeWorldManagerOrder } = await loadKernel('core-kernels/native-world-manager-order.ts')
const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')

const specs = [
  { name: 'hub-registration-physics', hub: true, elements: ['fire', 'water', 'ether'], ticks: 180 },
  { name: 'mixed-primary-world', elements: ['fire', 'earth', 'ether'], ticks: 240 },
  { name: 'air-water-secondary', elements: ['water', 'air', 'fire'], ticks: 240, secondary: true },
  { name: 'shared-level-barrier', elements: ['fire', 'earth', 'air'], ticks: 55, experience: true },
  { name: 'staff-empty-contact', elements: ['air', 'fire', 'earth'], ticks: 90, staff: true },
]
const ids = ['zeta', 'alpha', 'beta']
const cases = []
for (const spec of specs) {
  const configurations = ids.map((id, index) => [id, {
    displayName: 'Oracle', element: spec.elements[index], discipline: spec.staff ? 'body' : 'arcane',
  }])
  let state = game.createGameSimulation(Object.fromEntries(configurations), { gameRngSeed: 17, combatRngSeed: 91 })
  if (!spec.hub) {
    state = game.enterBoneyardWorld(state, { ...NATIVE_GENERATED_BONEYARDS[0],
      choice: { id: 'default-random', source: 'default' }, runId: `multiplayer-${spec.name}`, seed: `multiplayer-${spec.name}` })
    state.world = { ...state.world, encounter: null, waves: null, arenaTransition: null }
    for (const [index, id] of ids.entries()) {
      state.playerEntities = entities.replacePlayerCharacter(state.playerEntities, id, {
        ...game.getPlayerCharacter(state, id), position: { x: 1000 + index * 15, y: 1000 },
      })
    }
  }
  if (spec.secondary) {
    const store = state.playerEntities
    const book = structuredClone(store.skillBooks[0])
    book.permanentRanks[72] = 1
    book.learnedSkillOrder.push(72)
    const refreshed = skills.refreshPlayerSkillRuntime(store.skillRuntimes[0], book, store.statBooks[0], store.economies[0])
    state.playerEntities = { ...store, skillBooks: [refreshed.skillBook, ...store.skillBooks.slice(1)],
      skillRuntimes: [refreshed.runtime, ...store.skillRuntimes.slice(1)],
      belts: [belt.bindNativeBeltSkill(belt.createNativePlayerBelt(refreshed.skillBook), refreshed.skillBook, 72, 2), ...store.belts.slice(1)] }
  }
  const frames = [{ frame: 0, expected: snapshot(state) }]
  const transactions = []
  let removedStaffTarget = false
  for (let frame = 1; frame <= spec.ticks; frame += 1) {
    const commands = []
    if (spec.experience && frame === 5) {
      state = game.grantGameSimulationPlayerExperience(state, 'zeta', 95)
      commands.push({ action: 'experience', playerId: 'zeta', amount: 95 })
    }
    if (spec.experience && frame >= 10 && frame % 5 === 0 && state.levelUpBarrier !== null) {
      const id = state.levelUpBarrier.pendingPlayerIds[0]
      const index = state.playerEntities.identities.findIndex(identity => identity.playerId === id)
      const offer = state.playerEntities.progressions[index].pendingOffer
      const choice = { choiceIndex: 0, offerSequence: offer.sequence, skillId: offer.options[0].skillId }
      state = game.selectGameSimulationPlayerSkill(state, id, choice)
      if (state === null) throw new Error('Recorded shared offer must be admitted')
      commands.push({ action: 'choice', playerId: id, choice })
    }
    if (spec.staff && !removedStaffTarget
      && state.primarySpells.transients.some(actor => actor.kind === 'player-staff-melee' || actor.kind === 'player-staff-spin')) {
      const target = state.world.enemies.actors.find(actor => actor.lifeState === 'alive')
      if (target) {
        const request = { actorId: target.id, amount: 1_000_000, tick: state.tick, sourcePlayerId: 'zeta' }
        const order = createNativeWorldManagerOrder(state.worldManagerOrder)
        const damaged = enemies.damageBoneyardEnemy(state.world.enemies, { ...request, registerWorldPainter: order.register })
        state = { ...state, worldManagerOrder: order.state(), world: { ...state.world, enemies: damaged.store } }
        commands.push({ action: 'damage', ...request })
        removedStaffTarget = true
      }
    }
    const inputs = Object.fromEntries(ids.map((id, index) => {
      const target = state.world.kind === 'boneyard' ? state.world.enemies.actors.find(actor => actor.lifeState === 'alive') : null
      return [id, { aim: spec.hub ? null : target?.position ?? { x: 1170, y: 1000 },
        cast: { primary: !spec.hub && !spec.experience && !spec.staff && frame < 160,
          quickbar: spec.secondary && index === 0 && frame < 70 ? 2 : null },
        movement: spec.hub ? { x: frame < 90 ? 1 : 0, y: frame >= 90 ? 1 : 0 }
          : spec.staff ? { x: 1, y: 0 } : { x: 0, y: 0 },
        viewportWidth: [1600, 1280, 900][index], viewportHeight: [900, 720, 1600][index] }]
    }))
    const enemySpawnIntents = frame === 1 && !spec.hub && !spec.experience
      ? [{ enemyToken: 'ZOMBIE', flags: [], id: 1, locationPolicy: 'near-player', nativeTypeId: 1006,
          pathfindingMode: 1, position: { x: spec.staff ? 1070 : 1170, y: 1000 }, positionPolicy: 'direct',
          spawnTick: state.tick + 1, waveOrdinal: 0 }] : []
    transactions.push({ commands, inputs, enemySpawnIntents })
    state = game.stepGameSimulationTick(state, inputs, { enemySpawnIntents })
    if (frame <= 6 || frame % 10 === 0 || commands.length > 0 || frame >= spec.ticks - 2) frames.push({ frame, expected: snapshot(state) })
  }
  if (spec.staff && !removedStaffTarget) throw new Error('Staff case must kill a real target during its live action')
  cases.push({ spec, configurations, transactions, frames })
}
writeFixture('session-multiplayer-goldens.json', 'tools/website-oracle/session-multiplayer-goldens.mjs',
  'Website@fa08ae249ff5563570bdb2a653e2ef9358ce7b83/frontend/src/game/core-server/game-simulation.ts::stepGameSimulationTick (ordered multi-participant public Game)', { cases })
