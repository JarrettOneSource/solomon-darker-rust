#!/usr/bin/env node
// Reuse retained public-kernel INPUTS only; every expected tick runs current unmodified Source.
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'
const golem = await loadKernel('core-kernels/native-secondary-golem.ts')
const random = await loadKernel('core-kernels/native-rng.ts')
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim()
const inputs = JSON.parse(readFileSync(resolve(fixturesDir, 'secondary-summon-goldens.json'), 'utf8')).journeys
const journeys = []
for (const input of inputs) {
  const { name, initial, mode, epochs, seed } = input
  let actor = structuredClone(initial)
  let rng = random.createNativeRng(seed)
  let tick = 0
  const records = []
  for (const epoch of epochs) for (let local = 0; local < epoch.ticks; local += 1) {
    tick += 1
    const result = golem.stepNativeSecondaryGolem(actor, { ownerPosition: epoch.owner, rng,
      resolveFootTarget: (current, requested) => mode === 'wall' && requested.x > 120 ? current : requested,
      resolveMovement: requested => mode === 'wall' && requested.x > 120 ? actor.position : requested,
      targets: epoch.targets })
    actor = result.actor; rng = result.rng
    if (tick === 1 || tick % 25 === 0 || result.assemblyMilestone !== null || result.contact !== null
      || result.footstep || result.provokeStarted || local === epoch.ticks - 1) records.push({ tick, ...result })
  }
  journeys.push({ name, initial, mode, epochs, seed, records })
}
writeFixture('current-golem-contact-goldens.json', 'tools/website-oracle/current-golem-contact-goldens.mjs',
  'current native-secondary-golem.ts attack origin/contact cone/heading and movement budget over independently retained input journeys',
  { sourceRevision, journeys })
