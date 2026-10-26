const assert = require('node:assert/strict');
const test = require('node:test');
const bank = require('../../web/assets/boneyards.json');
const {prepareBoneyardNavigation} = require('./solomon-entrance.cjs');

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
