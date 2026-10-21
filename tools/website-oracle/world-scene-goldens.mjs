#!/usr/bin/env node
// The real source world from player movement through the first encounter release.
import { loadKernel, writeFixture } from './oracle.mjs'

const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const { createBoneyardWorld } = await loadKernel('core-server/boneyard-world-construction.ts')
const { stepBoneyardWorldTick } = await loadKernel('core-server/boneyard-world.ts')
const { spawnPlayerCharacterInBoneyard } = await loadKernel('core-server/boneyard-world-placement.ts')
const { createIdlePlayerCharacterInput } = await loadKernel('core-kernels/player-character.ts')
const { createNativeWorldManagerOrder } = await loadKernel('core-kernels/native-world-manager-order.ts')

function snapshot(world, player, contacts = [], lightCameraBounds = world.arenaTransition.cameraBounds) {
  const encounter = world.encounter
  const waves = world.waves
  return {
    wizard: { position: player.position, velocity: player.velocity, headingIndex: player.headingIndex,
      gaitDegrees: player.gaitDegrees, walkCyclePrimary: player.walkCyclePrimary },
    lanternPosition: world.lanternPosition, arenaTransition: world.arenaTransition,
    encounter: { phase: encounter.phase, position: encounter.position, headingDeg: encounter.headingDeg,
      runEventId: encounter.runEventId, voiceTicksRemaining: encounter.voiceTicksRemaining,
      phaseTicksRemaining: encounter.phaseTicksRemaining, rngState: encounter.rngState,
      targetPlayerId: encounter.targetPlayerId },
    waves: { phase: waves.phase, waveOrdinal: waves.waveOrdinal,
      slumpgutPollCursor: waves.slumpgutPollCursor, populationThreshold: waves.populationThreshold,
      lowPopulationTicks: waves.lowPopulationTicks, activeBurstIndex: waves.activeBurstIndex,
      activeGroupIndex: waves.activeGroupIndex, spawnCountdown: waves.spawnCountdown, rngState: waves.rngState },
    contacts,
    enemyWorldFeedback: world.enemyWorldFeedback,
    loot: { lastStepTick: world.loot.lastStepTick, lastSuccessfulItemLevel: world.loot.lastSuccessfulItemLevel,
      nextKeyDropLevel: world.loot.nextKeyDropLevel, nextItemId: world.loot.nextItemId,
      sharedRng: world.loot.sharedRng,
      goodies: world.loot.goodies.map(({ eid, id, rewardSeed, phase, timer, active, exhausted, sceneryRegistrationOrdinal }) => (
        { eid, id, rewardSeed, phase, timer, active, exhausted, sceneryRegistrationOrdinal }
      )) },
    lightCameraBounds,
  }
}
const cases = []
for (const [templateIndex, paused, castOwnsFacing, alive] of [[0, false, false, true], [3, true, true, true], [0, false, false, false]]) {
  const template = NATIVE_GENERATED_BONEYARDS[templateIndex]
  const seed = alive ? `scene-world-${templateIndex}` : 'scene-world-ghost'
  const loaded = { ...template, choice: { id: 'default-random', source: 'default' }, seed, runId: 'oracle' }
  const order = createNativeWorldManagerOrder()
  order.register('actor')
  let world = createBoneyardWorld(loaded, order.register('actor'), order.register('actor'))
  let player = spawnPlayerCharacterInBoneyard({ displayName: 'oracle', element: 'ether', discipline: 'arcane' }, world)
  player.position = { x: template.scene.solomonDig.position.x + 30, y: template.scene.solomonDig.position.y + 50 }
  player.primaryCast = { ...player.primaryCast, channelActive: castOwnsFacing }
  const webbed = paused ? { severity: 1.25, strength: 12, cocoonHealth: 0, hitPulse: 0 } : null
  if (webbed) world = { ...world, enemies: { ...world.enemies, webbedPlayers: { local: webbed } } }
  const records = [{ tick: 0, state: snapshot(world, player) }]
  let tick = 0
  let releaseTick = null
  while (tick < 2000) {
    if (!alive && tick >= 200) break
    if (releaseTick !== null && tick >= releaseTick + 32) break
    tick++
    const lightCameraBounds = world.arenaTransition.cameraBounds
    const movement = { x: tick % 200 < 100 ? 1 : -1, y: 0 }
    const step = stepBoneyardWorldTick(world, { local: player },
      { local: { ...createIdlePlayerCharacterInput(), movement } },
      { local: { alive, eligible: alive, collisionEnabled: alive, movementScale: 0.8 } },
      tick, order.register, order.register, {}, [], [], undefined, paused || releaseTick !== null)
    const changed = step.world.encounter.phase !== world.encounter.phase || step.world.encounter.runEventId !== world.encounter.runEventId
    world = step.world
    player = step.players.local
    if (world.encounter.runEventId > 0 && releaseTick === null) releaseTick = tick
    if (tick % 25 === 0 || changed || (releaseTick !== null && tick === releaseTick + 32)) records.push({ tick,
      state: snapshot(world, player, step.movementContactsByPlayerId.local, lightCameraBounds) })
  }
  if (alive && world.encounter.runEventId !== 1) throw new Error('scene oracle did not release native waves')
  cases.push({ templateIndex, seed, paused, castOwnsFacing, alive, webbed,
    nativeRegistrationBase: world.enemies.nextNativeRegistrationOrder, records })
}
writeFixture('world-scene-goldens.json', 'tools/website-oracle/world-scene-goldens.mjs',
  'Website/frontend/src/game/core-server/boneyard-world.ts complete movement/encounter/release phases', { cases })
