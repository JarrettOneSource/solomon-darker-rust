import { loadKernel, writeFixture } from './oracle.mjs'

const loot = await loadKernel('renderer/native-loot-presentation.ts')
const messages = await loadKernel('loot-message-presentation.ts')
const puppet = await loadKernel('core-kernels/native-puppet-hit.ts')
const base = { id: 17, position: { x: 320, y: 260 }, kind: 'gold', tier: 0,
  alpha: 1, animationPhase: 37.5, rotationDeg: -21, orbKind: 'health', orbValue: 0.8,
  scatterActive: false, scatterSeed: 0xf1234567, scatterProgress: 0,
  bounceHeight: -13.25, itemNativeTypeId: null, itemNativeSubtype: null,
  framePhase: 4.5, bonusKind: null, itemContentId: null,
  painterRegistration: { managerLane: 'actor', registrationOrdinal: 42 } }
const actors = []
const record = actor => actors.push({ actor, plan: loot.nativeLootPresentationPlan(actor),
  painter: loot.nativeLootPainterLayer(actor) })
for (const orbKind of ['health', 'mana']) for (const alpha of [0, 0.2, 0.9999, 1, 1.1]) {
  for (const animationPhase of [-90, 0, 37.5, 90, 180, 270, 400]) {
    record({ ...base, kind: 'orb', orbKind, alpha, animationPhase })
  }
}
for (const tier of [0, 1, 2, 3]) for (const scatterActive of [false, true]) {
  for (const scatterProgress of [-2.5, 0, 3.8, 9, 14]) {
    for (const animationPhase of [90, 270]) record({ ...base, tier, scatterActive, scatterProgress, animationPhase })
  }
}
for (const [itemNativeTypeId, itemNativeSubtype] of [[7001, null], [7001, 0], [7001, 1], [7012, null], [7002, null], [null, null]]) {
  for (const tier of [0, 1, 2, 3]) record({ ...base, kind: 'sack', itemNativeTypeId, itemNativeSubtype, tier })
}
for (const bonusKind of [0, 1, 2]) for (const framePhase of [-1, 0, 7.9, 17, 24]) {
  for (const alpha of [0.1, 1]) record({ ...base, kind: 'bonus', bonusKind, framePhase, alpha })
}
const scatters = []
for (const seed of [0, 1, 0x7fffffff, 0x80000000, 0xffffffff, 0xf1234567]) {
  const actor = { ...base, kind: 'gold', scatterActive: true, tier: 3, scatterSeed: seed }
  let current = seed
  const plans = []
  for (let index = 0; index < 24; index++) {
    const plan = loot.nativeLootPresentationPlan(actor, current)
    current = plan.nextScatterSeed
    plans.push(plan)
  }
  scatters.push({ actor, plans })
}
const goodies = []
for (const phase of [0, 1, 2]) for (const active of [false, true]) {
  for (const timer of [0, 99, 100]) for (const tick of [9, 10, 19, 20, 111]) {
    for (const complexLighting of [false, true]) {
      for (const hit of [null, { strength: 0.75, tick: 10, timer: 0.8 }]) {
        const goodie = { id: 9, position: { x: 300, y: 300 }, phase, active, timer, sceneryRegistrationOrdinal: 6 }
        goodies.push({ goodie, tick, hit, complexLighting,
          hitAlpha: hit ? puppet.nativePuppetHitAlpha(hit, tick) : 0,
          layers: loot.nativeGoodiePresentationPlan(goodie, tick, hit ?? undefined, complexLighting),
          painter: loot.nativeGoodiePainterLayer(goodie) })
      }
    }
  }
}
// A display projection may carry feedback.tick as a fraction; its alpha must
// reach the Goodie renderer without narrowing the clock and sampling it again.
for (const tick of [120.75, 121.25, 121.75]) {
  const goodie = { id: 9, position: { x: 300, y: 300 }, phase: 0,
    active: true, timer: 99, sceneryRegistrationOrdinal: 6 }
  const hit = { strength: .75, tick: 120.75, timer: .8 }
  goodies.push({ goodie, tick, hit, complexLighting: true,
    hitAlpha: puppet.nativePuppetHitAlpha(hit, tick),
    layers: loot.nativeGoodiePresentationPlan(goodie, tick, hit, true),
    painter: loot.nativeGoodiePainterLayer(goodie) })
}
const event = (eventId, tick, text, sound) => ({ eventId, tick, text, sound })
const journeys = []
const actions = [
  { tick: 0 },
  event(1, 1, '12 GOLD', 'pickup-coin'), { tick: 1 },
  event(2, 3, '35 GOLD', 'pickup-coin'), { tick: 3 },
  event(3, 4, 'BONUS SKILL POINT'), { tick: 4 },
  event(4, 4, 'Shield +1'), { tick: 4 },
  event(5, 4, 'DAMAGE x4'), { tick: 5 }, { tick: 18 },
  event(6, 20, '7 GOLD', 'pickup-coin'), { tick: 20 },
  event(7, 110, '2 GOLD', 'pickup-coin'), { tick: 110 },
  event(8, 111, '9007199254740992 GOLD'), { tick: 111 },
  event(9, 112, '9007199254740991 GOLD'), { tick: 112 },
  event(10, 113, '0004 GOLD'), { tick: 113 },
  event(11, 114, undefined), { tick: 130 }, { tick: 129 }, { tick: 300 }, { tick: 600 },
]
const presentation = new messages.NativeLootMessagePresentation(0)
for (const action of actions) {
  if ('eventId' in action) journeys.push({ event: action, accepted: presentation.consume(action) })
  else journeys.push({ tick: action.tick, sample: presentation.sample(action.tick) })
}
writeFixture('loot-presentation-goldens.json', 'tools/website-oracle/loot-presentation-goldens.mjs',
  'Website renderer/native-loot-presentation.ts and loot-message-presentation.ts (pinned ed0a2d59)',
  { actors, scatters, goodies, journeys })
