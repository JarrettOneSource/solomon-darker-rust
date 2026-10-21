import { readFileSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const source = await loadKernel('renderer/player-staff-vfx-presentation.ts')
const actions = JSON.parse(readFileSync(resolve(fixturesDir, 'staff-action-goldens.json')))
const effects = []
for (const value of actions.presentations) for (const journey of value.vfxJourneys) {
  for (const actor of journey.samples) {
    if (actor) effects.push({ actor, plan: source.nativePlayerStaffVfxRenderPlan(actor) })
  }
}
const pikes = []
const snapshots = actions.physical.flatMap(value => value.result.impacts
  .map(impact => impact.pikeBreakPresentationRng).filter(Boolean)).slice(0, 12)
for (const presentationRng of snapshots) for (const enhancedShadow of [false, true]) {
  for (const ageTicks of [0, 1, 3, 10, 25, 39, 40, 49, 50, 75, 98, 99, 100]) {
    const actor = { ...actions.pikeBreak[0], ageTicks, enhancedShadow, presentationRng }
    pikes.push({ actor, draws: source.nativePlayerStaffPikeBreakDraws(actor) })
  }
}
writeFixture('staff-presentation-goldens.json', 'tools/website-oracle/staff-presentation-goldens.mjs',
  'Website/frontend/src/game/renderer/player-staff-vfx-presentation.ts', {
    sourceRevision: execFileSync('git', ['-C', websiteDir, 'rev-parse', 'HEAD'], {encoding:'utf8'}).trim(), effects, pikes,
  })
