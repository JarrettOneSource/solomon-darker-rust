#!/usr/bin/env node
// Full source collector with actual player, projectile, death and secondary-light owners.
import { loadKernel, writeFixture } from './oracle.mjs'

const { boneyardWorldLightQuery } = await loadKernel('core-server/boneyard-world-light.ts')
const { createPlayerCharacter } = await loadKernel('core-kernels/player-character.ts')
const { nativePrimaryViewBounds } = await loadKernel('core-kernels/primary-spell-targeting.ts')
const { nativeRegionPointGain } = await loadKernel('core-kernels/native-region-point-gain.ts')
const light = await loadKernel('core-kernels/native-boneyard-light-model.ts')
const registration = (managerLane, registrationOrdinal) => ({ managerLane, registrationOrdinal })
const bounds = { x: 0, y: 0, w: 2400, h: 3700 }
const cases = []

for (const tick of [25, 107, 425]) {
  for (const participantCount of [0, 1, 2]) {
    const players = Object.fromEntries(Array.from({ length: participantCount }, (_, index) => {
      const player = createPlayerCharacter({ displayName: 'oracle', discipline: 'arcane', element: 'ether' },
        { x: 600 + index * 1200, y: 700 + index * 1800, facingDeg: 180 })
      return [`player-${index}`, { ...player, headingIndex: index * 7 + 3,
        primaryCast: { ...player.primaryCast, weaponPulse: 0.2 + index * 0.3 } }]
    }))
    const identities = Object.keys(players).map(playerId => ({ playerId }))
    const lightings = identities.map((_, index) => ({
      overlayEffectPhase: 0.35 - index * 0.25,
      lightRegistration: registration('actor', 30 - index * 10),
    }))
    const inputs = Object.fromEntries(identities.map(({ playerId }, index) => [playerId,
      { viewportWidth: index ? 1000 : 1600, viewportHeight: index ? 1200 : 900 }]))
    const deathEffects = [
      { kind: 'banish-black', position: { x: 750, y: 800 }, ageTicks: 50, painterRegistration: registration('transient', 4) },
      { kind: 'banish-black', position: { x: 700, y: 700 }, ageTicks: 225, painterRegistration: registration('actor', 30) },
      { kind: 'banish-white', position: { x: 650, y: 700 }, ageTicks: 0, painterRegistration: registration('actor', 0) },
    ]
    const projectiles = [
      { kind: 'ether', id: 14, position: { x: 650, y: 750 }, worldKey: 'boneyard:oracle', lightRegistration: registration('actor', 8) },
      { kind: 'earth', charge: 1.2, id: 15, position: { x: 1800, y: 2500 }, worldKey: 'boneyard:oracle', lightRegistration: registration('transient', 1) },
      { kind: 'ether', id: 16, position: { x: 650, y: 750 }, worldKey: 'boneyard:other', lightRegistration: registration('actor', 2) },
    ]
    const secondaryActors = [
      { kind: 'magic-circle', id: 5, position: { x: 800, y: 700 }, scale: 2, alpha: 0.8, ageTicks: 9,
        miscLightAppendOrdinal: 3, lightRegistration: registration('actor', 30), worldKey: 'boneyard:oracle' },
      { kind: 'fire-burn', id: 2, position: { x: 850, y: 700 }, radius: 0.8, alpha: 0.6, ageTicks: 6,
        miscLightAppendOrdinal: 1, lightRegistration: registration('actor', 30), worldKey: 'boneyard:oracle' },
      { kind: 'ether-burn', id: 3, position: { x: 1750, y: 2500 }, radius: 1.2, alpha: 0.5, ageTicks: 4,
        miscLightAppendOrdinal: 1, lightRegistration: registration('transient', 0), worldKey: 'boneyard:oracle' },
    ]
    const world = { bounds, runId: 'oracle', arenaTransition: { cameraBounds: { x: 150, y: 200, w: 2100, h: 3300 } },
      lanternPosition: { x: 1200, y: 1100 }, lanternLightRegistration: registration('actor', 3) }
    const enemies = { actors: [], deathEffects, bossSpells: [], projectiles: [], projectileEffects: [], mageLightningPulses: [] }
    const useDefaults = tick === 107
    const environment = { inputs: useDefaults ? undefined : inputs,
      playerEntities: useDefaults ? undefined : { identities, lightings },
      primarySpells: { projectiles, transients: [] }, secondaryAbilities: { actors: secondaryActors } }
    const query = boneyardWorldLightQuery(world, players, enemies, tick, environment)
    const providerOwners = deathEffects.filter(effect => effect.kind === 'banish-black').map(effect => ({
      registration: effect.painterRegistration,
      sources: [{ castsDirectionalShadow: true, position: effect.position, radius: 3,
        intensity: Math.min(1, Math.max(0, Math.fround(2 - effect.ageTicks * .004999999888241291))) }],
    }))
    for (const projectile of projectiles.filter(spell => spell.worldKey === 'boneyard:oracle')) {
      providerOwners.push({ registration: projectile.lightRegistration, sources: [projectile.kind === 'earth'
        ? light.nativeBoulderLightSource(projectile) : light.nativeMissileLightSource(projectile, tick)] })
    }
    const miscOwners = secondaryActors.map(actor => ({ birthTick: tick - actor.ageTicks, id: actor.id,
      appendOrdinal: actor.miscLightAppendOrdinal,
      owner: { registration: actor.lightRegistration, sources: [light.nativeSecondaryMiscLightSource(actor)] } }))
    const queries = []
    for (const x of [0, 600, 750, 1000, 1200, 1750, 2000, 2400]) {
      for (const y of [0, 700, 900, 1100, 2500, 3000, 3700]) {
        const position = { x, y }
        queries.push({ position, scalar: query.scalarAt(position), pointGain: query.cameras.reduce((gain, camera) =>
          Math.max(gain, nativeRegionPointGain(position, { x: camera.x + camera.w / 2, y: camera.y + camera.h / 2 }, camera.w, false)), 0) })
      }
    }
    cases.push({ tick, world, providerOwners, miscOwners,
      players: Object.entries(players).map(([id, player], index) => ({ id, position: player.position,
        headingIndex: player.headingIndex, weaponPulse: player.primaryCast.weaponPulse,
        overlayEffectPhase: useDefaults ? 0 : lightings[index].overlayEffectPhase,
        lightRegistration: useDefaults ? null : lightings[index].lightRegistration,
        viewport: useDefaults ? null : { x: inputs[id].viewportWidth, y: inputs[id].viewportHeight } })),
      expected: { cameras: query.cameras, acceptedSources: query.acceptedSources, queries } })
  }
}
const views = []
for (const cameraBounds of [bounds, { x: 25, y: 55, w: 300, h: 250 }]) {
  for (const focus of [{ x: -100, y: -100 }, { x: 1200.123, y: 1600.456 }, { x: 3000, y: 5000 }]) {
    for (const padding of [0, 25]) {
      const viewport = { x: 1600, y: 900 }
      views.push({ bounds: cameraBounds, focus, viewport, scale: 1.35, padding,
        expected: nativePrimaryViewBounds({ bounds: cameraBounds, focus, viewportWidth: viewport.x,
          viewportHeight: viewport.y, scale: 1.35, padding }) })
    }
  }
}
const gains = [0, 1600 / 1.35].flatMap(width => [false, true].flatMap(alternate =>
  [0, 400, 900, 1600].map(x => ({ position: { x, y: 0 }, cameraCenter: { x: 0, y: 0 }, width, alternate,
    expected: nativeRegionPointGain({ x, y: 0 }, { x: 0, y: 0 }, width, alternate) }))))
writeFixture('world-light-goldens.json', 'tools/website-oracle/world-light-goldens.mjs',
  'Website@fa08ae249ff5563570bdb2a653e2ef9358ce7b83/frontend/src/game/core-server/boneyard-world-light.ts; primary-spell-targeting.ts; native-region-point-gain.ts',
  { cases, views, gains })
