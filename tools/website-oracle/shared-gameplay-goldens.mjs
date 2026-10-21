import {loadKernel, writeFixture} from './oracle.mjs'

const pause = await loadKernel('pause-menu-contract.ts')
const resume = await loadKernel('gameplay-resume-grace.ts')
const pauses = []
for (const source of ['inventory', 'pause-menu', 'skill-book', 'skill-selector']) {
  for (const localPlayerId of ['owner', 'peer']) {
    const state = {ownerPlayerId: 'owner', ownerDisplayName: 'Felix', source}
    pauses.push({state, localPlayerId, presentation: pause.gameplayPausePresentation(state, localPlayerId)})
  }
}
const progress = []
for (const remainingMs of [null, 0, 600, 2000, 3000]) {
  for (const elapsedMs of [-10, 0, 100, 800, 2000, 4000]) {
    const grace = {reason: 'pause-menu-closed', remainingMs, sequence: 1}
    progress.push({remainingMs, elapsedMs, progress: resume.gameplayResumeGraceProgress(grace, elapsedMs)})
  }
}
writeFixture(
  'shared-gameplay-goldens.json',
  'tools/website-oracle/shared-gameplay-goldens.mjs',
  ['pause-menu-contract.ts', 'gameplay-resume-grace.ts'],
  {pauses, progress},
)
