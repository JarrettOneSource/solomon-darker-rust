import { loadKernel, writeFixture } from './oracle.mjs'
const air = await loadKernel('core-kernels/native-air-presentation.ts')
const earth = await loadKernel('renderer/earth-boulder-presentation.ts')
const ether = await loadKernel('renderer/primary-spell-ether-native.ts')
const fire = await loadKernel('renderer/primary-spell-fire-native.ts')
const effects = await loadKernel('core-kernels/primary-spell-fire-effects.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const point = p => [p.x, p.y]
const sprite = (record, draw, queue = 0) => [draw.alpha, draw.atlas === 'DeadHawg' ? 1 : 0,
  draw.blend === 'add' ? 1 : 0, draw.offset?.x ?? draw.x ?? 0, draw.offset?.y ?? draw.y ?? 0,
  queue, record, draw.rotation ?? (draw.rotationDegrees ?? 0) * Math.PI / 180,
  draw.scaleX ?? draw.scale, draw.scaleY ?? draw.scale, draw.tint]
const plan = (p, draws, overrides = {}) => ({ position: point(p.position), world_y: p.worldY,
  sort_bias: 0, light: p.regionLightPoint ? point(p.regionLightPoint) : null, draws, ...overrides })
const sprites = []
for (const age of [0, 7, 18]) {
  const p = ether.etherPrimaryFlightPlan(7, age, 3, 0.75, age === 7)
  sprites.push({ kind: 'ether', age, plan: plan({ position: { x: 0, y: -10 }, worldY: 0 },
    p.draws.map(d => sprite({ core: 110, ray: 112, spark: 111 }[d.sprite], d))) })
  const impact = ether.etherPrimaryImpactPlan({ id: 7, ageTicks: age, birthTick: 17, origin: { x: 100, y: 200 }, visualScale: 0.75 })
  sprites.push({ kind: 'ether-impact', age, plan: plan(impact, impact.draws.map(d => sprite({ core: 110, ray: 112, spark: 111 }[d.sprite], d))) })
}
for (const charge of [0.18, 0.55, 1.2]) {
  const p = earth.earthBoulderPresentationPlan({ id: 7, ageTicks: 17, charge,
    assemblyCharge: charge - 0.001, shellCharge: charge - 0.002,
    orientation: [1, 0, 0, 0, 1, 0, 0, 0, 1], phase: 'held', flightTicks: 0 }, 17)
  const draws = [
    sprite(15, { ...p.aura, offset: p.visualOffset, tint: 0xe6ffe6 }),
    sprite(86, { ...p.openingFlash, blend: 'add', offset: p.visualOffset, tint: 0xffffff }),
    ...p.rocks.map(r => sprite(r.record, { ...r, alpha: p.bodyAlpha, tint: 0xffffff,
      offset: { x: p.visualOffset.x + r.position.x, y: p.visualOffset.y + r.position.y } })),
  ]
  sprites.push({ kind: 'earth', charge, plan: plan({ position: { x: 100, y: 200 }, worldY: 200,
    regionLightPoint: { x: 100, y: 200 } }, draws, { sort_bias: p.sortBias }) })
}
for (const age of [0, 7, 15]) {
  const state = { id: 7, ageTicks: age, position: { x: 100, y: 200 }, direction: { x: 0.6, y: -0.8 }, underpowered: age === 7 }
  const body = fire.nativeFireballPlan(state, 99)
  sprites.push({ kind: 'fireball', age, plan: plan(body, body.draws.map(d => sprite(d.frame, d))) })
  const impact = fire.nativeFireImpactPlan({ id: 7, ageTicks: age, origin: state.position })
  sprites.push({ kind: 'fire-impact', age, plan: plan(impact, impact.draws.map(d => sprite(d.frame, d))) })
  const particle = fire.nativeFireParticlePlan({ id: 7, ageTicks: age, origin: state.position, direction: state.direction, variant: 3 })
  sprites.push({ kind: 'fire-particle', age, plan: plan(particle, [sprite(particle.frame, { ...particle, blend: 'add' })]) })
}
for (const age of [0, 10, 35]) {
  const p = fire.nativeFireExplosionPlan({ ageTicks: age, id: 7, origin: { x: 100, y: 200 }, visualScale: 1.9,
    presentation: age === 10 ? 'steam' : 'fire' }, 0.7)
  sprites.push({ kind: 'fire-explosion', age, plan: plan(p, p.draws.map(d => sprite(d.entry, d,
    d.role === 'explosion-core' ? 2 : d.role === 'explosion-array' ? 1 : 0))) })
}
for (const age of [0, 7]) {
  const p = fire.nativeFireEmberPlan({ id: 7, ageTicks: age, height: -20, life: age === 0 ? 2 : 0.4,
    phase: 1.5, position: { x: 100, y: 200 } }, 99)
  sprites.push({ kind: 'fire-ember', age, plan: plan(p, p.draws.map(d => sprite(d.entry, d))) })
}
for (const phase of [1.5, 2.5]) {
  const state = { id: 7, ageTicks: 10, position: { x: 100, y: 200 }, life: 1.5,
    scale: 0.8, atlasPhase: phase, horizontalSign: -1, drawAlpha: 1, fadeAlpha: 0.7 }
  const patch = fire.nativeFirePatchPlan(state)
  const glow = fire.nativeFireGroundGlowPlan(state, 99)
  sprites.push({ kind: 'fire-patch', phase, plan: plan(patch, [
    sprite(15, { ...glow, offset: { x: glow.position.x - patch.position.x, y: glow.position.y - patch.position.y } }, 1),
    sprite(patch.entry, patch),
  ]) })
}
const imp = { ...effects.spawnNativeFireGoodImp({ burnDamage: 10, damage: 12, id: 7, lifetimeTicks: 300,
  ownerId: 'view', worldKey: 'boneyard', position: { x: 100, y: 200 } }, random.createNativeRng(7)).goodImp,
  headingDegrees: 253.4, verticalOffset: -20, effectAlpha: 0.7, effectPhase: 9.2,
  contactAgeTicks: 5, contactOrigin: { x: 105, y: 190 } }
const impPlan = fire.nativeFireGoodImpPlan(imp)
sprites.push({ kind: 'fire-good-imp', plan: plan(impPlan, impPlan.draws.map(d => sprite(d.entry, d))) })

const pairs = values => Array.from({ length: values.length / 2 }, (_, i) => [values[i * 2], values[i * 2 + 1]])
const mesh = p => ({ indices: Array.from(p.indices), uvs: pairs(p.uvs), vertices: pairs(p.vertices) })
const light = p => p === null ? null : [p.castsDirectionalShadow, p.intensity, point(p.position), p.radius]
const corona = p => p === null ? null : ({ alpha: p.alpha, center: point(p.center),
  draws: [...p.circles.map(d => sprite(d.record, { ...d, blend: 'add', rotation: 0 })), ...p.forks.map(d => sprite(d.record, { ...d, blend: 'add' }))] })
const airCases = []
for (const endpoint of [{ x: 80, y: 10 }, { x: 400, y: -300 }, { x: -200, y: 500 }]) {
  for (const age of [0, 1, 2, 4]) for (const underpowered of [false, true]) {
    const input = { ageTicks: age, birthTick: 17, endpoint, id: 7,
      midpoint: { x: endpoint.x * 0.45 + 3, y: endpoint.y * 0.45 - 5 }, underpowered }
    const p = air.buildNativeAirLightningPlan(input)
    airCases.push({ input, plan: {
      body: p.body?.layers.map(layer => ({ alpha: layer.alpha, mesh: mesh(layer), parameters: Array.from(layer.parameterSamples),
        phase: layer.phaseDegrees, offset: layer.phaseOffset, record: layer.textureRecord, tint: layer.tint, width: layer.width,
        branch: layer.branch ? { geometry_record: layer.branch.geometryRecord, mirror_x: layer.branch.mirrorX,
          scale: layer.branch.scale, texture_record: layer.branch.textureRecord, mesh: mesh(layer.branch) } : null })) ?? null,
      source: corona(p.sourceCorona), contact: corona(p.contactCorona), light: light(p.contactLight),
    } })
  }
}
const pathLights = []
for (const weak of [false, true]) {
  const origin = { x: 100, y: 200 }, midpoint = { x: 400, y: 350 }, endpoint = { x: 700, y: 800 }
  pathLights.push({ weak, lights: air.buildNativeAirPathLightSources({ id: 7, birthTick: 17, origin, midpoint, endpoint, weakCast: weak }).map(light) })
}
writeFixture('primary-presentation-goldens.json', 'tools/website-oracle/primary-presentation-goldens.mjs',
  'Website native-air-presentation.ts; renderer/primary-spell-{ether,fire}-native.ts, earth-boulder-presentation.ts', { sprites, airCases, pathLights })
