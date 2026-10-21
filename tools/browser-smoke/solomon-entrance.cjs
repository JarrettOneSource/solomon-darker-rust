// Development-only path planning through the unchanged Source navigator. The
// browser receives ordinary keyboard events; no simulation state or input packet
// is installed/sent by this observer.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {pathToFileURL} = require('node:url');

async function walkSolomonEntrance(page, {document, snapshot, playerId, output}) {
  const game = path.resolve(process.env.SOLOMON_WEBSITE_DIR
    || path.join(__dirname, '../../../website-rust-current'), 'frontend/src/game');
  const load = file => import(pathToFileURL(path.join(game, file)).href);
  const [saves, entities, simulation, navigation] = await Promise.all([
    load('save/game-save-document.ts'), load('core-server/player-entity-store.ts'),
    load('core-server/game-simulation.ts'), load('host/boneyard-entrance-navigation.ts'),
  ]);
  const restored = saves.restoreGameSaveDocument(document);
  let planning = restored.state;
  assert.equal(planning.world.kind, 'boneyard');
  assert.equal(planning.world.runId, snapshot().boneyard.run_id,
    'the read-only Source route must use the actual accepted run geometry');
  const navigator = new navigation.BoneyardEntranceNavigator();
  const held = new Set(), frames = [];
  const deadline = Date.now() + 120000;
  try {
    while (snapshot().boneyard.encounter?.run_event_id === 0 && Date.now() < deadline) {
      const accepted = snapshot();
      const wizard = accepted.participants[playerId()].wizard;
      const encounter = accepted.boneyard.encounter;
      assert.equal(accepted.boneyard.run_id, planning.world.runId);
      assert.equal(accepted.participants[playerId()].player.combat.life_state, 'Alive');
      // This disposable Source object only answers a route query at the real
      // accepted position. Its state is never stepped or installed in the host.
      const character = simulation.gameSimulationPlayerRecords(planning)[restored.playerId];
      planning = {...planning, tick: accepted.ticks,
        playerEntities: entities.replacePlayerCharacter(planning.playerEntities, restored.playerId,
          {...character, position: wizard.position}),
        world: {...planning.world, encounter: {...planning.world.encounter,
          phase: encounter.phase.toLowerCase(), runEventId: encounter.run_event_id,
          position: encounter.position}},
      };
      const input = navigator.input(planning, restored.playerId);
      const movement = input?.movement || {x: 0, y: 0};
      const desired = new Set();
      // Keyboard's eight directions follow the actual navigator's current
      // bearing. Replanning continues from accepted collision-resolved positions.
      const diagonal = Math.tan(Math.PI / 8);
      if (Math.abs(movement.x) > Math.abs(movement.y) * diagonal)
        desired.add(movement.x < 0 ? 'KeyA' : 'KeyD');
      if (Math.abs(movement.y) > Math.abs(movement.x) * diagonal)
        desired.add(movement.y < 0 ? 'KeyW' : 'KeyS');
      for (const key of held) if (!desired.has(key)) {
        await page.keyboard.up(key);
        held.delete(key);
      }
      for (const key of desired) if (!held.has(key)) {
        await page.keyboard.down(key);
        held.add(key);
      }
      frames.push({tick: accepted.ticks, position: wizard.position,
        encounter: {phase: encounter.phase, runEventId: encounter.run_event_id},
        movement, keys: [...desired]});
      await page.waitForTimeout(100);
    }
    assert.ok(snapshot().boneyard.encounter === null
      || snapshot().boneyard.encounter.run_event_id > 0,
    'ordinary movement and the real Solomon sequence enable combat');
    return frames;
  } finally {
    for (const key of held) await page.keyboard.up(key);
    fs.writeFileSync(path.join(output, 'solomon-entrance.json'), JSON.stringify(frames, null, 2));
  }
}

module.exports = {walkSolomonEntrance};
