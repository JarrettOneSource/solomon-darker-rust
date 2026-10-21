import { loadKernel, writeFixture } from './oracle.mjs'
const harmful = await loadKernel('core-kernels/player-harmful-contact.ts')
const harden = await loadKernel('core-kernels/native-harden-effects.ts')
const profile = await loadKernel('core-kernels/player-progression.ts')
const skills = await loadKernel('core-kernels/player-skill-runtime.ts')
const economy = await loadKernel('core-kernels/hub-economy.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const book = profile.createPlayerSkillBook({ element: 'ether', discipline: 'arcane', displayName: 'Contact' })
const money = economy.createHubEconomy(77, { starterElement: 'ether' })
const created = skills.createPlayerSkillRuntime(book, profile.playerStatBook(), money)
const progression = profile.createPlayerProgression(1)
const derived = skills.playerSkillDerivedStats(created.runtime, created.skillBook, profile.playerStatBook(), progression, money)
const profiles = [
  { armor: 0, coating: 0, concentration: null, mindChug: 0, deflect: 0, resistance: 0, magic: 0, incoming: 1 },
  { armor: 2, coating: Math.fround(0.2), concentration: null, mindChug: 0, deflect: 9.99, resistance: 0.3, magic: 0.2, incoming: 0.75 },
  { armor: 20, coating: Math.fround(0.21), concentration: null, mindChug: 0, deflect: 0, resistance: 0.3, magic: 0.2, incoming: 1.25 },
  { armor: 2, coating: 1, concentration: 68, mindChug: 0, deflect: 100, resistance: 0.9, magic: 0.9, incoming: 1 },
  { armor: 2, coating: 1, concentration: null, mindChug: 1, deflect: 50, resistance: 0.3, magic: 0.2, incoming: 1 },
]
const position = { x: 123.25, y: -54.5 }
const contacts = []
for (const seed of [1, 15, 44, 121, 999]) for (const spec of profiles) {
  for (const [physicalDamage, magicDamage] of [[0, 0], [10, 0], [0, 10], [10, 10], [0.05, 0.03]]) for (const inRange of [false, true]) {
    const runtime = { ...created.runtime, concentrationSkillIdA: spec.concentration, harden: { armor: spec.armor, coating: spec.coating } }
    const stats = { ...derived, damageResistance: spec.resistance, magicResistance: spec.magic, incomingDamageFactor: spec.incoming, deflectChancePercent: spec.deflect }
    const result = harmful.resolvePlayerHarmfulContact(runtime, stats, { mindChugTicksRemaining: spec.mindChug }, { physicalDamage, magicDamage }, inRange, random.createNativeRng(seed), position)
    contacts.push({ seed, spec, physicalDamage, magicDamage, inRange, result })
  }
}
const flashes = []
for (const seed of [...Array.from({ length: 32 }, (_, index) => index), 121]) for (const chance of [0, 8.99, 9.01, 50, 100]) {
  flashes.push({ seed, chance, result: harmful.resolvePlayerFlashResponse({ flashChancePercent: chance, flashDurationTicks: 400 }, random.createNativeRng(seed)) })
}
const breakups = []
for (const seed of [1, 15, 44]) for (const coating of [0.1, Math.fround(0.1), 0.15, 0.25, 0.35, 0.45, 1]) for (const blocked of [false, true]) {
  let { effects, rng, nextId, pitch } = harden.createNativeHardenBreakup(coating, position, 'player', 'boneyard:oracle', 100, 7, random.createNativeRng(seed))
  const initial = { effects, rng, nextId, pitch }
  const samples = []
  for (let tick = 101; tick <= 850; tick++) {
    const retained = []
    for (const effect of effects) {
      const stepped = harden.stepNativeHardenEffect(effect, tick, rng, () => !blocked)
      rng = stepped.rng
      if (stepped.effect) retained.push(stepped.effect)
    }
    effects = retained
    if ([101, 102, 103, 110, 125, 150, 200, 400, 600, 850].includes(tick)) samples.push({ tick, effects, rng })
  }
  breakups.push({ seed, coating, blocked, initial, samples })
}
const ranges = [74.99, 75, 75.01].map(x => ({ x, result: harmful.playerDeflectReflectionSourceInRange({ x: 0, y: 0 }, 25, { x, y: 0 }, 25) }))
writeFixture('harmful-goldens.json', 'tools/website-oracle/harmful-goldens.mjs',
  'Website/frontend/src/game/core-kernels/player-harmful-contact.ts + native-harden-effects.ts', { contacts, flashes, breakups, ranges })
