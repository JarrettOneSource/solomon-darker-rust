import { loadKernel, writeFixture } from './oracle.mjs'

const { projectModMinimap } = await loadKernel('mod-ui/mod-minimap.ts')
const { modContentProjection } = await loadKernel('protocol/codecs/mod-content.ts')
const content = {
  boasts: [],
  content: [{
    art: [], contentId: '5000000000000000012', contentKind: 'ui', description: '',
    key: 'field_minimap', modId: 'example.mod', name: 'Field Minimap', presentation: 'prefab.minimap',
  }, {
    art: [{ path: 'art/powerup.png', slot: 'world' }],
    contentId: '5000000000000000004', contentKind: 'powerup', description: '',
    key: 'field_powerup', modId: 'example.mod', name: 'Field Powerup', presentation: null,
  }],
  manifestSha256: 'a'.repeat(64),
  powerups: [{ contentId: '5000000000000000004', id: 1e20, spawnedTick: 1, x: 70, y: 80 }],
  revision: 1,
  statuses: [],
}
const snapshot = {
  players: {
    self: { position: { x: 10, y: 20 } },
    ally: { position: { x: -3000, y: 4000 } },
  },
  world: { kind: 'boneyard', enemies: [{ id: 7, position: { x: 50, y: 60 } }] },
}
const enemies = [null, [], { id: 'not a number', x: 1, y: 2 }, { id: 9, x: 90, y: 100 }]
const surface = (fields, extra = {}) => ({
  accessible_name: 'Nearby foes', actions: ['mark', 4, null, 'inspect'],
  bindings: { wave: 9, label: 'example', nested: { health: true } },
  content_id: '5000000000000000022', mount: 'hud.bottom_left',
  view: { operation: 'prefab.minimap', fields }, ...extra,
})
const cases = []
function identity(marker) {
  if (marker.kind === 'self' || marker.kind === 'party') return { kind: 'player', value: marker.id }
  if (marker.id.startsWith('mod-enemy-')) return { kind: 'mod-enemy', value: Number(marker.id.slice(10)) }
  if (marker.id.startsWith('enemy-')) return { kind: 'enemy', value: Number(marker.id.slice(6)) }
  return { kind: 'powerup', value: Number(marker.id.slice(8)) }
}
function record(name, runtime = null, sourceSnapshot = snapshot, sourceContent = content, viewer = 'self') {
  // Fixtures pass through the same public codec as an admitted browser message.
  modContentProjection(sourceContent)
  const model = projectModMinimap(sourceSnapshot, sourceContent, viewer, runtime)
  cases.push({
    name, content: sourceContent, runtime, viewer,
    snapshot: {
      players: Object.entries(sourceSnapshot.players).map(([id, player]) => ({ id, position: [player.position.x, player.position.y] })),
      enemies: (sourceSnapshot.world.enemies ?? []).map(enemy => ({ id: enemy.id, position: [enemy.position.x, enemy.position.y] })),
      inBoneyard: sourceSnapshot.world.kind === 'boneyard',
    },
    expected: model && {
      accessibleName: model.accessibleName, actions: model.actions, bindings: model.bindings,
      center: [model.center.x, model.center.y], contentId: model.contentId,
      markers: model.markers.map(marker => ({ id: identity(marker), kind: marker.kind, position: [marker.x, marker.y] })),
      mount: model.mount, range: model.range, size: [model.size.width, model.size.height],
    },
  })
}
record('no runtime uses admitted definition defaults')
record('native and custom enemies', { enemies })
record('missing local viewer', null, snapshot, content, 'absent')
record('no definition', null, snapshot, { ...content, content: content.content.filter(entry => entry.contentKind !== 'ui') })
record('other UI surface suppresses minimap', { ui_surfaces: [{ view: { operation: 'button' } }] })
record('invalid surface rows are ignored', { ui_surfaces: [null, [], 'bad'], enemies })
record('first matching definition', null, snapshot, { ...content, content: [
  { ...content.content[0], contentId: '5000000000000000011', name: 'First map' }, ...content.content,
] })
for (const kind of ['hub', 'boneyard']) {
  for (const layers of [undefined, [], ['party'], ['visible_hostiles'], ['powerups'], [{ source: 'party' }, { source: 'powerups' }], [null, 5, {}]]) {
    record(`${kind} layers ${JSON.stringify(layers)}`, {
      ui_surfaces: [surface({ layers })], enemies,
    }, { ...snapshot, world: { ...snapshot.world, kind } })
  }
}
for (const size of [undefined, 240, { width: 320, height: 180 }, { height: 140 }, { width: 0, height: -10 }, 0, -3, 'bad']) {
  for (const range of [undefined, 330, 0, -10, 'bad']) {
    record(`size ${JSON.stringify(size)} range ${JSON.stringify(range)}`, { ui_surfaces: [surface({ size, range })] })
  }
}
for (const mount of ['hud.top_left', 'hud.top_right', 'hud.bottom_left', 'hud.bottom_right', 'hud.overlay', '', 4]) {
  record(`mount ${mount}`, { ui_surfaces: [surface({}, { mount, accessible_name: '', content_id: '' })] })
}
record('first matching surface', { ui_surfaces: [
  { view: { operation: 'other' } }, surface({ range: 123 }), surface({ range: 456 }),
] })
record('missing fields and malformed actions/bindings', { ui_surfaces: [
  { view: { operation: 'prefab.minimap' }, actions: ['ok', false], bindings: [], mount: false },
] })
writeFixture('mod-minimap-goldens.json', 'mod-minimap-goldens.mjs',
  'mod-ui/mod-minimap.ts (pinned ed0a2d59)', { cases })
