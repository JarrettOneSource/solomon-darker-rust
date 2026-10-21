// Current saves and authentic historical inputs pass through the unmodified public codec.
import { execFileSync } from 'node:child_process'
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { deflateSync } from 'node:zlib'
import { fixturesDir, loadKernel, websiteDir } from './oracle.mjs'
import { withRetainedFlameActors } from './retained-flame-save-state.mjs'
import { withRetainedSecondaryDeathActors } from './retained-secondary-death-save-state.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const saves = await loadKernel('save/game-save-document.ts')
const { createNativeStoneskinWarp } = await loadKernel('core-kernels/native-stoneskin.ts')
const { NATIVE_GENERATED_BONEYARDS } = await loadKernel('host/native-generated-boneyards.ts')
const { STOCK_TUTORIAL_BONEYARD } = await loadKernel('host/stock-tutorial-boneyard.ts')
const editor = await loadKernel('../editor/format/boneyard.ts')
const projection = await loadKernel('host/project-boneyard.ts')
const binaryFixtureBytes = readFileSync(resolve(fixturesDir, 'boneyard-binary-goldens.json'))
const terrainInput = JSON.parse(binaryFixtureBytes).cases.find(test =>
  test.name === 'all authored geometry, exact optional fields and Solomon clutter filtering')
assert.ok(terrainInput)
const authoredTerrain = projection.projectBoneyard(
  editor.parseBoneyard(new Uint8Array(Buffer.from(terrainInput.bytes, 'base64')))).terrain
assert.ok(authoredTerrain.some(terrain => terrain.uid !== undefined
  && terrain.profileSamples?.length > 0 && terrain.sideSign !== undefined))
