#!/usr/bin/env node
// Observe real birth callbacks inside the source's full Boneyard world step.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const { createBoneyardWorld, prepareBoneyardWorldNavigation } = await loadKernel('core-server/boneyard-world-construction.ts')
const { stepBoneyardWorldTick } = await loadKernel('core-server/boneyard-world.ts')
const { spawnPlayerCharacterInBoneyard } = await loadKernel('core-server/boneyard-world-placement.ts')
const { createIdlePlayerCharacterInput } = await loadKernel('core-kernels/player-character.ts')
const { createNativeWorldManagerOrder } = await loadKernel('core-kernels/native-world-manager-order.ts')
const { startBoneyardArenaTransition, stepBoneyardArenaTransition } = await loadKernel('core-kernels/boneyard-arena-transition.ts')
const { startBoneyardWaveDirector } = await loadKernel('core-kernels/boneyard-wave-director.ts')
const { boneyardWorldLightQuery } = await loadKernel('core-server/boneyard-world-light.ts')
const { createBoneyardEnemyVisibility } = await loadKernel('core-server/boneyard-enemy-visibility.ts')
const { boneyardProjectilePointGain } = await loadKernel('core-server/boneyard-projectile-world.ts')
const feedback = await loadKernel('core-kernels/native-enemy-world-feedback.ts')

const cases = []
for (const templateIndex of [0, 3]) {
  const template = NATIVE_GENERATED_BONEYARDS[templateIndex]
  for (const positionPolicy of ['direct', 'dark', 'light', 'offscreen']) {
    const seed = `world-placement-${templateIndex}-${positionPolicy}`
    const loaded = { ...template, choice: { id: 'default-random', source: 'default' }, seed, runId: 'oracle' }
    const lanternRegistration = { managerLane: 'actor', registrationOrdinal: 5 }
    const world = createBoneyardWorld(loaded, lanternRegistration, { managerLane: 'actor', registrationOrdinal: 6 })
    // Native waves use the host's preprepared combat mesh. Pause that timeline while
    // testing externally requested births, so no unrelated wave births consume RNG.
    prepareBoneyardWorldNavigation(world)
    world.waves = startBoneyardWaveDirector(world.waves)
    world.arenaTransition = startBoneyardArenaTransition(world.arenaTransition)
    for (let tick = 0; tick < 400; tick++) world.arenaTransition = stepBoneyardArenaTransition(world.arenaTransition)
    const player = spawnPlayerCharacterInBoneyard({ displayName: 'oracle', element: 'ether', discipline: 'arcane' }, world)
    player.position = { x: template.scene.solomonDig.position.x + 100, y: template.scene.solomonDig.position.y + 100 }
    const intent = { enemyToken: 'ZOMBIE', flags: [], id: 1, locationPolicy: 'anywhere', nativeTypeId: 1006,
      position: template.scene.solomonDig.position, positionPolicy, spawnTick: 1, waveOrdinal: 0 }
    const order = createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 10, transient: 0 } })
    const sharedRngState = world.loot.sharedRng
    const step = stepBoneyardWorldTick(world, { local: player }, { local: createIdlePlayerCharacterInput() },
      { local: { alive: true, eligible: true, collisionEnabled: true, movementScale: 1 } }, 1,
      order.register, order.register, {}, [], [intent], undefined, true)
    const light = boneyardWorldLightQuery(step.world, step.players, step.world.enemies, 1)
    const visibility = createBoneyardEnemyVisibility(world.arenaTransition.combatBounds, step.players,
      { local: createIdlePlayerCharacterInput() }, light.acceptedSources)
    const points = [step.players.local.position, world.lanternPosition, { x: -100, y: -100 },
      { x: world.bounds.w / 2, y: world.bounds.h / 2 }, { x: 1200, y: 3000 }]
    const visibilityQueries = points.map(position => ({ position, native: visibility.nativeVisibility(position),
      projected: visibility.projectedPointVisible(position) }))
    let feedbackState = feedback.stepNativeEnemyWorldFeedback(feedback.createNativeEnemyWorldFeedbackState())
    const feedbackEvents = [
      { output: 'wraith-fragments', count: 2 }, { explosion: step.players.local.position },
      { output: 'imp-split', count: 0 }, { explosion: { x: -10000, y: -10000 } },
      { output: 'faculty-break', count: 1 },
    ].map(event => {
      if (event.explosion) {
        const gain = boneyardProjectilePointGain(world.arenaTransition.combatBounds,
          { position: step.players.local.position, viewportWidth: 1600, viewportHeight: 900 }, event.explosion, false)
        feedbackState = feedback.applyNativeEnemyWorldFeedback(feedbackState, Math.fround(4 * gain))
      } else {
        for (const intensity of feedback.nativeEnemyWorldFeedbackImpulses(event.output, event.count)) {
          feedbackState = feedback.applyNativeEnemyWorldFeedback(feedbackState, intensity)
        }
      }
      return { ...event, expected: feedbackState }
    })
    cases.push({ templateIndex, seed, intent, player: step.players.local, lanternRegistration,
      nativeRegistrationBase: world.enemies.nextNativeRegistrationOrder,
      sharedRngState,
      visibilityQueries, nativeViewBounds: visibility.nativeViewBounds, feedbackEvents,
      activeBounds: world.arenaTransition.combatBounds, cameraBounds: world.arenaTransition.cameraBounds,
      expected: { position: step.world.enemies.actors[0].position,
        steeringRngState: step.world.enemies.steeringRngState,
        sharedRngState: step.world.loot.sharedRng, lootSeed: step.world.enemies.actors[0].lootSeed,
        actorRngState: step.world.enemies.rngState, painterOrder: order.state() } })
  }
}
writeFixture('world-geometry-goldens.json', 'tools/website-oracle/world-geometry-goldens.mjs',
  'Website/frontend/src/game/core-server/boneyard-world.ts::stepBoneyardWorldTick actual spawn callbacks',
  { sourceRevision: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim(), cases })
