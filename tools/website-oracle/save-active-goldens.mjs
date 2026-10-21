// Accepted saves with live actors, produced through the unmodified public Game API.
// A deliberately empty authored arena isolates actor interchange from encounter admission.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { loadKernel, writeFixture } from './oracle.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const saves = await loadKernel('save/game-save-document.ts')
const { boneyardGeometrySha256 } = await loadKernel('host/project-boneyard.ts')
const playerId = 'active-owner'
const scene = { name: 'Save Actor Arena', environmentMode: 0,
  bounds: { x: 0, y: 0, w: 2000, h: 2000 }, spawn: { x: 1000, y: 1000, facingDeg: 0 },
  objects: [], sprites: [], roads: [], fences: [], terrain: [], solomonDig: null }
const loadedBoneyard = { scene, choice: { id: 'save-arena', name: scene.name, source: 'default' },
  sourceSha256: createHash('sha256').update(JSON.stringify(scene)).digest('hex'),
  geometrySha256: boneyardGeometrySha256(scene), seed: '00000000000000000000000000000000', runId: 'save-actors' }
const cases = []
for (const element of ['fire', 'water', 'air', 'ether', 'earth']) {
  const config = { displayName: 'Actor Save Oracle', element, discipline: 'mind' }
  let state = game.enterBoneyardWorld(game.createGameSimulation({ [playerId]: config },
    { gameRngSeed: 17, combatRngSeed: 91 }), loadedBoneyard)
  let births = 0
  for (let tick = 0; tick < 300; tick += 1) {
    state = game.stepGameSimulationTick(state, { [playerId]: {
      movement: { x: 0, y: 0 }, aim: { x: 1000, y: 1300 },
      cast: { primary: true, quickbar: null }, viewportWidth: 1600, viewportHeight: 900,
    } })
    if (!state.primarySpells.projectiles.length && !state.primarySpells.transients.length) continue
    if (births++ !== 0 && tick !== 100) continue
    const options = { integrity: 'global-clean', loadedBoneyard, mods: [], modState: {}, playerId, state }
    const document = saves.createGameSaveDocument(options)
    const restored = saves.restoreGameSaveDocument(document)
    cases.push({ name: `${element}-${tick}`, document: JSON.parse(document), restored: restored.state,
      reencoded: JSON.parse(saves.createGameSaveDocument({ ...options, state: restored.state,
        loadedBoneyard: restored.loadedBoneyard })) })
    if (tick === 100) break
  }
  assert(births > 0, `${element} must create a live spell actor`)
}
writeFixture('save-active-goldens.json', 'tools/website-oracle/save-active-goldens.mjs',
  'Website/frontend/src/game/save/game-save-document.ts active elemental actor create/restore', { cases })
