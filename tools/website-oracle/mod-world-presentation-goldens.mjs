import { loadKernel, writeFixture } from './oracle.mjs'

const enemies = await loadKernel('mod-ui/mod-enemies.ts')
const effects = await loadKernel('mod-ui/mod-spell-effects.ts')
const scenes = await loadKernel('mod-ui/mod-scene-model.ts')
const assets = [
  { byteLength: 1, contentType: 'image/png', kind: 'image', modId: 'example.art', path: 'world.png', sha256: 'a'.repeat(64) },
  { byteLength: 1, contentType: 'audio/ogg', kind: 'audio', modId: 'example.art', path: 'hit.ogg', sha256: 'b'.repeat(64) },
]
const actor = {
  id: 7, mod_id: 'example.art', image_path: 'world.png', frame_x: 3, frame_y: 8,
  frame_width: 24, frame_height: 32, image_width: 96, image_height: 128,
  x: -3.25, y: 17.75, current_health: 17, maximum_health: 30, life_state: 'alive',
  name: 'Gilded friend', scale: .75, light_color: 0x7faace, light_radius: 26,
  sound_path: 'hit.ogg', sound_volume: .35, sound_event_tick: 109,
}
const effect = {
  ...actor, kind: 'channel', radius: 9.25, started_tick: 104, target_x: -27.5, target_y: 104.5,
}
const runtime = {
  enemies: [actor, { ...actor, id: 8, life_state: 'dying', sound_event_tick: null }],
  spell_effects: ['area', 'channel', 'projectile'].map((kind, index) => ({ ...effect, kind, id: 30 + index })),
  scenes: [{
    owner_id: 'run-1', scene_content_id: 'example.art:crypt', epoch: 4, room_index: 1,
    rooms: [
      { content_id: 'example.art:room0', name: 'Crypt', description: 'An authored room.',
        geometry: { floor: '#1a2B3c', width: 160, height: 4096, walls: [{ x: 3, width: 25 }, null] },
        props: [{ kind: 'altar', x: 13, color: '#336699' }, false] },
      { content_id: 'example.art:room1', name: '', geometry: { floor: 'red', width: 159, height: 4097 } },
    ],
  }],
}
const invalid = {
  enemies: [null, [], 'enemy', { ...actor, id: 1.25 }, { ...actor, mod_id: 'EXAMPLE.ART' },
    { ...actor, scale: 0 }, { ...actor, maximum_health: 0 }, { ...actor, life_state: 'dead' },
    { ...actor, id: 9, sound_path: 'absent.ogg', sound_event_tick: undefined }],
  spell_effects: [null, [], { ...effect, id: 0 }, { ...effect, kind: 'unknown' },
    { ...effect, frame_width: -1 }, { ...effect, image_path: false },
    { ...effect, id: 90, target_x: 'not a number', started_tick: 1.5 }],
  scenes: [{ ...runtime.scenes[0], room_index: 3 }],
}
const cases = [
  { name: 'accepted-families', runtime, ownerId: 'run-1' },
  { name: 'filtered-invalid-rows', runtime: invalid, ownerId: 'run-1' },
  { name: 'foreign-owner', runtime, ownerId: 'player-2' },
  { name: 'no-runtime', runtime: null, ownerId: 'run-1' },
].map(input => ({
  ...input,
  enemies: enemies.projectModEnemies(input.runtime, assets),
  effects: effects.projectModSpellEffects(input.runtime, assets),
  scene: scenes.projectModScene(input.runtime, input.ownerId),
}))
writeFixture('mod-world-presentation-goldens.json', 'tools/website-oracle/mod-world-presentation-goldens.mjs',
  'Pinned Website mod-ui/mod-enemies.ts, mod-spell-effects.ts and mod-scene-model.ts', { assets, cases })
