// Preserve the original documents and commands while recording today's public restore,
// hydration, inventory and future-tick results. Historical output is not today's contract.
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fixturesDir, loadKernel, websiteDir, writeFixture } from './oracle.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const saves = await loadKernel('save/game-save-document.ts')
const economy = await loadKernel('core-kernels/hub-economy.ts')
const families = {}, historicalInputs = []
for (const family of ['save-document', 'save-active', 'save-migration',
  'save-world-migration', 'save-spell-migration', 'save-scenery-retention', 'save-profile']) {
  const bytes = readFileSync(resolve(fixturesDir, `${family}-goldens.json`))
  const fixture = JSON.parse(bytes)
  historicalInputs.push({ file: `${family}-goldens.json`, source: fixture.source,
    sha256: createHash('sha256').update(bytes).digest('hex') })
  const cases = []
  for (const old of fixture.cases) {
    const { name, document } = old
    try {
      if (family === 'save-profile') {
        const profile = saves.restoreGameSaveProfile(JSON.stringify(document))
        const fresh = game.createGameSimulation({ [fixture.playerId]: fixture.config }, {
          gameRngSeed: 17, combatRngSeed: 91,
        })
        const hydrated = saves.hydrateGameSaveProfile(fresh, fixture.playerId, profile)
        cases.push({ name, document, accepted: true, economy: profile.economy,
          hagathaRuntime: profile.hagathaRuntime, players: hydrated.playerEntities })
        continue
      }
      const restored = saves.restoreGameSaveDocument(JSON.stringify(document))
      const options = { integrity: restored.integrity, loadedBoneyard: restored.loadedBoneyard,
        mods: restored.mods, modState: restored.modState, nativeSource: restored.nativeSource,
        playerId: restored.playerId, state: restored.state }
      if (old.frames) {
        let state = restored.state, elapsed = 0
        const frames = []
        for (const oldFrame of old.frames) {
          while (elapsed < oldFrame.ticks) {
            state = game.stepGameSimulationTick(state, {})
            elapsed++
          }
          // Keep the actual live continuation, including a later Game Over. Re-encoding
          // it as a checkpoint would reject that phase and mislabel restore as failed.
          frames.push({ ticks: elapsed, simulation: state })
        }
        cases.push({ name, document, accepted: true, frames })
        continue
      }
      if (family === 'save-migration') {
        const state = restored.state
        cases.push({ name, document, accepted: true, restored: {
          playerEntities: state.playerEntities, primarySpells: state.primarySpells,
          secondaryAbilities: state.secondaryAbilities, run: state.run,
          worldManagerOrder: state.worldManagerOrder, gameRng: state.gameRng,
          hubParticipants: state.world.participants,
        } })
        continue
      }
      cases.push({ name, document, accepted: true, restored: restored.state,
        reencoded: JSON.parse(saves.createGameSaveDocument(options)),
        profileDocument: JSON.parse(saves.createGameProfileSaveDocument(options)),
        retired: JSON.parse(saves.retireGameSaveWizard(JSON.stringify(document))),
      })
    } catch (error) { cases.push({ name, document, accepted: false, error: error.message }) }
  }
  const current = { ...fixture, cases }
  if (family === 'save-profile') {
    let state = saves.restoreGameSaveDocument(JSON.stringify(fixture.feedbackDocument)).state
    current.feedbackActions = []
    for (const { command } of fixture.feedbackActions) {
      const result = game.applyGameSimulationHubAction(state, fixture.playerId, command)
      state = result.state
      const owned = game.getPlayerEconomy(state, fixture.playerId)
      current.feedbackActions.push({ command, accepted: result.accepted,
        feedback: owned.actionFeedback, revision: owned.revision, rng: owned.rng })
    }
    current.archivedFeedback = economy.archiveCompletedRunEconomy(
      game.getPlayerEconomy(state, fixture.playerId), {
        displayName: 'Profile Oracle', groundGold: 0, groundItems: [], starterElement: 'fire',
        transferCarriedItems: false,
      }).actionFeedback
  }
  families[family] = current
}
writeFixture('current-save-runtime-goldens.json.zlib',
  'tools/website-oracle/current-save-runtime-goldens.mjs',
  'Current Website public save restore, profile hydration, Hub actions and Game future ticks', {
    sourceRevision: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim(),
    historicalInputs, families,
  })
