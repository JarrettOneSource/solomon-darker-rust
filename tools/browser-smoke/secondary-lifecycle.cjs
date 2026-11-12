// Exercise the real secondary renderer and its world-reset boundary on a Mac GPU.
// Frozen actor samples supply visible targets; WebGL reports their actual lifetime.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const http = require('node:http');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { inflateSync } = require('node:zlib');
const { chromium } = require(process.env.PW || 'playwright-core');
const { launchOwnedBrowser } = require('./browser-lifecycle.cjs');

const output = process.env.OUT;
if (!process.env.CLIENT_DIR || !output) throw new Error('CLIENT_DIR and OUT are required');
const client = path.resolve(process.env.CLIENT_DIR);
const fixturePath = path.resolve(__dirname, '../../crates/sim/tests/fixtures/website/secondary-tick-goldens.json.zlib');
const fixtureBytes = inflateSync(fs.readFileSync(fixturePath));
const fixture = JSON.parse(fixtureBytes);
const receipt = { passed: false, fixtureSha256: createHash('sha256').update(fixtureBytes).digest('hex'),
  qualification: 'Frozen actor renderer fixture; actual GPU resource retirement and same-world reconstruction, not a historical performance reproduction',
  results: [], errors: [] };

function input(name, kind, hit) {
  const sample = fixture.cases.find(row => row.name === name)?.samples.find(row => row.tick === 500);
  assert.ok(sample, `${name} tick 500 exists`);
  const actors = structuredClone(sample.state.actors);
  const owner = actors.find(actor => actor.kind === kind);
  assert.ok(owner, `${kind} is live in the recording`);
  return { actors, width: 640, height: 480, worldKey: owner.worldKey,
    origin: { x: owner.position.x - 320, y: owner.position.y - 280 }, scale: 1,
    frame: 500, pointGain: 0.75, referenceY: owner.position.y, saturation: 0.65,
    enhancedEffects: true, nativeWorldPixelCenters: true,
    ...(hit ? { hit: { id: owner.id, alpha: 0.7, tint: 0xff8080 } } : {}) };
}

function observeResources() {
  const canvas = document.querySelector('#secondary-fixture');
  const gl = canvas.getContext('webgl2');
  const resources = [];
  for (const kind of ['Texture', 'Framebuffer']) {
    const create = gl[`create${kind}`].bind(gl);
    const destroy = gl[`delete${kind}`].bind(gl);
    gl[`create${kind}`] = (...args) => {
      const handle = create(...args);
      resources.push({ kind, handle, deletes: 0 });
      return handle;
    };
    gl[`delete${kind}`] = handle => {
      const row = resources.find(row => row.kind === kind && row.handle === handle);
      if (row) row.deletes++;
      return destroy(handle);
    };
  }
  const allocate = gl.texImage2D.bind(gl);
  gl.texImage2D = (...args) => {
    const row = resources.find(row => row.kind === 'Texture'
      && row.handle === gl.getParameter(gl.TEXTURE_BINDING_2D));
    if (row) row.size = [args[3], args[4]];
    return allocate(...args);
  };
  window.secondaryResourceState = () => resources.map(({ kind, handle, deletes, size }, index) =>
    ({ index, kind, deletes, size, live: gl[`is${kind}`](handle) }));
  window.secondaryPixels = async () => {
    const pixels = new Uint8Array(canvas.width * canvas.height * 4);
    gl.readPixels(0, 0, canvas.width, canvas.height, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
    const digest = await crypto.subtle.digest('SHA-256', pixels);
    let paintedPixels = 0;
    for (let index = 0; index < pixels.length; index += 4) {
      if (pixels[index] !== 31 || pixels[index + 1] !== 31 || pixels[index + 2] !== 31) paintedPixels++;
    }
    return { sha256: Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, '0')).join(''), paintedPixels };
  };
  const extension = gl.getExtension('WEBGL_debug_renderer_info');
  return { renderer: extension ? gl.getParameter(extension.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER),
    version: gl.getParameter(gl.VERSION), attributes: gl.getContextAttributes() };
}

