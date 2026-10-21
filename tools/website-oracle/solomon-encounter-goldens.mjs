#!/usr/bin/env node
// Exact opening encounter state, with sparse checkpoints and every phase/audio transition.
import { loadKernel, writeFixture } from './oracle.mjs'

const encounter = await loadKernel('core-kernels/boneyard-encounter.ts')
const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const tutorial = await loadKernel('core-kernels/native-tutorial.ts')
const dig = NATIVE_GENERATED_BONEYARDS[0].scene.solomonDig

function scenario(seed, mode = 'ordinary', contact = true, totalTicks = 2200) {
  const options = mode === 'tutorial'
    ? { dialogueMode: mode, tutorialDialogueTicks: tutorial.nativeTutorialDialogueTicks() }
    : {}
  let state = encounter.createSolomonEncounter(dig, seed, options)
  const records = [{ tick: 0, state }]
  for (let tick = 1; tick <= totalTicks; tick++) {
    const players = contact ? {
      local: { position: { x: dig.position.x + 30, y: dig.position.y + 50 } },
    } : {}
    const next = encounter.stepSolomonEncounter(state, players, tick)
    if (tick % 25 === 0 || next.phase !== state.phase || next.digEventId !== state.digEventId
      || next.voiceEvents.length !== state.voiceEvents.length || tick === totalTicks) {
      records.push({ tick, state: next })
    }
    state = next
  }
  return { seed, mode, contact, totalTicks, tutorialDialogueTicks: options.tutorialDialogueTicks ?? 0, dig, records }
}

const scenarios = [scenario('dig-without-contact', 'ordinary', false, 1800)]
const helloCues = new Set()
for (let seed = 0; helloCues.size < 4 && seed < 100; seed++) {
  const candidate = scenario(`ordinary-${seed}`)
  const cue = candidate.records.find(record => record.state.voiceEvents.length > 0)?.state.voiceEvents[0].cue
  if (cue && !helloCues.has(cue)) {
    helloCues.add(cue)
    scenarios.push(candidate)
  }
}
if (helloCues.size !== 4) throw new Error('encounter oracle did not cover every hello cue')
scenarios.push(scenario('tutorial', 'tutorial', true, 4200))

writeFixture('solomon-encounter-goldens.json', 'tools/website-oracle/solomon-encounter-goldens.mjs',
  'Website/frontend/src/game/core-kernels/boneyard-encounter.ts; native-tutorial.ts::nativeTutorialDialogueTicks',
  { scenarios })
