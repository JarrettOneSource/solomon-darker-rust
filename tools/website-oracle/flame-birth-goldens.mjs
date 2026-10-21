import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const revision = execFileSync('git', ['-C', websiteDir, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim()
assert.equal(revision, 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83')
const factory = await loadKernel('core-kernels/native-weld-flame-birth.ts')
const fade = await loadKernel('core-kernels/native-weld-flame-lash.ts')
const weld = await loadKernel('core-kernels/native-weld-primary-runtime.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const manager = await loadKernel('core-kernels/native-world-manager-order.ts')
const base = { buildId: 1003, direction: { x: .6, y: -.8 }, id: 71,
  origin: { x: 100.25, y: 201.125 }, ownerId: 'flame-owner', tick: 133,
  endpoint: { x: 800, y: 50 }, midpoint: { x: 550, y: 0 }, targetId: 'enemy:3',
  vector: [1, 2, 3, .5, 0, 0, 0, 0], worldKey: 'boneyard:flame' }
const cases = []
for (const length of ['short', 'long']) for (const role of ['main', 'chain']) {
  for (const enhancedEffects of [true, false]) {
    const seed = enhancedEffects ? 17 : 731
    const channel = { ...base, enhancedEffects, underpowered: role === 'main' && !enhancedEffects }
    const order = manager.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 300, transient: 900 } })
    const geometry = length === 'short'
      ? { start: base.origin, control: { x: 110, y: 200 }, end: { x: 130, y: 200 }, offset: { x: 0, y: role === 'chain' ? -20 : 0 } }
      : { start: base.origin, control: { x: 450.125, y: 100.75 }, end: base.endpoint, offset: { x: 0, y: -20 } }
    const cameraDisplacement = role === 'main' && !channel.underpowered ? { x: .5, y: -1 } : null
    const input = { channel, sourceFadeId: 72, geometry, role, width: role === 'chain' ? Math.fround(.7) : channel.underpowered ? .75 : 1,
      cameraDisplacement, creatorRegistration: { managerLane: 'actor', registrationOrdinal: 48 }, miscAppendOrdinal: role === 'chain' ? 2 : 0 }
    const result = factory.createNativeFlameLashFactory({ ...input, rng: random.createNativeRng(seed), registerWorldPainter: order.register })
    const lights = factory.nativeFlameLashPathLights(result.channel.flameVisual, enhancedEffects)
    cases.push({ name: `${length}-${role}-${enhancedEffects}`, seed, input,
      channel: result.channel, source: result.source, rng: result.rng, order: order.state(), lights })
  }
}
const fades = []
for (const variant of ['source', 'endpoint', 'chain']) for (const enhancedEffects of [true, false]) {
  const order = manager.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 300, transient: 900 } })
  const input = { ...base, variant, enhancedEffects, alpha: variant === 'endpoint' ? .5 : 1 }
  const result = fade.createNativeWeldFlameLashFade({ ...input, rng: random.createNativeRng(731), registerWorldPainter: order.register })
  let actor = result.actor
  const frames = []
  while (actor) {
    frames.push({ actor, light: fade.nativeFlameLashFadeLight(actor) })
    actor = weld.stepNativeWeldWorldActor(actor, result.rng).actor
  }
  fades.push({ input, rng: result.rng, order: order.state(), frames })
}
writeFixture('flame-birth-goldens.json', 'tools/website-oracle/flame-birth-goldens.mjs',
  `Website@${revision}/core-kernels/native-weld-flame-{birth,lash}.ts`, { cases, fades })
