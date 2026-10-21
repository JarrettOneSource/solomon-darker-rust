// Real portable import retains both floating thresholds before ordinary XP resumes.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, writeFixture } from './oracle.mjs'

const portable = await loadKernel('save/portable-game-profile.ts')
const portability = await loadKernel('save/game-save-portability.ts')
const saves = await loadKernel('save/game-save-document.ts')
const progression = await loadKernel('core-kernels/player-progression.ts')
const profileFixture = JSON.parse(readFileSync(resolve(fixturesDir, 'portable-game-profile-goldens.json'), 'utf8'))
const progressionFixture = JSON.parse(readFileSync(resolve(fixturesDir, 'progression-goldens.json'), 'utf8'))
const fields = Object.keys(progressionFixture.offers[0].state)
const project = value => Object.fromEntries(fields.map(key => [key, value[key]]))
const cases = []
for (const [name, nextThreshold, previousThreshold] of [
  ['stock integer thresholds', 90, 0],
  ['fractional imported thresholds', 90.25, 1.5],
  ['inverted imported thresholds', 10, 20],
  ['equal fractional imported thresholds', 25.5, 25.5],
]) {
  const input = JSON.parse(profileFixture.baseDocument)
  Object.assign(input.wizard, { nextThreshold, previousThreshold, experience: 42.125 })
  const document = JSON.stringify(input)
  const parsed = await portable.parsePortableGameProfile(document)
  const imported = portability.createWebGameSaveFromPortableProfile(parsed)
  const restored = saves.restoreGameSaveDocument(imported.document)
  let state = restored.state.playerEntities.progressions[0]
  const book = restored.state.playerEntities.skillBooks[0]
  let rng = restored.state.gameRng
  const initial = { state: project(state), book, rng, progress: progression.playerExperienceProgress(state) }
  const records = []
  for (const amount of [0, 1, 100]) {
    const result = progression.grantPlayerExperience(state, book, amount, rng)
    state = result.progression
    rng = result.rng
    records.push({ amount, state: project(state), rng, progress: progression.playerExperienceProgress(state) })
  }
  cases.push({ name, portableDocument: document, initial, records })
}
writeFixture('progression-import-threshold-goldens.json', 'tools/website-oracle/progression-import-threshold-goldens.mjs',
  'Website save/game-save-portability.ts import and core-kernels/player-progression.ts', { cases })
