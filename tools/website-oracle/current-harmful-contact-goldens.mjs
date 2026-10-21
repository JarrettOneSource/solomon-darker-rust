#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { loadKernel, websiteDir, writeFixture } from './oracle.mjs'

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
const position = { x: 123.25, y: -54.5 }
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const profiles = [
  { armor: 0, coating: 0, deflect: 0, concentration: null },
  { armor: 2, coating: 1, deflect: 0, concentration: null },
  { armor: 20, coating: 1, deflect: 0, concentration: null },
  { armor: 2, coating: 1, deflect: 100, concentration: 68 },
]
const contacts = []
for (const seed of [1, 44]) {
  for (const spec of profiles) {
    for (const fraction of [0, 0.75, 1]) {
      for (const enhancedEffects of [false, true]) {
        for (const [physicalDamage, magicDamage] of [[10, 0], [0, 10], [10, 10]]) {
          const runtime = { ...created.runtime, concentrationSkillIdA: spec.concentration,
            harden: { armor: spec.armor, coating: spec.coating } }
          const stats = { ...derived, damageResistance: 0.3, magicResistance: 0.2,
            incomingDamageFactor: 1.25, deflectChancePercent: spec.deflect }
          const current = { ...progression, rescueProtection: { ...progression.rescueProtection, fraction } }
          const result = harmful.resolvePlayerHarmfulContact(runtime, stats, current,
            { physicalDamage, magicDamage }, true, random.createNativeRng(seed), position, enhancedEffects)
          // The received damage and the complete chip are separate assertions: rescue
          // scales health/shield lanes without skipping the chip's shared RNG program.
          const { hardenChip, ...damage } = result
          contacts.push({ seed, spec, fraction, enhancedEffects, physicalDamage, magicDamage,
            damage, hardenChip })
        }
      }
    }
  }
}

const breakups = []
for (const seed of [1, 44]) {
  for (const coating of [0.15, 1]) {
    for (const enhancedEffects of [false, true]) {
      for (const blocked of [false, true]) {
        let { effects, rng, nextId, pitch } = harden.createNativeHardenBreakup(coating, position,
          'player', 'boneyard:current-contact', 100, 7, random.createNativeRng(seed), enhancedEffects)
        const initial = { effects, rng, nextId, pitch }
        const samples = []
        for (let tick = 101; tick <= 850; tick += 1) {
          const retained = []
          for (const effect of effects) {
            const result = harden.stepNativeHardenEffect(effect, tick, rng, () => !blocked)
            rng = result.rng
            if (result.effect !== null) retained.push(result.effect)
          }
          effects = retained
          if ([101, 102, 103, 125, 150, 200, 400, 600, 850].includes(tick)) {
            samples.push({ tick, effects, rng })
          }
        }
        breakups.push({ seed, coating, enhancedEffects, blocked, initial, samples })
      }
    }
  }
}
writeFixture('current-harmful-contact-goldens.json', 'current-harmful-contact-goldens.mjs',
  'current core-kernels/player-harmful-contact.ts and native-harden-effects.ts',
  { sourceRevision, contacts, breakups })
