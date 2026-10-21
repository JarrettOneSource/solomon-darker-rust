// Refresh only the retained Skorcha schedule columns through unchanged current
// Source. Other NPC/student/trader recordings retain their original provenance.
import {execFileSync} from 'node:child_process'
import {readFileSync, writeFileSync} from 'node:fs'
import {resolve} from 'node:path'
import {fixturesDir, loadKernel, websiteDir} from './oracle.mjs'
const source = await loadKernel('core-server/hub-skorcha.ts')
const path = resolve(fixturesDir, 'hub-npc-goldens.json')
const fixture = JSON.parse(readFileSync(path, 'utf8'))
let count = 0
for (const sample of fixture.traderCases) {
  let schedule = source.createHubSkorchaSchedule(sample.seed, {hiddenTicks: 70, visibleTicks: 90})
  for (const row of sample.ticks) {
    if (row.tick > 0) schedule = source.stepHubSkorchaSchedule(schedule, {hiddenTicks: 70, visibleTicks: 90})
    row.skorcha = schedule.skorcha === null ? null : {
      variant: schedule.skorcha.variant, position: schedule.skorcha.position,
      gesture: schedule.skorcha.gesture, hatFrame: source.hubSkorchaHatFrame(schedule.skorcha),
      gestureTicksRemaining: schedule.skorcha.gestureTicksRemaining,
    }
    row.transitionTicksRemaining = schedule.transitionTicksRemaining
    count++
  }
}
fixture.skorchaSourceRevision = execFileSync('git', ['-C', websiteDir, 'rev-parse', 'HEAD'], {encoding:'utf8'}).trim()
fixture.skorchaGenerator = 'tools/website-oracle/current-skorcha-goldens.mjs'
writeFileSync(path, `${JSON.stringify(fixture)}\n`)
console.log(JSON.stringify({schedules:fixture.traderCases.length,ticks:count}))
