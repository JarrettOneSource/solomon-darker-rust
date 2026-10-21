import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'
import { addWeldVisualSamples } from './weld-visual-rows.mjs'

const weld = await loadKernel('core-kernels/native-weld-primary-runtime.ts')
const lash = await loadKernel('core-kernels/native-weld-flame-lash.ts')
const blizzard = await loadKernel('core-kernels/native-weld-blizzard.ts')
const primary = await loadKernel('core-kernels/primary-spells.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const managers = await loadKernel('core-kernels/native-world-manager-order.ts')
const cases = []
function capture(name, input, actors, rng, order) {
  const frames = [{ tick: 0, actors, rng }]
  for (let tick = 1; actors.length > 0 && tick <= 50; tick += 1) {
    actors = actors.flatMap(actor => {
      const result = weld.stepNativeWeldWorldActor(actor, rng)
      rng = result.rng
      return result.actor ? [result.actor] : []
    })
    if ([1, 2, 3, 4, 19, 20, 37, 50].includes(tick) || actors.length === 0) frames.push({ tick, actors, rng })
  }
  cases.push({ name, input, frames, order: order.state().nextRegistrationOrdinal })
}
const base = { direction: { x: -0.6, y: 0.8 }, id: 300,
  origin: { x: 100.25, y: 200.125 }, ownerId: 'hero', tick: 37,
  vector: [1, 2, 3, 0.5, 1, 2, 0, 0, 0], worldKey: 'boneyard:weld' }
for (const seed of [17, 731]) {
  for (const variant of ['chain', 'endpoint']) {
    for (const alpha of [1, Math.fround(0.2)]) {
      const input = { ...base, seed, family: 'lash', variant, alpha }
      const order = managers.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 500, transient: 70 } })
      const result = lash.createNativeWeldFlameLashFade({ ...input, rng: random.createNativeRng(seed) })
      capture(`lash-${seed}-${variant}-${alpha}`, input, [result.actor], result.rng, order)
    }
  }
  for (const buildId of [1000, 1001, 1002, 1006, 1007, 1008, 1009]) {
    const input = { ...base, seed, buildId, family: 'impact' }
    const order = managers.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 500, transient: 70 } })
    const result = primary.createPrimarySpellWeldImpact(base.id, { ...input, position: base.origin }, base.tick, random.createNativeRng(seed))
    capture(`impact-${seed}-${buildId}`, input, [result.impact], result.rng, order)
  }
  const order = managers.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 500, transient: 70 } })
  const input = { ...base, seed, family: 'glows' }
  const result = blizzard.createNativeWeldBlizzardSourceGlows({ ...input, firstId: base.id, rng: random.createNativeRng(seed) })
  const contact = blizzard.createNativeWeldBlizzardContactGlow({ ...input, id: base.id + 2,
    position: base.origin, rng: result.rng, registerWorldPainter: order.register })
  capture(`glows-${seed}`, input, [...result.actors, contact.actor], contact.rng, order)
  for (const tick of [0, 1, 77]) {
    const order = managers.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 500, transient: 70 } })
    const input = { ...base, seed, tick, family: 'chain', castDirection: { x: 0.8, y: 0.6 } }
    const result = weld.createNativeWeldBlizzardChainEffects({ ...input, firstId: base.id,
      source: base.origin, rng: random.createNativeRng(seed), registerWorldPainter: order.register })
    capture(`chain-${seed}-${tick}`, input, result.actors, result.rng, order)
  }
}
for (const buildId of [1003, 1004]) {
  const input = { ...base, buildId, family: 'channel', endpoint: { x: -40, y: 500 },
    midpoint: { x: 17.5, y: 90 }, targetId: buildId === 1003 ? 'enemy:1' : null, underpowered: buildId === 1004 }
  const order = managers.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 500, transient: 70 } })
  capture(`channel-${buildId}`, input, [weld.createNativeWeldChannelActor(input)], random.createNativeRng(17), order)
}
const geometry = []
for (const birthTick of [0, 1, 77]) {
  for (const underpowered of [false, true]) {
    for (const widen of [0, 0.5]) {
      for (const endpoint of [base.origin, { x: -40, y: 500 }]) {
        const input = { birthTick, endpoint, source: base.origin, underpowered, widen }
        geometry.push({ input, beam: blizzard.nativeWeldBlizzardBeamPlan(input),
          contact: blizzard.nativeWeldBlizzardContactPolygon(input) })
      }
    }
  }
}
for (const sample of cases) addWeldVisualSamples(sample, sample.frames, frame => frame.actors, [0, 3])
writeFixture('weld-effects-goldens.json', 'tools/website-oracle/weld-effects-goldens.mjs',
  'Website/frontend/src/game/core-kernels/native-weld-primary-runtime.ts', {
    sourceRevision: execFileSync('git', ['-C', websiteDir, 'rev-parse', 'HEAD'], {encoding: 'utf8'}).trim(), cases, geometry,
  })
