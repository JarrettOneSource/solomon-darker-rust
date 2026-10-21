// The source owns three independent retained tables, not one rectangle. Exercise
// public save restore before and after the actual world cleanup epoch.
import { createHash } from 'node:crypto'
import { loadKernel, writeFixture } from './oracle.mjs'

const game = await loadKernel('core-server/game-simulation.ts')
const saves = await loadKernel('save/game-save-document.ts')
const arena = await loadKernel('core-kernels/boneyard-arena-transition.ts')
const { boneyardGeometrySha256 } = await loadKernel('host/project-boneyard.ts')
const playerId = 'retained-scenery-owner'
const scene = {
  name: 'Retained Scenery Arena', environmentMode: 0,
  bounds: { x: 0, y: 0, w: 2000, h: 2000 },
  spawn: { x: 1000, y: 1000, facingDeg: 0 },
  objects: [
    { eid: 'tree-a', typeId: 2001, pos: { x: 300, y: 300 } },
    { eid: 'tree-b', typeId: 2001, pos: { x: 900, y: 300 } },
    { eid: 'tree-c', typeId: 2001, pos: { x: 300, y: 900 } },
    { eid: 'tree-d', typeId: 2001, pos: { x: 900, y: 1800 } },
    { eid: 'grave-a', typeId: 2029, pos: { x: 200, y: 1750 } },
    { eid: 'grave-b', typeId: 2029, pos: { x: 800, y: 850 } },
    { eid: 'monument', typeId: 2040, pos: { x: 1100, y: 600 } },
  ],
  fences: [{ eid: 'north-fence', typeId: 2003,
    points: [{ x: 100, y: 100 }, { x: 200, y: 100 }] }],
  sprites: [], roads: [], terrain: [], solomonDig: null,
}
const loaded = {
  scene, choice: { id: 'retained-scenery-arena', name: scene.name, source: 'default' },
  sourceSha256: createHash('sha256').update(JSON.stringify(scene)).digest('hex'),
  geometrySha256: boneyardGeometrySha256(scene),
  seed: '00000000000000000000000000000000', runId: 'retained-scenery-arena',
}
const original = game.enterBoneyardWorld(game.createGameSimulation({ [playerId]: {
  displayName: 'Retained Scenery Oracle', element: 'earth', discipline: 'body',
} }, { gameRngSeed: 17, combatRngSeed: 91 }), loaded)
const cases = []

function record(name, state, version = 37) {
  const document = JSON.parse(saves.createGameSaveDocument({
    state, playerId, loadedBoneyard: loaded, integrity: 'global-clean', mods: [], modState: {},
  }))
  document.schemaVersion = version
  const restored = saves.restoreGameSaveDocument(JSON.stringify(document))
  state = restored.state
  const frames = [{ ticks: 0, simulation: state }]
  for (let tick = 1; tick <= 4; tick++) {
    state = game.stepGameSimulationTick(state, {})
    if ([1, 4].includes(tick)) frames.push({ ticks: tick, simulation: state })
  }
  cases.push({ name, document, accepted: true, frames })
}

record('current-complete-authored-tables', original)
const selected = structuredClone(original)
selected.world.earthquakeSceneryTargets = selected.world.earthquakeSceneryTargets
  .filter(target => [0, 2, 4, 6].includes(target.id))
selected.world.primarySceneryTargets = selected.world.primarySceneryTargets
  .filter(target => ['scenery:tree-a', 'scenery:tree-d', 'scenery:grave-b'].includes(target.id))
selected.world.scenerySpellTargets = selected.world.scenerySpellTargets
  .filter(target => target.id === 'scenery:grave-a')
record('current-independent-retained-tables', selected)
record('legacy-nonrectangular-primary-intersection-keeps-fenceposts', selected, 33)

let transition = arena.startBoneyardArenaTransition(
  arena.createBoneyardArenaTransition(scene.bounds, scene.spawn),
)
for (let tick = 0; tick < 400; tick++) transition = arena.stepBoneyardArenaTransition(transition)
const sealed = { ...original, world: { ...original.world, arenaTransition: transition } }
record('sealed-tables-retire-on-the-next-world-epoch', sealed)
record('sealed-cleanup-survives-restoration', game.stepGameSimulationTick(sealed, {}))

writeFixture('save-scenery-retention-goldens.json',
  'tools/website-oracle/save-scenery-retention-goldens.mjs',
  'Website public save/restore + boneyard-world.ts retained scenery tables', { cases })
