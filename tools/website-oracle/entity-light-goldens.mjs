// Entity-owned providers, before the manager's submission rounding and containment pass.
import { loadKernel, writeFixture } from './oracle.mjs'
const light = await loadKernel('core-kernels/native-boneyard-light-model.ts')
const { BONEYARD_WAVE_ENEMY_TYPES } = await loadKernel('core-kernels/boneyard-wave-schema.ts')

const enemies = []
for (const enemyToken of Object.keys(BONEYARD_WAVE_ENEMY_TYPES)) {
  for (const frame of [0, 1, 731, 4294967301]) {
    for (let variant = 0; variant < 6; variant++) {
      const owner = { enemyToken, id: 17, burning: variant % 2 === 1, scale: 1.3,
        demonSkull: { bodyOffset: { x: 3.25, y: -12.5 } }, flags: [], position: { x: 123.125, y: 450.75 },
        animation: { state: variant >= 3 ? 'death' : 'idle', alpha: variant / 5, bodyPose: variant === 2 ? 9 : 0 },
        lighting: { providerCopies: variant % 3, charge: 0.7, glow: 0.85 } }
      const multipleShadows = frame % 2 === 0
      enemies.push({ owner, frame, multipleShadows, expected: light.nativeEnemyLightSources(owner, frame, multipleShadows) })
    }
  }
}
const projectiles = []
for (const kind of ['arrow', 'firebolt', 'guided-missile', 'demon-bomb', 'poison-pool']) {
  for (const payload of ['none', 'normal', 'fire', 'cold', 'poison']) {
    for (const frame of [0, 31, 4294967301]) {
      const owner = { id: 731, kind, payload, position: { x: 100.125, y: -25.25 } }
      projectiles.push({ owner, frame, multipleShadows: frame !== 31,
        expected: light.nativeEnemyProjectileLightProvider(owner, frame, frame !== 31) })
    }
  }
}
const effects = []
for (const kind of ['demon-fire', 'demon-explosion-lit-array', 'fire-burst', 'guided-impact', 'arrow-tumble', 'demon-explosion-core', 'poison-bubble']) {
  for (const ageTicks of [0, 1, 10, 25, 100]) {
    for (const registration of [null, { managerLane: 'actor', registrationOrdinal: 30 }]) {
      const owner = { kind, ageTicks, alpha: 0.2, position: { x: 50.5, y: 75.25 }, lightRegistration: registration }
      effects.push({ owner, multipleShadows: ageTicks % 2 === 0, pointGain: ageTicks === 10 ? 0.25 : 1,
        expected: light.nativeEnemyProjectileEffectLightProvider(owner, ageTicks % 2 === 0, ageTicks === 10 ? 0.25 : 1) })
    }
  }
}
const secondary = []
for (const kind of ['moving-fire', 'fire-patch', 'ring-fire-explosion', 'ring-fire-fragment', 'shockwave',
  'mindblast-shockwave', 'freeze-wave', 'storm-cloud', 'acid-rain', 'leviathan', 'ether-bolt', 'golem',
  'magic-trap', 'ether-drain', 'ether-fade', 'comet', 'magic-circle', 'fire-burn', 'ether-burn', 'electric-burn', 'acid-splash']) {
  for (const ageTicks of [0, 1, 20, 1499, 1500]) {
    for (const variant of [0, 1]) {
      const owner = { kind, id: 1731, ageTicks, variant, position: { x: 700.5, y: 1234.25 },
        radius: ageTicks === 0 ? 0 : 1.5, alpha: 0.7, phase: ageTicks === 0 ? 0 : 0.8, scale: 1.3, slowFactor: 0.03 }
      secondary.push({ owner, frame: 731, multipleShadows: variant === 0, pointGain: 0.75,
        expected: light.nativeSecondaryProviderLightSource(owner, 731, variant === 0, 0.75),
        misc: light.nativeSecondaryMiscLightSource(owner) })
    }
  }
}
const weld = []
for (let buildId = 1000; buildId <= 1009; buildId++) {
  for (const frame of [0, 10, 731]) {
    const owner = { id: 42, buildId, position: { x: 123.5, y: 731.5 } }
    weld.push({ kind: 'projectile', owner, frame, multipleShadows: frame !== 10,
      expected: light.nativeWeldProjectileLightSource(owner, frame, frame !== 10) })
  }
}
for (const scale of [0, 0.5, 1, 2.5]) {
  const owner = { origin: { x: 123.5, y: 731.5 }, scale }
  weld.push({ kind: 'rock', owner, multipleShadows: false, expected: light.nativeWeldRockLightSource(owner, false) })
}
for (const phase of ['falling', 'impact']) {
  for (const fallHeight of [-0.2, 0, 0.5, 1, 1.1]) {
    for (const impactTicksRemaining of [0, 25, 60]) {
      const owner = { position: { x: 400, y: 800 }, phase, fallHeight, impactTicksRemaining, impactRadiusScalar: 3 }
      weld.push({ kind: 'meteor', owner, expected: light.nativeWeldMeteorLightSource(owner) })
    }
  }
}
writeFixture('entity-light-goldens.json', 'tools/website-oracle/entity-light-goldens.mjs',
  'Website core-kernels/native-boneyard-light-model.ts entity providers', { enemies, projectiles, effects, secondary, weld })
console.log(JSON.stringify({ enemies:enemies.length,projectiles:projectiles.length,effects:effects.length,secondary:secondary.length,weld:weld.length }))
