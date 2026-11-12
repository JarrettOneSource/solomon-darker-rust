// Actual authored sprite cache and wearable preparation sharing one verified image.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const http = require('node:http');
const path = require('node:path');
const crypto = require('node:crypto');
const {chromium} = require(path.resolve(process.env.PW));
const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
const client = path.resolve(process.env.CLIENT_DIR);
const backend = process.env.GPU_BACKEND || 'native';
const files = new Map();
const binaries = [];
for (const name of fs.readdirSync(client).filter(name => /\.(js|wasm)$/.test(name))) {
  const bytes = fs.readFileSync(path.join(client, name));
  files.set(`/probe/${name}`, [name.endsWith('.wasm') ? 'application/wasm' : 'text/javascript', bytes]);
  binaries.push({name, bytes:bytes.length, sha256:crypto.createHash('sha256').update(bytes).digest('hex')});
}
(async () => {
  const server = http.createServer((request, response) => {
    if (request.url === '/') {
      response.writeHead(200, {'content-type':'text/html'});
      response.end('<!doctype html><title>Authored frame sampling regression</title><link rel="icon" href="data:,"><main id="solomon-unforge-isolated-fixture"></main>');
      return;
    }
    const file = files.get(request.url);
    response.writeHead(file ? 200 : 404, {'content-type':file?.[0] || 'text/plain'});
    response.end(file?.[1] || 'Not found');
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  let owned;
  try {
    owned = await launchOwnedBrowser(chromium, {headless:true,
      executablePath:process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      args:[...(backend === 'native' ? [] : [`--use-angle=${backend}`]), '--ignore-gpu-blocklist', ...(backend === 'swiftshader' ? ['--enable-unsafe-swiftshader'] : [])]});
    const page = await owned.browser.newPage();
    const errors = [], consoleErrors = [], failedRequests = [], failedResponses = [];
    page.on('pageerror', error => errors.push(String(error)));
    page.on('console', message => {if (message.type() === 'error') consoleErrors.push(message.text());});
    page.on('requestfailed', request => failedRequests.push({url:request.url(), error:request.failure()}));
    page.on('response', response => {if (response.status() >= 400) failedResponses.push({url:response.url(), status:response.status()});});
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    const {actual, gpu} = await page.evaluate(async () => {
      const module = await import('/probe/solomon_client.js');
      await module.default();
      const canvas = document.createElement('canvas');
      canvas.width = canvas.height = 256;
      const gl = canvas.getContext('webgl2', {premultipliedAlpha:false, preserveDrawingBuffer:true});
      if (!gl) throw Error('WebGL2 is unavailable');
      const debug = gl.getExtension('WEBGL_debug_renderer_info');
      const gpu = {renderer:debug ? gl.getParameter(debug.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER),
        version:gl.getParameter(gl.VERSION), attributes:gl.getContextAttributes()};
      const compare = (actual, expected) => {
        let maximumDifference = 0, changedPixels = 0;
        for (let i = 0; i < actual.length; i += 4) {
          let changed = false;
          for (let channel = 0; channel < 4; channel++) {
            const delta = Math.abs(actual[i + channel] - expected[i + channel]);
            maximumDifference = Math.max(maximumDifference, delta);
            changed ||= delta > 1;
          }
          changedPixels += Number(changed);
        }
        return {maximumDifference, changedPixels};
      };
      const rows = [];
      try {
        for (const name of ['integer', 'half-span', 'half-span-between-texels', 'fractional-origin']) {
          module.mod_atlas_fixture_init(gl, name);
          const expected = module.mod_atlas_fixture_render(true, false);
          const before = module.mod_atlas_fixture_render(false, false);
          module.mod_atlas_fixture_prepare_wearable();
          const after = module.mod_atlas_fixture_render(false, false);
          const expectedWearable = module.mod_atlas_fixture_render(true, true);
          const actualWearable = module.mod_atlas_fixture_render(false, true);
          const restored = module.mod_atlas_fixture_render(false, false);
          rows.push({name, crop:compare(before, expected), sharedImage:compare(after, before),
            wearable:compare(actualWearable, expectedWearable), restoration:compare(restored, after)});
          module.mod_atlas_fixture_dispose();
        }
      } finally {
        module.mod_atlas_fixture_dispose();
        gl.getExtension('WEBGL_lose_context')?.loseContext();
      }
      return {actual:rows, gpu};
    });
    const receipt = {sheet:{width:4080,height:170,firstFrame:[0,0,8,8],scale:7.5,rotation:0.125},
      binaries, backend, gpu, actual, errors, consoleErrors, failedRequests, failedResponses};
    if (process.env.OUT) {
      fs.mkdirSync(process.env.OUT, {recursive:true});
      fs.writeFileSync(path.join(process.env.OUT, 'receipt.json'), JSON.stringify(receipt, null, 2));
    }
    for (const failures of [errors, consoleErrors, failedRequests, failedResponses]) assert.deepEqual(failures, []);
    for (const row of actual) {
      assert.ok(row.crop.maximumDifference <= 1, `Authored frame matches independent crop: ${JSON.stringify(row)}`);
      assert.equal(row.sharedImage.maximumDifference, 0, `Wearable preparation preserves icon filtering: ${JSON.stringify(row)}`);
      assert.ok(row.wearable.maximumDifference <= 1, `Wearable retains point sampling: ${JSON.stringify(row)}`);
      assert.equal(row.restoration.maximumDifference, 0, `Wearable draw restores icon sampling: ${JSON.stringify(row)}`);
    }
    console.log(`Authored sprite bounds and shared-image sampler lifetime passed: ${actual.length} cases`);
  } finally {
    if (owned) await owned.close();
    await new Promise(resolve => server.close(resolve));
  }
})().catch(error => {console.error(error); process.exitCode = 1;});
