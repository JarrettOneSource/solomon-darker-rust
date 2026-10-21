#!/usr/bin/env node
// Record the actual current constructors with both Source quality settings.
// No earlier fixture values or Rust output participate in these expectations.
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const stores = await loadKernel('core-server/boneyard-enemy-store.ts')
const working = await loadKernel('core-server/enemies/work.ts')
const construction = await loadKernel('core-server/enemies/construction.ts')
const deaths = await loadKernel('core-server/enemies/deaths.ts')
const effects = await loadKernel('core-server/enemies/death-effects.ts')
const emission = await loadKernel('core-server/enemies/projectile-emission.ts')
const projectiles = await loadKernel('core-server/enemies/projectiles.ts')
const skullEffects = await loadKernel('core-server/enemies/demon-skull-effects.ts')
const bosses = await loadKernel('core-server/enemies/boss-spells.ts')
const bossConstruction = await loadKernel('core-server/enemies/boss-spell-construction.ts')
const managers = await loadKernel('core-kernels/native-world-manager-order.ts')
const portal = await loadKernel('core-kernels/native-survival-portal.ts')
const schema = await loadKernel('core-kernels/boneyard-wave-schema.ts')

const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], {
  cwd: websiteDir, encoding: 'utf8',
}).trim()
const rng = state => [state.indexA, state.indexB, ...state.words]
const owner = { id: 1, position: { x: 150.25, y: 250.75 } }
const cases = []
const programs = [
  ['bouncer', 'SKELETON'], ['bouncer-explicit-clock', 'SKELETON'],
  ...['SKELETON', 'HEARTMONGER', 'ZOMBIE', 'WRAITH', 'COFFIN', 'PORTAL', 'DIREFACULTY']
    .map(token => ['terminal', token]),
  ['arrow', 'SKELETONARCHER'], ['bomb', 'SKELETONARCHER'],
  ['mage', 'SKELETONMAGE'], ['eye-trail', 'DEMONSKULL'],
  ['falling-bone', 'DIREFACULTY'], ['tragic-circle', 'DIREFACULTY'],
  ['ultra-banish', 'DEMONSKULL'],
]
for (const enhancedEffects of [true, false]) {
  for (const [program, token] of programs) {
    const seed = `current-quality:${program}:${token}:${enhancedEffects}`
    const order = managers.createNativeWorldManagerOrder({
      nextRegistrationOrdinal: { actor: 100, transient: 70 },
    })
    const context = {
      enhancedEffects, tick: 10,
      players: { hero: { alive: true, connected: true, eligible: true,
        collisionRadius: 25, position: program === 'bomb' ? { x: 700, y: 750 } : owner.position, velocityPerTick: { x: 0, y: 0 } } },
      resolveSpawnIntents: () => [], registerWorldPainter: order.register,
      resolveMovement: request => request.requestedPosition,
      projectileWorldBlocked: () => false,
      clipSpellSegment: ({ end }) => end,
      navigation: { isPathClear: () => true, findRoute: () => null },
    }
    const work = working.createEnemyWork(stores.createBoneyardEnemyStore(seed, 50), context, true)
    const intent = { enemyToken: token, nativeTypeId: schema.BONEYARD_WAVE_ENEMY_TYPES[token],
      flags: [], id: 1, locationPolicy: 'anywhere', position: owner.position,
      spawnTick: 10, waveOrdinal: 31,
      ...(token === 'PORTAL' ? { authoredRecipe: portal.nativePortalRecipe(
        Object.values(portal.NATIVE_PORTAL_PROGRAM_BY_SOURCE_SHA256)[0].phases[0]) } : {}),
    }
    let actor = construction.materializeSpawnIntents(work, context, [intent])[0]
    work.actors.push(actor)
    let retained = null
    let spellInput = null
    let ticks = 0
    if (program.startsWith('bouncer')) {
      effects.spawnBouncer(work, owner, 10, 113, program,
        program === 'bouncer-explicit-clock' ? { opacityTimer: 1.5 } : {})
    } else if (program === 'terminal') {
      actor = { ...actor, currentHealth: 0, lifeState: 'dying', deathStartedTick: 10,
        lastDamagedByPlayerId: 'hero' }
      if (token === 'DIREFACULTY') {
        actor = { ...actor, deathTick: 249 }
        context.tick = 260
      }
      retained = deaths.stepDyingActor(work, actor, context) !== null
    } else if (program === 'arrow' || program === 'bomb') {
      emission.spawnProjectile(work, actor, 10, program === 'arrow' ? 'arrow' : 'demon-bomb',
        8, program === 'arrow' ? { speed: 6, minimumSpeed: 6, lifetimeTicks: 40, headingDeg: 0 } : {})
      if (program === 'bomb') {
        for (let tick = 11; tick <= 14; tick += 1) {
          context.tick = tick
          projectiles.stepProjectiles(work, context)
          ticks += 1
        }
      }
    } else if (program === 'mage') {
      actor = { ...actor, brain: { ...actor.brain, phase: 'cast',
        lightningTicksRemaining: 4, lightningTargetPlayerId: 'hero',
        lightningTargetPosition: owner.position } }
      emission.stepMageLightningPulse(work, actor, context)
    } else if (program === 'eye-trail') {
      skullEffects.spawnUnholyEyeTrail(work, owner, 10, 37.25)
    } else {
      const kind = program
      const source = { ...bossConstruction.createBossSpellOwner(work, 1, 10, 8, kind),
        position: owner.position,
        ...(kind === 'falling-bone' ? { entry: 113, height: -1, colorRamp: 0, rotationDeg: 25 } : {}),
        ...(kind === 'tragic-circle' ? { remainingTicks: 500 } : {}),
        ...(kind === 'ultra-banish' ? { remainingTicks: 130, alpha: 1, flashAlpha: 0,
          lightRadius: 1, megaDeath: true } : {}),
      }
      spellInput = structuredClone(source)
      work.bossSpells.push(source)
      const count = kind === 'ultra-banish' ? 4 : 1
      for (let tick = 11; tick < 11 + count; tick += 1) {
        context.tick = tick
        bosses.stepBossSpells(work, context)
        ticks += 1
      }
    }
    cases.push({ seed, program, enhancedEffects, intent, ticks, spellInput, expected: {
      retained, deathEffects: work.deathEffects, projectileEffects: work.projectileEffects,
      projectiles: work.projectiles, pulses: work.mageLightningPulses,
      bossSpells: work.bossSpells, rng: work.rngState, steering: rng(work.steeringRngState),
      nextDeathEffectId: work.nextDeathEffectId, nextProjectileEffectId: work.nextProjectileEffectId,
      painterOrder: order.state().nextRegistrationOrdinal,
    } })
  }
}
writeFixture('current-enemy-quality-goldens.json', 'current-enemy-quality-goldens.mjs',
  'Current Website core-server/enemies constructor, terminal and retained projectile programs',
  { sourceRevision, cases })
console.log(`current enemy quality: ${cases.length} actual Source programs`)
