const assert = require('node:assert/strict');
const test = require('node:test');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const bank = require('../../web/assets/boneyards.json');
const {prepareBoneyardNavigation, walkSolomonEntrance} = require('./solomon-entrance.cjs');

const template = bank.tutorial;
const world = {source_sha256: template.sourceSha256, geometry_sha256: template.geometrySha256};

test('browser navigation uses the tracked Rust scene and closes its helper', async () => {
  const plan = await prepareBoneyardNavigation(world);
  try {
    assert.deepEqual(plan.scene.spawn, {x: template.scene.spawn.x, y: template.scene.spawn.y});
    const start = {x: 1025, y: 1900};
    const end = {x: start.x + 20, y: start.y};
    assert.deepEqual(await plan.findRoute(start, end, plan.scene.bounds), [start, end]);
    assert.deepEqual(await plan.direction(start, end, plan.scene.bounds), {x: 1, y: 0});
    assert.equal(await plan.containsContact(start, start), true);
    assert.equal(await plan.containsContact(start, {x: start.x + 1000, y: start.y}), false);
    const screen = await plan.screenPoint(start, start, plan.scene.bounds, {x: 1600, y: 900});
    assert.ok(Math.abs(screen.x - 800) < 1e-9);
    assert.ok(Math.abs(screen.y - 693.45) < 1e-9);
  } finally {
    assert.equal(await plan.close(), 0);
  }
  await assert.rejects(plan.findRoute(plan.scene.spawn, plan.scene.spawn, plan.scene.bounds), /closed/);
});

test('browser navigation rejects geometry from a different accepted scene', async () => {
  await assert.rejects(prepareBoneyardNavigation({...world, geometry_sha256: 'wrong'}), /geometry/);
});

test('Solomon entrance crosses the gate instead of settling at its strict boundary', async () => {
  const template = bank.templates.find(row => row.sourceSha256
    === '506200e6f89dd26150c7fcc76f5cddfdb321412657ac979ea5924b567b4a2933');
  assert.ok(template, 'the retained north-entry scene from the observed gate stall');
  const world = {source_sha256:template.sourceSha256, geometry_sha256:template.geometrySha256};
  const plan = await prepareBoneyardNavigation(world);
  const output = fs.mkdtempSync(path.join(os.tmpdir(), 'solomon-gate-crossing-'));
  const held = new Set();
  try {
    const gate = template.scene.fences.find(row => row.eid === 'fence-4');
    const center = {x:(gate.points[0].x+gate.points[1].x)/2,y:gate.points[0].y};
    const threshold = center.y+plan.bodyRadius+35;
    const position = {x:center.x,y:threshold-0.1};
    // The real Rust navigator checks six-unit candidates. It correctly stops when
    // every candidate moves farther from a goal only a fraction of a unit away.
    assert.deepEqual(await plan.direction(position,{x:center.x,y:threshold},plan.scene.bounds),
      {x:0,y:0});
    const encounter = {phase:'Digging',position:{x:center.x,y:threshold+200},run_event_id:0};
    const accepted = {ticks:0,participants:{local:{wizard:{position},
      player:{combat:{life_state:'Alive'}}}},boneyard:{...world,run_id:'gate-crossing',
      encounter,gate_leaves:gate.points.map(hinge=>({fence_eid:gate.eid,hinge})),
      arena_transition:{entry_side:'North',full_bounds:plan.scene.bounds}}};
    const page = {keyboard:{
      down:async key=>held.add(key),up:async key=>held.delete(key),
    },waitForTimeout:async()=>{
      const before = position.y;
      if(held.has('KeyS'))position.y+=6;
      assert.ok(position.y>before, 'the actual navigation choice must request gate-crossing progress');
      if(position.y>threshold)encounter.run_event_id=1;
      accepted.ticks+=5;
    }};
    const frames = await walkSolomonEntrance(page,{snapshot:()=>accepted,
      playerId:()=> 'local',output,plan});
    assert.ok(frames[0].target.y>threshold, 'the waypoint lies beyond the strict crossing condition');
    assert.ok(position.y>threshold, 'the selected ordinary key crosses that condition');
    assert.equal(held.size,0, 'the walker releases its keys after the entrance');
  } finally {
    assert.equal(await plan.close(),0);
    fs.rmSync(output,{recursive:true});
  }
});
