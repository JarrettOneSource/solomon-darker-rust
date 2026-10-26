// Read-only Rust navigation chooses ordinary keys. Accepted snapshots remain
// the movement authority; the helper has no connection to the running game.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {spawn} = require('node:child_process');
const {createInterface} = require('node:readline');

async function prepareBoneyardNavigation(world) {
  const executable = process.env.SOLOMON_NAVIGATION_BINARY || path.join(__dirname,
    '../../target/debug/examples', process.platform === 'win32' ? 'browser_navigation.exe' : 'browser_navigation');
  assert.ok(fs.existsSync(executable),
    'build the navigation helper: cargo build -p solomon-assets --example browser_navigation');
  const child = spawn(executable, [world.source_sha256, world.geometry_sha256,
    path.join(__dirname, '../../web/assets')], {stdio: ['pipe', 'pipe', 'pipe']});
  let stderr = '', processError, closing = false, closePromise;
  child.stderr.on('data', data => {stderr = (stderr + data).slice(-8192);});
  child.on('error', error => {processError = error;});
  child.stdin.on('error', error => {processError = error;});
  const exited = new Promise(resolve => child.once('close', (code, signal) => resolve({code, signal})));
  const lines = createInterface({input: child.stdout})[Symbol.asyncIterator]();
  async function bounded(promise, label, timeout = 30000) {
    let timer;
    try {
      return await Promise.race([promise, new Promise((_, reject) => {
        timer = setTimeout(() => {
          child.kill('SIGKILL');
          reject(new Error(`navigation helper timed out: ${label}`));
        }, timeout);
      })]);
    } finally { clearTimeout(timer); }
  }
  async function read() {
    const line = await bounded(lines.next(), 'response');
    if (line.done) {
      const exit = await exited;
      throw new Error(`navigation helper exited (${exit.code ?? exit.signal}): ${processError || stderr}`);
    }
    return JSON.parse(line.value);
  }
  function close() {
    if (!closePromise) {
      closing = true;
      child.stdin.end();
      closePromise = bounded(exited, 'shutdown', 5000).then(exit => {
        assert.equal(exit.signal, null, 'navigation helper exits after stdin closes');
        return exit.code;
      });
    }
    return closePromise;
  }
  let pending = Promise.resolve();
  function query(value) {
    if (closing) return Promise.reject(new Error('navigation helper is closed'));
    pending = pending.then(async () => {
      await new Promise((resolve, reject) => child.stdin.write(JSON.stringify(value) + '\n',
        error => error ? reject(error) : resolve()));
      return read();
    });
    return pending;
  }
  try {
    const metadata = await read();
    return {...metadata, close,
      findRoute: (start, end, bounds, ignoreEndRadius = null) => query({kind: 'route', start, end,
        bounds, ignore_end_radius: ignoreEndRadius}),
      direction: (start, end, bounds) => query({kind: 'direction', start, end, bounds}),
      containsContact: (solomon, player) => query({kind: 'contact', solomon, player}),
      screenPoint: (position, focus, bounds, view) => query({kind: 'screen_point', position, focus, bounds, view}),
    };
  } catch (error) {
    await close();
    throw error;
  }
}

async function walkSolomonEntrance(page, {snapshot, playerId, output, plan}) {
  const initial = snapshot().boneyard;
  const ownsPlan = !plan;
  plan ||= await prepareBoneyardNavigation(initial);
  const held = new Set(), frames = [];
  try {
    let route = [], waypoint = 1, targetKey, plannedAtTick = -Infinity;
    const deadline = Date.now() + 120000;
    const distance = (a, b) => Math.hypot(a.x - b.x, a.y - b.y);
    const bounds = initial.arena_transition?.full_bounds;
    const fullBounds = bounds || plan.scene.bounds;
    // Choose the entrance from the authored spawn; the wizard may have wandered.
    // Cross far enough to clear the swinging leaves before approaching Solomon.
    const pairs = new Map();
    for (const leaf of initial.gate_leaves) {
      const pair = pairs.get(leaf.fence_eid) || [];
      pair.push(leaf.hinge); pairs.set(leaf.fence_eid, pair);
    }
    const gate = [...pairs].filter(([, pair]) => pair.length === 2)
      .map(([id, pair]) => ({id, x: (pair[0].x + pair[1].x) / 2, y: (pair[0].y + pair[1].y) / 2}))
      .sort((a, b) => distance(a, plan.scene.spawn) - distance(b, plan.scene.spawn))[0];
    // Stock Tutorial has no arena transition and spawns south of its gate.
    // Generated arenas carry their entry side explicitly.
    const direction = initial.arena_transition
      ? initial.arena_transition.entry_side === 'South' ? -1 : 1
      : gate && plan.scene.spawn.y > gate.y ? -1 : 1;
    const approachMargin = plan.bodyRadius + plan.gateSway + 15;
    const crossedMargin = plan.bodyRadius + 35;
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
        } else if (!await plan.containsContact(encounter.position, at)) {
          const end = crossed ? encounter.position : {x: gate.x, y: gate.y - direction * approachMargin};
          const key = crossed ? 'solomon' : `gate:${gate.id}`;
          if (targetKey !== key || accepted.ticks - plannedAtTick >= 100 || route.length < 2) {
            // The route uses fixed scenery; actual movement pushes swinging gates.
            route = await plan.findRoute(at, end, fullBounds, crossed ? 30 : null);
            assert.ok(route && route.length > 1, 'Rust navigation reaches the entrance target');
            targetKey = key; plannedAtTick = accepted.ticks; waypoint = 1;
          }
          while (waypoint < route.length - 1 && distance(at, route[waypoint]) <= 20) waypoint++;
          target = route[waypoint]; phase = key;
        }
        if (target) {
          movement = await plan.direction(at, target, fullBounds);
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
    try {
      for (const key of held) await page.keyboard.up(key);
      fs.writeFileSync(path.join(output, 'solomon-entrance.json'), JSON.stringify(frames, null, 2));
    } finally {
      if (ownsPlan) assert.equal(await plan.close(), 0, 'navigation helper exits cleanly');
    }
  }
}

module.exports = {prepareBoneyardNavigation, walkSolomonEntrance};