const cases = [], historicalInputs = []
function record(name, document) {
  try {
    const restored = saves.restoreGameSaveDocument(JSON.stringify(document))
    const options = { integrity: restored.integrity, loadedBoneyard: restored.loadedBoneyard,
      mods: restored.mods, modState: restored.modState, nativeSource: restored.nativeSource,
      playerId: restored.playerId, state: restored.state }
    cases.push({ name, document, accepted: true,
      reencoded: JSON.parse(saves.createGameSaveDocument(options)),
      retired: JSON.parse(saves.retireGameSaveWizard(JSON.stringify(document))),
      gameRng: restored.state.gameRng, combatRng: restored.state.combatRng })
  } catch (error) {
    cases.push({ name, document, accepted: false, error: error.message })
  }
  return cases.at(-1)
}
for (const enhancedEffects of [true, false]) {
  for (const name of ['hub', 'hub-shared', 'tutorial', 'boneyard', 'boneyard-terrain']) {
    const playerId = 'current-save-owner'
    const config = { displayName: 'Current Save', element: 'fire', discipline: 'mind' }
    const participants = { [playerId]: name === 'tutorial'
      ? { ...config, element: 'ether', discipline: 'arcane' } : config }
    if (name === 'hub-shared') participants.friend = { ...config, element: 'water', discipline: 'air' }
    let state = game.createGameSimulation(participants, {
      enhancedEffects, gameRngSeed: 17, combatRngSeed: 91,
    })
    let loadedBoneyard = null
    if (name === 'tutorial' || name.startsWith('boneyard')) {
      const template = name === 'tutorial' ? STOCK_TUTORIAL_BONEYARD : NATIVE_GENERATED_BONEYARDS[0]
      loadedBoneyard = { ...template, choice: {
        id: name === 'tutorial' ? 'stock-tutorial' : 'default-random',
        name: name === 'tutorial' ? 'Tutorial' : 'Random Boneyard', source: 'default',
      }, seed: '00000000000000000000000000000000', runId: `current-save-${name}` }
      if (name === 'boneyard-terrain') {
        loadedBoneyard.scene = { ...template.scene, terrain: authoredTerrain }
        loadedBoneyard.geometrySha256 = projection.boneyardGeometrySha256(loadedBoneyard.scene)
      }
      state = game.enterBoneyardWorld(state, loadedBoneyard)
    }
    for (let tick = 0; tick < 40; tick++) {
      state = game.stepGameSimulationTick(state, Object.fromEntries(Object.keys(participants).map(id => [id, {
        movement: { x: 1, y: 0 }, aim: { x: 1100, y: 1000 },
        cast: { primary: name === 'boneyard', quickbar: null }, viewportWidth: 1600, viewportHeight: 900,
      }])))
    }
    const document = JSON.parse(saves.createGameSaveDocument({ integrity: 'global-clean',
      loadedBoneyard, mods: [], modState: {}, playerId, state }))
    const recorded = record(`current-${name}-effects-${enhancedEffects}`, document)
    if (name === 'boneyard-terrain') {
      assert.equal(recorded.accepted, true, recorded.error)
      assert.deepEqual(recorded.reencoded.continuation.loadedBoneyard.scene.terrain, authoredTerrain)
    }
    if (name === 'hub' || name === 'boneyard') {
      let flameState = withRetainedFlameActors(state, playerId)
      for (let age = 0; age < 2; age++) {
        record(`current-${name}-flame-age-${age}-effects-${enhancedEffects}`,
          JSON.parse(saves.createGameSaveDocument({ integrity: 'global-clean', loadedBoneyard,
            mods: [], modState: {}, playerId, state: flameState })))
        flameState = game.stepGameSimulationTick(flameState, {})
      }
    }
    if (name === 'boneyard') {
      const deathState = withRetainedSecondaryDeathActors(state, playerId)
      const deathDocument = JSON.parse(saves.createGameSaveDocument({ integrity: 'global-clean',
        loadedBoneyard, mods: [], modState: {}, playerId, state: deathState }))
      record(`current-retained-death-effects-${enhancedEffects}`, deathDocument)
      for (const [suffix, mutate] of [
        ['weapon-future-birth', sim => { sim.world.deathWeapons[0].birthTick = sim.tick + 1 }],
        ['weapon-future-age', sim => { sim.world.deathWeapons[0].ageTicks = 1 }],
        ['weapon-life', sim => { sim.world.deathWeapons[0].life = 100000 }],
        ['weapon-settled-motion', sim => { sim.world.deathWeapons[0].motion.bounceVelocity = 0 }],
        ['weapon-duplicate', sim => { sim.world.deathWeapons.push(structuredClone(sim.world.deathWeapons[0])) }],
        ['weapon-allocator', sim => { sim.world.nextDeathWeaponId = sim.world.deathWeapons[0].id }],
        ['weapon-owner', sim => { sim.world.deathWeapons[0].ownerId = '__proto__' }],
        ['golem-fragment-count', sim => { sim.secondaryAbilities.actors.find(a => a.kind === 'golem-death').golemDeath.fragments.pop() }],
        ['golem-fragment-life', sim => { sim.secondaryAbilities.actors.find(a => a.kind === 'golem-death').golemDeath.fragments[0].life = 11 }],
        ['golem-lifetime', sim => { sim.secondaryAbilities.actors.find(a => a.kind === 'golem-death').lifetimeTicks = 133 }],
        ['golem-registration', sim => { const roots = sim.secondaryAbilities.actors.find(a => a.kind === 'golem-death').painterRegistrations; roots[1] = roots[0] }],
        ['drain-duplicate-capture', sim => { const animations = sim.secondaryAbilities.actors.find(a => a.kind === 'ether-drain').etherDrain.animations; animations.push(structuredClone(animations[0])) }],
        ['drain-invalid-art', sim => { sim.secondaryAbilities.actors.find(a => a.kind === 'ether-drain').etherDrain.animations[0].entry = 79 }],
      ]) {
        const changed = structuredClone(deathDocument)
        mutate(changed.continuation.simulation)
        record(`current-${suffix}-effects-${enhancedEffects}`, changed)
      }
    }
    if (name !== 'hub') continue
    const motionDocument = structuredClone(document)
    motionDocument.continuation.simulation.secondaryAbilities.cameraDisplacements = [
      { worldKey: 'hub:courtyard', tick: 40, displacement: { x: 2.25, y: -1.125 } },
      { worldKey: 'hub:library', tick: 41, displacement: { x: 0, y: 0 } },
    ]
    motionDocument.continuation.simulation.secondaryAbilities.stoneskinWarp =
      createNativeStoneskinWarp(state.secondaryAbilities.rng).positions
    record(`current-secondary-motion-effects-${enhancedEffects}`, motionDocument)
    for (const [suffix, mutate] of [
      ['camera-duplicate', secondary => { secondary.cameraDisplacements[1].worldKey = 'hub:courtyard' }],
      ['camera-empty-world', secondary => { secondary.cameraDisplacements[0].worldKey = '' }],
      ['camera-extra-field', secondary => { secondary.cameraDisplacements[0].extra = 0 }],
      ['camera-negative-tick', secondary => { secondary.cameraDisplacements[0].tick = -1 }],
      // The public disk loader retains grid components verbatim; wire admission validates them.
      ['warp-short', secondary => { secondary.stoneskinWarp.pop() }],
      ['warp-too-far', secondary => { secondary.stoneskinWarp[0] = -59 }],
      ['warp-not-float', secondary => { secondary.stoneskinWarp[0] = -64.00000001 }],
    ]) {
      const changed = structuredClone(motionDocument)
      mutate(changed.continuation.simulation.secondaryAbilities)
      record(`current-${suffix}-effects-${enhancedEffects}`, changed)
    }
    const flashDocument = structuredClone(document)
    const flash = { alpha: 0.5, blue: 1, decayPerTick: 0.125, green: 0.5,
      pointAttenuated: true, red: 0.25 }
    flashDocument.continuation.simulation.screenFlashes = { epoch: 2, nextOrder: 4,
      writes: [37, 38, 40].map((tick, index) => ({ order: index + 1, tick,
        worldKey: 'hub:courtyard', position: { x: 1000, y: 700 }, flash,
        onlyIfClear: index === 1 })) }
    record(`current-shared-flashes-effects-${enhancedEffects}`, flashDocument)
    for (const [suffix, mutate] of [
      ['future-tick', flashes => { flashes.writes[2].tick = 41 }],
      ['unordered', flashes => { flashes.writes[1].order = 1 }],
      ['allocator-behind', flashes => { flashes.nextOrder = 3 }],
      ['zero-decay', flashes => { flashes.writes[0].flash.decayPerTick = 0 }],
      ['empty-world', flashes => { flashes.writes[0].worldKey = '' }],
      ['nonboolean-conditional', flashes => { flashes.writes[0].onlyIfClear = 1 }],
    ]) {
      const changed = structuredClone(flashDocument)
      mutate(changed.continuation.simulation.screenFlashes)
      record(`current-flash-${suffix}-effects-${enhancedEffects}`, changed)
    }
    for (const slot of [7, 8, 15, 16, 23]) {
      const bank = structuredClone(document)
      bank.continuation.simulation.playerEntities.belts[0][slot] = { kind: 'health-potion' }
      record(`current-belt-${slot}-effects-${enhancedEffects}`, bank)
    }
    for (const [suffix, mutate] of [
      ['short-belt', player => { player.belts[0].length = 8 }],
      ['bad-belt-rank', player => { player.belts[0][23] = { kind: 'skill', skillId: 11 } }],
      ['bad-second-root', player => { player.skillBooks[0].disciplineRoot = 8 }],
      ['bad-primary-root', player => { player.skillBooks[0].elementRoot = 5 }],
      ['drifted-second-root', player => { player.skillBooks[0].disciplineRoot = 0 }],
      ['precise-heading', player => { player.locomotions[0].headingDegrees = 91.125; player.locomotions[0].headingIndex = 6 }],
      ['bad-heading-index', player => { player.locomotions[0].headingDegrees = 91.125; player.locomotions[0].headingIndex = 8 }],
      ['missing-corpse-consumed', player => { delete player.progressions[0].corpseConsumed }],
      ['living-corpse-consumed', player => { player.progressions[0].corpseConsumed = true }],
    ]) {
      const changed = structuredClone(document)
      mutate(changed.continuation.simulation.playerEntities)
      record(`current-${suffix}-effects-${enhancedEffects}`, changed)
    }
  }
}
for (const file of ['save-document', 'save-active', 'save-migration', 'save-world-migration',
  'save-spell-migration', 'save-scenery-retention']) {
  const bytes = readFileSync(resolve(fixturesDir, `${file}-goldens.json`))
  const fixture = JSON.parse(bytes)
  historicalInputs.push({ file: `${file}-goldens.json`, source: fixture.source,
    sha256: createHash('sha256').update(bytes).digest('hex') })
  for (const test of fixture.cases) {
    if (test.document) record(`historical-${file}-${test.name}`, test.document)
  }
}
const bytes = Buffer.from(JSON.stringify({
  source: 'Current Website save/game-save-document.ts public restore/create/retire',
  sourceRevision: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: websiteDir, encoding: 'utf8' }).trim(),
  generator: 'tools/website-oracle/current-save-continuation-goldens.mjs',
  terrainInput: { file: 'boneyard-binary-goldens.json', case: terrainInput.name,
    sha256: createHash('sha256').update(binaryFixtureBytes).digest('hex') },
  historicalInputs, cases,
}) + '\n')
writeFileSync(resolve(fixturesDir, 'current-save-continuation-goldens.json.zlib'), deflateSync(bytes, { level: 9 }))
writeFileSync(resolve(fixturesDir, 'current-save-continuation-goldens.manifest.json'), JSON.stringify({
  bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex'),
  cases: cases.length, accepted: cases.filter(value => value.accepted).length,
}, null, 2) + '\n')
console.log(`Recorded ${cases.length} current and historical continuation cases`)
