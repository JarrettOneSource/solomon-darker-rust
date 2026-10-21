// Full native Fire rows, including shared/private RNG ownership and retirement branches.
import { loadKernel, writeFixture } from './oracle.mjs'
const fire = await loadKernel('core-kernels/primary-spell-fire-effects.ts')
const presentation = await loadKernel('core-kernels/primary-spell-fire-native.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const ownerId = 'owner'
const worldKey = 'boneyard'
const patches = []
for (const nativeType of ['fire', 'goodguy', 'moving']) {
  const moving = nativeType === 'moving'
  const options = { id: 1, burnDamage: 10, damage: 8, nativeType, ownerId, worldKey,
    position: { x: 123.1, y: -456.2 }, ...(moving ? {
      life: 0.03, scale: 0.8, supplementalContact: true,
      velocity: { x: 1.25, y: -0.5 }, velocityMultiplier: { x: 0.95, y: 0.75 },
    } : {}) }
  const born = fire.spawnNativeFirePatch(options, random.createNativeRng(7))
  const frames = [{ tick: 0, patch: born.patch, contact: null }]
  let patch = born.patch
  for (let tick = 1; tick < 300 && patch; tick += 1) {
    const stepped = fire.stepNativeFirePatch(patch, tick)
    patch = stepped.patch
    if ([1, 2, 3, 10, 30].includes(tick) || !patch) frames.push({ tick, ...stepped })
  }
  patches.push({ nativeType, options, rng: born.rng, frames })
}
const detonations = []
for (const kind of ['none', 'immolate', 'imp']) {
  const spentEmber = kind === 'none' ? { kind } : kind === 'immolate'
    ? { kind, damage: 20 } : { kind, damage: 20, lifetimeTicks: 300 }
  const payload = { burnDamage: 10, emberDamage: 8, emberFragments: 4,
    explodeDamage: 12, explodeRadius: 15, privateSeed: 123456, spentEmber }
  const born = fire.createNativeFireDetonation(10, payload, { x: 100, y: 200 }, ownerId, worldKey, random.createNativeRng(900))
  let ember = born.embers[0]
  const frames = [{ tick: 0, ember, retirement: { kind: 'none' } }]
  for (let tick = 1; tick <= 600 && ember; tick += 1) {
    const before = ember
    const stepped = fire.stepNativeFireEmber(ember)
    ember = stepped.ember
    if ([1, 20, 80, 160].includes(tick)) frames.push({ tick, ...stepped })
    if (!ember) {
      if (frames.at(-1).tick !== tick - 1) frames.push({ tick: tick - 1, ember: before, retirement: { kind: 'none' } })
      frames.push({ tick, ...stepped })
    }
  }
  detonations.push({ kind, payload, born, frames })
}
function targets() {
  return [
    { id: 'enemy:a', position: { x: 100, y: 0 }, registrationOrder: 0 },
    { id: 'enemy:b', position: { x: 150, y: 120 }, registrationOrder: 1 },
  ].map(target => ({ ...target, active: true, actorFlags: 2, attachment: { x: 0, y: 0 }, bodyRadius: 15,
    cellBindingOrder: target.registrationOrder, headingDeg: 0, kind: 'enemy', nativePriority: 0,
    pendingRemove: false, queryLane: 'grid' }))
}
const imps = []
for (const mode of ['open', 'blocked', 'empty']) {
  const options = { burnDamage: 10, damage: 12, id: 51, lifetimeTicks: mode === 'empty' ? 4 : mode === 'blocked' ? 400 : 120,
    ownerId, worldKey, position: { x: 0, y: 0 } }
  const born = fire.spawnNativeFireGoodImp(options, random.createNativeRng(7))
  let rng = born.rng
  let goodImp = born.goodImp
  const actors = mode === 'empty' ? [] : targets()
  const frames = [{ tick: 0, goodImp, rng, contact: null, releaseFire: false, releasePosition: goodImp.position }]
  const contacts = []
  for (let tick = 1; tick <= 500 && goodImp; tick += 1) {
    if (tick >= 60 && actors.length) actors[0].active = false
    const stepped = fire.stepNativeFireGoodImp(goodImp, {
      canOccupy: () => mode !== 'blocked', rng, targets: actors,
    })
    goodImp = stepped.goodImp
    rng = stepped.rng
    if (stepped.contact) contacts.push({ tick, ...stepped.contact })
    if ([1, 2, 10, 30, 60, 90, 120, 250, 300].includes(tick) || !goodImp) frames.push({ tick, ...stepped })
  }
  imps.push({ mode, options, frames, contacts })
}
const presentationCases = [1, 2, 51, 1000].map(id => ({ id,
  fade: presentation.nativeFireParticleFadeStep(id), lifetime: presentation.nativeFireParticleLifetimeTicks(id),
  variant: presentation.nativeFireParticleVariant(id), pitch: presentation.nativeFireImpactPitch(id),
  signed: presentation.nativeFirePresentationSignedRandom(id, 7, 3, 0.5),
}))
writeFixture('fire-spell-goldens.json', 'tools/website-oracle/fire-spell-goldens.mjs',
  'Website/frontend/src/game/core-kernels: primary-spell-fire-effects.ts, primary-spell-fire-native.ts, native-enemy-pathfinding.ts',
  { patches, detonations, imps, presentationCases })
