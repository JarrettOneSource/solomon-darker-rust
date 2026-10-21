import { loadKernel, writeFixture } from './oracle.mjs'

const source = await loadKernel('renderer/native-screen-feedback.ts')
const attenuation = await loadKernel('core-kernels/native-region-point-gain.ts')
const context = {
  cameraCenter: { x: 0, y: 0 },
  localPlayerAlternate: false,
  visibleWorldWidth: 1000,
}
const flash = (overrides = {}) => ({
  alpha: 1, blue: 1, decayPerTick: Math.fround(0.025), green: 1,
  pointAttenuated: false, red: 1, ...overrides,
})
const event = (eventId, tick, overrides = {}) => ({
  actorId: null, cameraDisplacement: null, cameraMagnitude: 0, cue: null,
  eventId, gain: 1, kind: 'impact', ownerId: 'player', pitch: 1,
  position: { x: 0, y: 0 }, screenFlash: null, skillId: null,
  tick, worldKey: 'boneyard:test', ...overrides,
})
const enemy = (eventId, tick, overrides = {}) => ({
  actorId: 1, eventId, runId: 'test', sourcePosition: { x: 300, y: 0 },
  tick, type: 'enemy-screen-flash', ...overrides,
})
const journeys = []
function journey(name, initialTick, actions, feedbackContext = context) {
  const state = new source.NativeSecondaryScreenFeedbackPresentation(initialTick, 'boneyard:test')
  const samples = []
  for (const action of actions) {
    let result = null
    let error = null
    try {
      switch (action.kind) {
        case 'secondary': state.consume(action.event, feedbackContext); break
        case 'enemy': state.consumeEnemy(action.event, feedbackContext); break
        case 'displacement': state.consumePrimaryCameraDisplacement(action.input); break
        case 'magnitude': state.consumePrimaryCameraMagnitude(action.input); break
        case 'ether': state.consumePrimaryEtherBlast(action.effect, feedbackContext); break
        case 'sample': result = state.sample(action.tick); break
        case 'camera': result = state.sampleCameraDisplacement(action.tick); break
        case 'magnitude-sample': result = state.sampleCameraMagnitude(action.tick); break
        default: throw new Error(`unknown source action ${action.kind}`)
      }
    } catch (caught) {
      error = caught.message
    }
    samples.push({ action, result, error, state: { ...state } })
  }
  journeys.push({ name, initialTick, context: feedbackContext, samples })
}
const sample = tick => ({ kind: 'sample', tick })
const camera = tick => ({ kind: 'camera', tick })
const magnitudeSample = tick => ({ kind: 'magnitude-sample', tick })
const consume = value => ({ kind: 'secondary', event: value })
const consumeEnemy = value => ({ kind: 'enemy', event: value })
const displacement = (eventId, tick, x, y, worldKey = 'boneyard:test') => ({
  kind: 'displacement', input: { eventId, tick, displacement: { x, y }, worldKey },
})
const magnitude = (eventId, tick, value, worldKey = 'boneyard:test') => ({
  kind: 'magnitude', input: { eventId, tick, magnitude: value, worldKey },
})
const ether = (id, birthTick, charges, position, worldKey = 'boneyard:test') => ({
  kind: 'ether', effect: {
    id, birthTick, charges, origin: position, worldKey,
    ageTicks: 0, kind: 'ether-blast', ownerId: 'player',
  },
})
journey('overwrite-dedup-world-order', 10, [
  consume(event(1, 10, { screenFlash: flash({ red: 0.8, blue: 0.8 }) })),
  sample(10),
  consume(event(1, 10, { screenFlash: flash({ red: 0 }) })),
  consume(event(2, 10, { screenFlash: flash({ pointAttenuated: true }), position: { x: 675, y: 0 } })),
  sample(10),
  consume(event(4, 11, { worldKey: 'hub:Courtyard', screenFlash: flash() })),
  consume(event(3, 12, { screenFlash: flash() })),
  sample(12),
  consume(event(5, 8, { screenFlash: flash({ decayPerTick: Math.fround(0.05) }) })),
  sample(12), sample(11), sample(40),
])
for (const initialTick of [0, 15, 100]) {
  for (const decay of [0, Math.fround(0.005), Math.fround(0.025), Math.fround(0.1)]) {
    journey(`flash-decay-${initialTick}-${decay}`, initialTick, [
      consume(event(1, 10, { screenFlash: flash({ decayPerTick: decay }) })),
      ...[10, 11, 15, 20, 39, 40, 49, 50, 100, 200, 201, 400].map(sample),
    ])
  }
}
journey('camera-independent-cursors-and-strict-ties', 20, [
  displacement(1, 18, 6, -8), camera(20),
  displacement(1, 20, 100, 0), camera(20),
  displacement(2, 20, 3, 4), camera(20),
  displacement(3, 20, -5, 0), camera(20),
  magnitude(1, 18, 0.1), magnitudeSample(20),
  magnitude(1, 20, 1), magnitudeSample(20),
  consume(event(1, 20, { cameraMagnitude: 0.5, cameraDisplacement: { x: 0, y: 10 } })),
  camera(20), magnitudeSample(20),
  consume(event(2, 20, { cameraMagnitude: 0 })),
  displacement(4, 20, 100, 100, 'boneyard:other'),
  displacement(3, 20, 100, 0),
  magnitude(3, 20, 100, 'boneyard:other'),
  magnitude(2, 20, 100),
  ...[21, 22, 25, 28, 100, 400].flatMap(tick => [camera(tick), magnitudeSample(tick)]),
])
journey('native-cutoffs-and-color-clamps', 0, [
  displacement(1, 0, 0.5, 0), camera(0), camera(1),
  magnitude(1, 1, Math.fround(0.001)), magnitudeSample(1), magnitudeSample(2),
  magnitude(2, 2, -0.5), magnitudeSample(2), magnitudeSample(3),
  consume(event(1, 3, { screenFlash: flash({ red: -1, green: 0.5, blue: 2 }) })),
  sample(3), sample(4),
])
for (const mode of ['fixed', 'point', 'hit', 'hit-squared']) {
  for (const alternate of [false, true]) {
    journey(`enemy-shake-${mode}-${alternate}`, 0, [
      displacement(1, 10, 3, 4),
      consumeEnemy(enemy(1, 10, { cameraShake: { attenuation: mode, displacement: { x: -6, y: 0 } } })),
      camera(10), camera(11),
      consumeEnemy(enemy(2, 9, { cameraShake: { attenuation: mode, displacement: { x: 0, y: 16 } } })),
      camera(11), camera(30),
    ], { ...context, localPlayerAlternate: alternate })
  }
}
journey('enemy-black-flash-and-clear-only', 15, [
  consumeEnemy(enemy(1, 10, { screenFlash: flash({ red: 0, green: 0, blue: 0, decayPerTick: Math.fround(0.1) }) })),
  sample(15),
  consumeEnemy(enemy(2, 15, { screenFlash: flash(), screenFlashOnlyIfClear: true })),
  sample(15), sample(20),
  consumeEnemy(enemy(3, 20, { screenFlash: flash(), screenFlashOnlyIfClear: true })),
  sample(20),
  consumeEnemy(enemy(4, 20, { runId: 'other', screenFlash: flash({ red: 0 }) })),
  consumeEnemy(enemy(3, 20, { screenFlash: flash({ red: 0 }) })),
  sample(20),
  consumeEnemy(enemy(5, 21, { sourcePosition: { x: 1100, y: 0 }, screenFlash: flash({ pointAttenuated: true }) })),
  sample(21),
])
journey('source-position-contracts', 0, [
  consumeEnemy(enemy(1, 10, { sourcePosition: undefined, cameraShake: { attenuation: 'fixed', displacement: { x: 1, y: 1 } } })),
  consumeEnemy(enemy(2, 10, { sourcePosition: undefined, screenFlash: flash({ pointAttenuated: true }) })),
  consumeEnemy(enemy(3, 10, { sourcePosition: undefined, screenFlash: flash() })),
  sample(10),
])
for (const initialTick of [0, 100]) {
  for (const charges of [0, 1, 4, 16]) {
    journey(`ether-${initialTick}-${charges}`, initialTick, [
      ether(1, 20, charges, { x: 0, y: 0 }), sample(20), magnitudeSample(20),
      ether(1, 20, 100, { x: 0, y: 0 }),
      ether(3, 20, 100, { x: 0, y: 0 }, 'hub:Courtyard'),
      ether(2, 20, 100, { x: 0, y: 0 }),
      sample(21), magnitudeSample(21),
      ether(4, 18, charges, { x: 675, y: 0 }), sample(21), magnitudeSample(21),
      ...[30, 59, 60, 61, 200].flatMap(tick => [sample(tick), magnitudeSample(tick)]),
    ])
  }
}
const gains = []
for (const width of [-10, 0, 0.125, 1000, 1234.567]) {
  for (const alternate of [false, true]) {
    for (const ratio of [0, 0.099999, 0.1, 0.100001, 0.249999, 0.25, 0.5, 0.675, 1.099999, 1.1, 1.100001, 2]) {
      for (const direction of [{ x: 1, y: 0 }, { x: 0.6, y: 0.8 }]) {
        const center = { x: 12.25, y: -7.75 }
        const position = { x: center.x + direction.x * width * ratio, y: center.y + direction.y * width * ratio }
        gains.push({ width, alternate, center, position,
          point: attenuation.nativeRegionPointGain(position, center, width, alternate),
          hit: attenuation.nativeRegionHitPointGain(position, center, width, alternate) })
      }
    }
  }
}
const overlays = []
for (const overlay of [null, { alpha: 1, color: 0xffffff }, { alpha: 0.5, color: 0xe6ffff }]) {
  for (const reduced of [false, true]) {
    overlays.push({ overlay, reduced, result: source.presentNativeSecondaryScreenOverlay(overlay, reduced) })
  }
}
const actors = [
  { kind: 'earthquake', worldKey: 'boneyard:test', velocity: { x: 3, y: 4 } },
  { kind: 'earthquake', worldKey: 'boneyard:test', velocity: { x: -5, y: 0 } },
  { kind: 'earthquake', worldKey: 'boneyard:other', velocity: { x: 100, y: 0 } },
  { kind: 'golem', worldKey: 'boneyard:test', velocity: { x: 100, y: 0 } },
  { kind: 'earthquake', worldKey: 'boneyard:test', velocity: { x: 0, y: -6 } },
]
const shakes = []
for (const count of [0, 1, 2, 3, 4, 5]) {
  for (const current of [{ x: 0, y: 0 }, { x: 3, y: 4 }, { x: 1.23456789, y: -0.1 }]) {
    const selected = actors.slice(0, count)
    shakes.push({ actors: selected, current, result: source.nativeSecondaryWorldShake(selected, 'boneyard:test', current) })
  }
}
writeFixture(
  'screen-feedback-goldens.json', 'tools/website-oracle/screen-feedback-goldens.mjs',
  'Website/frontend/src/game/renderer/native-screen-feedback.ts and core-kernels/native-region-point-gain.ts',
  { journeys, gains, overlays, shakes },
)
