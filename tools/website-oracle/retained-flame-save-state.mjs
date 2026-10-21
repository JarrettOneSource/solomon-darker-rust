// Controlled initial-state input for the current complete-save continuation oracle.
// Constructors are imported unchanged; this is never used by a browser/authority.
import { loadKernel } from './oracle.mjs'
const factory = await loadKernel('core-kernels/native-weld-flame-birth.ts')
const fade = await loadKernel('core-kernels/native-weld-flame-lash.ts')
const manager = await loadKernel('core-kernels/native-world-manager-order.ts')
const run = await loadKernel('core-kernels/game-run.ts')
const entities = await loadKernel('core-server/player-entity-store.ts')

export function withRetainedFlameActors(game, ownerId = game.playerEntities.identities[0].playerId) {
  const index = game.playerEntities.identities.findIndex(identity => identity.playerId === ownerId)
  if (index < 0) throw new Error('Flame save input needs its actual owner')
  const lighting = game.playerEntities.lightings[index]
  const character = entities.playerCharacterAt(game.playerEntities, ownerId)
  const start = { x: Math.fround(character.position.x), y: Math.fround(character.position.y - 40) }
  const control = { x: Math.fround(start.x + 200), y: Math.fround(start.y - 50) }
  const end = { x: Math.fround(start.x + 400), y: Math.fround(start.y - 40) }
  const worldKey = game.world.kind === 'boneyard' ? `boneyard:${game.world.runId}` : `hub:${game.world.participants[ownerId].region}`
  const id = game.primarySpells.nextId
  const order = manager.createNativeWorldManagerOrder(game.worldManagerOrder)
  const channel = { id, buildId: 1003, direction: { x: 1, y: 0 }, endpoint: end, midpoint: control,
    enhancedEffects: game.enhancedEffects, origin: start, ownerId, targetId: null,
    tick: run.gameRunWorldTick(game.tick, game.run), underpowered: false,
    vector: [5, 5, 2, .5, 0, 0, 0, 0], worldKey }
  const endpoint = fade.createNativeWeldFlameLashFade({ ...channel, id: id + 1, origin: end,
    alpha: 1, variant: 'endpoint', rng: game.combatRng, registerWorldPainter: order.register })
  const born = factory.createNativeFlameLashFactory({ channel, sourceFadeId: id + 2,
    geometry: { start, control, end, offset: { x: 0, y: 0 } }, role: 'main', width: 1,
    cameraDisplacement: { x: .5, y: -1 }, creatorRegistration: lighting.lightRegistration,
    miscAppendOrdinal: 0, registerWorldPainter: order.register, rng: endpoint.rng })
  return { ...game, combatRng: born.rng, worldManagerOrder: order.state(), primarySpells: {
    ...game.primarySpells, nextId: id + 3,
    transients: [...game.primarySpells.transients, endpoint.actor, born.source, born.channel],
  } }
}