async function main() {
  fs.mkdirSync(output, { recursive: true });
  const files = new Map();
  receipt.wasm = [];
  for (const name of fs.readdirSync(client).filter(name => /\.(js|wasm)$/.test(name))) {
    const bytes = fs.readFileSync(path.join(client, name));
    files.set(`/client/${name}`, [name.endsWith('.wasm') ? 'application/wasm' : 'text/javascript', bytes]);
    receipt.wasm.push({ name, sha256: createHash('sha256').update(bytes).digest('hex') });
  }
  for (const name of ['BadGuys.bundle', 'BadGuys.png', 'DeadHawg.bundle', 'DeadHawg.png',
    'Demon.bundle', 'Demon.png', 'Golem.bundle', 'Golem.png',
    'player-harden-ice.png', 'player-mindblast-ring.png', 'etherplane.png']) {
    files.set(`/assets/stock/${name}`, [name.endsWith('.png') ? 'image/png' : 'application/octet-stream',
      fs.readFileSync(path.resolve(__dirname, '../../web/assets/stock', name))]);
  }
  const server = http.createServer((request, response) => {
    if (request.url === '/') {
      response.writeHead(200, { 'content-type': 'text/html' });
      response.end('<!doctype html><title>Secondary world retirement</title><main id="solomon-unforge-isolated-fixture"></main>');
      return;
    }
    const file = files.get(request.url);
    response.writeHead(file ? 200 : 404, { 'content-type': file?.[0] || 'text/plain' });
    response.end(file?.[1] || 'Not found');
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  let owned;
  try {
    owned = await launchOwnedBrowser(chromium, { headless: true,
      executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      args: ['--use-angle=metal', '--ignore-gpu-blocklist'] });
    const page = await owned.browser.newPage({ viewport: { width: 1280, height: 800 } });
    page.on('pageerror', error => receipt.errors.push(error.message));
    page.on('console', message => {
      if (message.type() === 'error' && !message.location().url.endsWith('/favicon.ico')) receipt.errors.push(message.text());
    });
    page.on('response', response => { if (response.status() >= 400 && !response.url().endsWith('/favicon.ico')) receipt.errors.push(`HTTP ${response.status()} ${response.url()}`); });
    await page.goto(`http://127.0.0.1:${server.address().port}`, { waitUntil: 'networkidle' });
    await page.evaluate(async () => {
      window.wasmBindings = await import('/client/solomon_client.js');
      await window.wasmBindings.default();
      await window.wasmBindings.secondary_fixture_init();
    });
    receipt.gpu = await page.evaluate(observeResources);
    const cases = [
      { name: 'leviathan', value: input('11-maximum', 'leviathan', false), targets: 1 },
      { name: 'leviathan-hit', value: input('11-maximum', 'leviathan', true), targets: 2 },
      { name: 'storm', value: input('27-maximum', 'storm-cloud', false), targets: 1 },
    ];
    for (const { name, value, targets } of cases) {
      const result = { name, targets, colorBytes: targets * 256 * 256 * 4 };
      receipt.results.push(result);
      const render = input => page.evaluate(input => window.wasmBindings.render_secondary_fixture(JSON.stringify(input)), input);
      const state = () => page.evaluate(() => window.secondaryResourceState());
      const reset = () => page.evaluate(() => window.wasmBindings.reset_secondary_fixture());
      await render(value);
      result.allocated = await state();
      const live = result.allocated.filter(row => row.live);
      assert.equal(live.filter(row => row.kind === 'Texture').length, targets, `${name} owns its color textures`);
      assert.equal(live.filter(row => row.kind === 'Framebuffer').length, targets, `${name} owns its framebuffers`);
      for (const row of live.filter(row => row.kind === 'Texture')) assert.deepEqual(row.size, [256, 256]);
      result.before = await page.evaluate(() => window.secondaryPixels());
      assert.ok(result.before.paintedPixels > 0, `${name} produces visible output before reset`);
      await page.locator('#secondary-fixture').screenshot({ path: path.join(output, `${name}-before.png`) });
      await reset();
      result.retired = await state();
      assert.equal(result.retired.filter(row => row.live).length, 0, `${name} world reset releases GPU resources immediately`);
      assert.ok(result.retired.every(row => row.deletes === 1), `${name} each retired resource is deleted once`);
      await reset();
      assert.deepEqual(await state(), result.retired, `${name} repeated reset has no stale handles`);
      await render(value);
      result.recreated = await state();
      assert.equal(result.recreated.length - result.retired.length, targets * 2, `${name} fresh world recreates resources with reused actor IDs`);
      result.after = await page.evaluate(() => window.secondaryPixels());
      assert.deepEqual(result.after, result.before, `${name} recreated world retains exact output pixels`);
      await page.locator('#secondary-fixture').screenshot({ path: path.join(output, `${name}-after.png`) });
      await render({ ...value, actors: [], hit: undefined });
      result.expired = await state();
      assert.ok(result.expired.every(row => !row.live && row.deletes === 1), `${name} ordinary actor expiry also releases targets once`);
      await reset();
      assert.deepEqual(await state(), result.expired);
      const error = await page.evaluate(() => document.querySelector('#secondary-fixture').getContext('webgl2').getError());
      assert.equal(error, 0, `${name} has no WebGL error`);
    }
    assert.deepEqual(receipt.errors, []);
    receipt.passed = true;
  } catch (error) {
    receipt.failure = String(error);
    throw error;
  } finally {
    try { if (owned) receipt.cleanup = await owned.close(); }
    catch (error) {
      receipt.passed = false;
      receipt.cleanup = error.cleanup || { error: String(error) };
      throw error;
    }
    finally {
      await new Promise(resolve => server.close(resolve));
      fs.writeFileSync(path.join(output, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`);
    }
  }
  console.log(JSON.stringify(receipt));
}
main().catch(error => { console.error(error); process.exitCode = 1; });
