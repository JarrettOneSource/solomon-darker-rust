#!/usr/bin/env node
// Run lifecycle transitions, including the native input and automatic Game Over gates.
import { loadKernel, writeFixture } from './oracle.mjs'

const run = await loadKernel('core-kernels/game-run.ts')
const presentation = await loadKernel('game-over-presentation.ts')
const audio = await loadKernel('game-over-audio.ts')

function scenario(name, actions) {
  let state = run.createGameRunLifecycle()
  const records = []
  for (const action of actions) {
    let accepted = true
    switch (action.kind) {
      case 'start':
        state = run.startGameRun(state, action.runId, action.players)
        break
      case 'synchronize':
        state = run.synchronizeGameRunParticipants(state, action.players)
        break
      case 'step':
        for (let tick = 0; tick < action.count; tick++) {
          state = run.stepGameRunLifecycle(state, new Set(action.alive))
        }
        break
      case 'continue': {
        const next = run.continueGameOver(state, action.runId, action.eventId)
        accepted = next !== null
        if (next !== null) state = next
        break
      }
      case 'confirm': {
        const next = run.confirmPostRunLoadout(state, action.player)
        accepted = next !== null
        if (next !== null) state = next
        break
      }
      case 'college-intro': {
        const next = run.continuePostRunToCollegeIntro(state)
        accepted = next !== null
        if (next !== null) state = next
        break
      }
    }
    records.push({ action, accepted, state })
  }
  return { name, records }
}

const step = (count, alive = []) => ({ kind: 'step', count, alive })
const start = (runId, players) => ({ kind: 'start', runId, players })
const resume = (runId, eventId) => ({ kind: 'continue', runId, eventId })
const scenarios = [
  scenario('input-gate-and-loadout', [
    { kind: 'college-intro' }, step(2), start('first-run', ['b', 'a', 'b']),
    step(2, ['b']), { kind: 'synchronize', players: ['a'] }, step(1, ['b']),
    step(499), resume('first-run', 1), step(1), resume('wrong-run', 1),
    resume('first-run', 2), resume('first-run', 1), resume('first-run', 1),
    step(19), step(1), { kind: 'confirm', player: 'b' }, { kind: 'confirm', player: 'a' },
    start('second-run', ['a']), { kind: 'synchronize', players: [] }, step(20),
  ]),
  scenario('automatic-gate-and-party-loadout', [
    start('automatic', ['b', 'a']), step(1), step(950), step(1),
    step(249), step(1), { kind: 'confirm', player: 'a' }, { kind: 'confirm', player: 'a' },
    { kind: 'synchronize', players: ['a'] },
  ]),
  scenario('post-run-college-intro', [
    start('intro', ['a']), step(1), step(1201), { kind: 'college-intro' },
  ]),
]

const presentationTicks = [0,1,39,40,200,201,202,268,300,301,400,401,499,500,549,550,600,820,821,822,920,921,922,950,951,1201]
const presentations = presentationTicks.flatMap(tick =>
  [[null,null],[1,'input'],[20,'input'],[1,'automatic'],[250,'automatic']].map(([exitTick,exitKind]) =>
    ({tick,exitTick,exitKind,value:presentation.gameOverPresentation(tick,exitTick,exitKind)})))
const riff = Array.from({length:952},(_,tick)=>({tick,value:presentation.solomonRiffPresentation(tick)}))
let previous = run.startGameRun(run.createGameRunLifecycle(),'game-over-audio',['local'])
const audioEvents = []
for(let tick=0;tick<1202;tick++) {
  const current=run.stepGameRunLifecycle(previous,new Set())
  const cues=audio.gameOverAudioEvents(previous,current)
  if(cues.length) audioEvents.push({tick,cues})
  previous=current
}
writeFixture('game-run-goldens.json', 'tools/website-oracle/game-run-goldens.mjs',
  ['Website/frontend/src/game/core-kernels/game-run.ts','Website/frontend/src/game/game-over-presentation.ts','Website/frontend/src/game/game-over-audio.ts'],
  { scenarios, presentations, riff, audioEvents })
