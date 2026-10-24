// Development-only Source collision/navigation queries. Only ordinary keyboard
// events reach the game; accepted snapshots remain the sole movement authority.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {pathToFileURL} = require('node:url');

async function prepareBoneyardNavigation(world) {
  const game = path.resolve(process.env.SOLOMON_WEBSITE_DIR
    || path.join(__dirname, '../../../website-rust-current'), 'frontend/src/game');
  const load = file => import(pathToFileURL(path.join(game, file)).href);
  const [generated, tutorial, collision, navigation, encounter, character, gates, camera] = await Promise.all([
    load('host/native-generated-boneyards.ts'), load('host/stock-tutorial-boneyard.ts'),
    load('core-server/boneyard-collision.ts'), load('core-server/boneyard-enemy-navigation.ts'),
    load('core-kernels/boneyard-encounter.ts'), load('core-kernels/player-character.ts'),
    load('core-kernels/boneyard-gate.ts'), load('renderer/boneyard-render-contract.ts'),
  ]);
  const template = [...generated.NATIVE_GENERATED_BONEYARDS, tutorial.STOCK_TUTORIAL_BONEYARD]
    .find(row => row.sourceSha256 === world.source_sha256);
  assert.ok(template, 'the accepted Boneyard has its unchanged Source template');
  assert.equal(template.geometrySha256, world.geometry_sha256,
    'Source planning uses the actual accepted geometry');
  const geometry = collision.createBoneyardCollisionWorld(template.scene);
  const prepared = JSON.parse(fs.readFileSync(path.join(__dirname,
    '../../web/assets/navigation', `${world.source_sha256}.json`), 'utf8'));
  assert.equal(prepared.sourceSha256, world.source_sha256);
  assert.equal(prepared.geometrySha256, world.geometry_sha256);
  for (const row of prepared.meshes) navigation.installPreparedBoneyardNavigationMesh(geometry, {
    key: [row.bounds.x, row.bounds.y, row.bounds.w, row.bounds.h, row.clearance].join(':'), mesh: row.mesh,
  });
  return {collision, navigation, encounter, camera, geometry, scene: template.scene,
    sourceSha256: world.source_sha256, geometrySha256: world.geometry_sha256,
    bodyRadius: character.PLAYER_CHARACTER_RADIUS, gateSway: gates.BONEYARD_GATE_INITIAL_SWAY};
}

async function walkSolomonEntrance(page, {snapshot, playerId, output, plan}) {
  const initial = snapshot().boneyard;
  plan ||= await prepareBoneyardNavigation(initial);
  const held = new Set(), frames = [];
  let route = [], waypoint = 1, targetKey, plannedAtTick = -Infinity;
  const deadline = Date.now() + 120000;
  const distance = (a, b) => Math.hypot(a.x - b.x, a.y - b.y);
  const bounds = initial.arena_transition?.full_bounds;
  const fullBounds = bounds
    ? {x: bounds.x, y: bounds.y, w: bounds.width, h: bounds.height} : plan.scene.bounds;
  // Source boneyard-entrance-navigation.ts chooses the entry from immutable
  // spawn, not the current wizard, and crosses radius+35 beyond its midpoint.
  const pairs = new Map();
  for (const leaf of initial.gate_leaves) {
    const pair = pairs.get(leaf.fence_eid) || [];
    pair.push(leaf.hinge); pairs.set(leaf.fence_eid, pair);
  }
  const gate = [...pairs].filter(([, pair]) => pair.length === 2)
    .map(([id, pair]) => ({id, x: (pair[0].x + pair[1].x) / 2, y: (pair[0].y + pair[1].y) / 2}))
    .sort((a, b) => distance(a, plan.scene.spawn) - distance(b, plan.scene.spawn))[0];
  // Stock Tutorial has no arena transition and spawns south of its gate.
  // Keep an explicit Source transition side; otherwise use the immutable spawn.
  const direction = initial.arena_transition
    ? initial.arena_transition.entry_side === 'South' ? -1 : 1
    : gate && plan.scene.spawn.y > gate.y ? -1 : 1;
  const approachMargin = plan.bodyRadius + plan.gateSway + 15;
  const crossedMargin = plan.bodyRadius + 35;
  try {
    while (snapshot().boneyard.encounter?.run_event_id === 0 && Date.now() < deadline) {
      const accepted = snapshot(), world = accepted.boneyard;
      assert.equal(world.run_id, initial.run_id, 'planning cannot cross a real run replacement');
      assert.equal(world.source_sha256, plan.sourceSha256);
      assert.equal(world.geometry_sha256, plan.geometrySha256);
      const actor = accepted.participants[playerId()];
      assert.equal(actor.player.combat.life_state, 'Alive');
      const at = actor.wizard.position, encounter = world.encounter;
      let target = null, movement = {x: 0, y: 0}, phase = encounter.phase;
      if (phase === 'Digging') {
        const crossed = !gate || (at.y - gate.y) * direction > crossedMargin;
        if (!crossed && (gate.y - at.y) * direction <= approachMargin + 10
          && Math.abs(gate.x - at.x) <= 40) {
          target = {x: gate.x, y: gate.y + direction * crossedMargin};
          phase = 'gate-crossing';
        } else if (!plan.encounter.solomonContactContains(encounter.position, at)) {
          const end = crossed ? encounter.position : {x: gate.x, y: gate.y - direction * approachMargin};
          const key = crossed ? 'solomon' : `gate:${gate.id}`;
          if (targetKey !== key || accepted.ticks - plannedAtTick >= 100 || route.length < 2) {
            // Source plans against immutable scenery; real movement pushes the
            // current swinging leaves in the authoritative simulation.
            const collision = plan.geometry;
            route = plan.navigation.findBoneyardEnemyRoute({bodyRadius: plan.bodyRadius,
              bounds: fullBounds, clearance: plan.bodyRadius, start: at, end, endBodyRadius: 0,
              world: collision, ignoredSourceIds: crossed
                ? new Set(plan.collision.boneyardBodyCollisionSourceIds(end, collision, 30)) : undefined});
            assert.ok(route && route.length > 1, 'unchanged Source navigation reaches the real entrance target');
            targetKey = key; plannedAtTick = accepted.ticks; waypoint = 1;
          }
          while (waypoint < route.length - 1 && distance(at, route[waypoint]) <= 20) waypoint++;
          target = route[waypoint]; phase = key;
        }
        if (target) {
          const length = distance(at, target);
          if (length) movement = {x: (target.x - at.x) / length, y: (target.y - at.y) / length};
        }
      }
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
      frames.push({tick: accepted.ticks, position: at, phase, target,
        encounter: {phase: encounter.phase, runEventId: encounter.run_event_id},
        movement, keys: [...desired]});
      await page.waitForTimeout(50);
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

module.exports = {prepareBoneyardNavigation, walkSolomonEntrance};
