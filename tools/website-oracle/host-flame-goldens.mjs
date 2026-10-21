// The actual current native host-history wrapper, spanning independently retained worlds.
import { loadKernel, writeFixture } from './oracle.mjs'
const { createHostFlamePresentationHistory } = await loadKernel('host/flame-presentation-history.ts')
const presentation = await loadKernel('protocol/flame-presentation.ts')
const factory = await loadKernel('core-kernels/native-weld-flame-birth.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const managers = await loadKernel('core-kernels/native-world-manager-order.ts')
function actors(id, tick, worldKey) {
  const order = managers.createNativeWorldManagerOrder({ nextRegistrationOrdinal: { actor: 30, transient: 90 } })
  const start = { x: 100, y: 200 }, end = { x: 500, y: 100 }, control = { x: 300, y: 150 }
  const result = factory.createNativeFlameLashFactory({ channel: { id, tick, buildId: 1003,
    direction: { x: .6, y: -.8 }, origin: start, endpoint: end, midpoint: control, targetId: null,
    underpowered: false, enhancedEffects: true, vector: [1, 2, 3, .5, 0, 0, 0, 0], ownerId: 'flame🧙', worldKey },
    geometry: { start, control, end, offset: { x: 0, y: 0 } }, sourceFadeId: id + 1,
    role: 'main', width: 1, cameraDisplacement: { x: .5, y: -1 },
    creatorRegistration: { managerLane: 'actor', registrationOrdinal: 48 }, miscAppendOrdinal: 0,
    rng: random.createNativeRng(17), registerWorldPainter: order.register })
  return [result.channel, result.source].map(presentation.flamePresentationBirth)
}
const history = createHostFlamePresentationHistory(), states = new Map()
const hub = actors(71, 1, 'hub:courtyard'), run = actors(81, 1, 'boneyard:first')
const inputs = [
  { scope: 'hub', key: 'hub', tick: 0, births: [], baseline: null },
  { scope: 'party:one', key: 'boneyard:first', tick: 0, births: [], baseline: null },
  { scope: 'hub', key: 'hub', tick: 1, births: hub, baseline: { epoch: 1, worldTick: 0 } },
  { scope: 'party:one', key: 'boneyard:first', tick: 1, births: run, baseline: { epoch: 2, worldTick: 0 } },
  { scope: 'party:one', key: 'boneyard:first', tick: 1, births: run.slice(1), baseline: { epoch: 2, worldTick: 1 } },
  { scope: 'hub', key: 'hub', tick: 1, births: [], baseline: { epoch: 1, worldTick: 1 } },
  { scope: 'party:one', key: 'boneyard:first', tick: 10, gameOverTicks: 9, births: run.slice(1), baseline: { epoch: 3, worldTick: 1 } },
  { scope: 'party:one', key: 'boneyard:first', tick: 11, gameOverTicks: 10, births: run.slice(1), baseline: { epoch: 3, worldTick: 1 } },
  { scope: 'hub', key: 'hub', tick: 3, births: [], baseline: { epoch: 4, worldTick: 1 } },
  { scope: 'party:one', key: 'boneyard:next', tick: 11, births: [], baseline: { epoch: 3, worldTick: 1 } },
]
const rows = inputs.map(input => {
  const worldTick = input.tick - (input.gameOverTicks ?? 0)
  const current = { tick: input.tick, run: input.gameOverTicks === undefined ? { phase: 'hub' } : { phase: 'game-over', gameOverTicks: input.gameOverTicks },
    world: input.key === 'hub' ? { kind: 'hub' } : { kind: 'boneyard', runId: input.key.slice(9) },
    primarySpells: { transients: input.births.map(birth => presentation.materializeFlamePresentation(birth, worldTick)).filter(Boolean) } }
  const previous = states.get(input.scope)
  if (previous) history.advance(previous, current)
  states.set(input.scope, current)
  return { input, frame: history.frame(current, input.baseline) }
})
writeFixture('host-flame-goldens.json', 'host-flame-goldens.mjs', 'current host/flame-presentation-history.ts public wrapper: multi-world epochs, same-tick mutations and stopped GameOver clock', { rows })
