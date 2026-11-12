// Actual UiFrame -> material -> egui tessellation -> shared GPU painter.
// CLIENT_DIR is wasm-bindgen web output built with browser-fixtures; FIXTURE_DIR
// contains exact unpremultiplied UI.rgba/Create.rgba decoded from tracked PNGs.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const http = require('node:http');
const path = require('node:path');
const crypto = require('node:crypto');
const {chromium} = require(path.resolve(process.env.PW));
const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
const client = path.resolve(process.env.CLIENT_DIR);
const fixtures = path.resolve(process.env.FIXTURE_DIR);
const root = path.resolve(__dirname, '../..');
const moduleName = process.env.CLIENT_JS || 'solomon_client.js';
const backend = process.env.GPU_BACKEND || 'native';
const files = new Map();
const binaries = [];
for (const name of fs.readdirSync(client).filter(name => /\.(js|wasm)$/.test(name))) {
  const bytes = fs.readFileSync(path.join(client, name));
  files.set(`/probe/${name}`, [name.endsWith('.wasm') ? 'application/wasm' : 'text/javascript', bytes]);
  binaries.push({name, bytes:bytes.length, sha256:crypto.createHash('sha256').update(bytes).digest('hex')});
}
const sources = [];
for (const name of ['UI', 'Create']) {
  for (const [url, file] of [[`/assets/stock/${name}.bundle`, path.join(root, 'web/assets/stock', `${name}.bundle`)],
    [`/fixture/${name}.rgba`, path.join(fixtures, `${name}.rgba`)]]) {
    const bytes = fs.readFileSync(file);
    files.set(url, ['application/octet-stream', bytes]);
    sources.push({file, bytes:bytes.length, sha256:crypto.createHash('sha256').update(bytes).digest('hex')});
  }
}
{
  const file = path.join(fixtures, 'Fonts.rgba');
  const bytes = fs.readFileSync(file);
  files.set('/fixture/Fonts.rgba', ['application/octet-stream', bytes]);
  sources.push({file, bytes:bytes.length, sha256:crypto.createHash('sha256').update(bytes).digest('hex')});
}
(async () => {
  const server = http.createServer((request, response) => {
    if (request.url === '/') {
      response.writeHead(200, {'content-type':'text/html'});
      response.end('<!doctype html><title>UI atlas crop regression</title><link rel="icon" href="data:,"><main id="solomon-unforge-isolated-fixture"></main>');
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
    const {actual, worldFonts, gpu} = await page.evaluate(async moduleName => {
      const module = await import(`/probe/${moduleName}`);
      await module.default();
      const canvas = document.createElement('canvas');
      canvas.width = canvas.height = 512;
      const gl = canvas.getContext('webgl2', {premultipliedAlpha:false, preserveDrawingBuffer:true});
      if (!gl) throw Error('WebGL2 is unavailable');
      const debug = gl.getExtension('WEBGL_debug_renderer_info');
      const gpu = {renderer:debug ? gl.getParameter(debug.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER),
        version:gl.getParameter(gl.VERSION), attributes:gl.getContextAttributes()};
      let draws = 0;
      const drawArrays = gl.drawArrays.bind(gl);
      gl.drawArrays = (...args) => {draws++; return drawArrays(...args);};
      const rows = [];
      let worldFonts;
      const observe = method => {
        const expected = module.ui_atlas_fixture_render(method, true);
        const before = draws;
        const actual = module.ui_atlas_fixture_render(method, false);
        const actualDraws = draws - before;
        const restored = module.ui_atlas_fixture_render(method, true);
        let maximumDifference = 0, changedPixels = 0, restorationDifference = 0;
        for (let i = 0; i < actual.length; i += 4) {
          let changed = false;
          for (let channel = 0; channel < 4; channel++) {
            const delta = Math.abs(actual[i + channel] - expected[i + channel]);
            maximumDifference = Math.max(maximumDifference, delta);
            restorationDifference = Math.max(restorationDifference, Math.abs(restored[i + channel] - expected[i + channel]));
            changed ||= delta > 1;
          }
          changedPixels += Number(changed);
        }
        return {method, maximumDifference, changedPixels, restorationDifference, actualDraws};
      };
      try {
        for (const [atlas, records] of [['UI', [3, 62, 37, 17, 49]], ['Create', [9, 11, 12, 6]]]) {
          const response = await fetch(`/fixture/${atlas}.rgba`);
          if (!response.ok) throw Error(`Missing exact RGBA fixture ${atlas}`);
          const pixels = new Uint8Array(await response.arrayBuffer());
          for (const record of records) {
            await module.ui_atlas_fixture_init(gl, atlas, pixels, record);
            for (const method of ['raw', 'rotated', 'slice', 'frame', 'shadow3', 'shadow4', 'group']) {
              rows.push({atlas, record, ...observe(method)});
            }
            module.ui_atlas_fixture_dispose();
          }
        }
        for (const record of ['half-span', 'half-span-between-texels', 'fractional-origin']) {
          module.ui_authored_fixture_init(gl, record);
          for (const method of ['raw', 'rotated', 'shadow3', 'shadow4']) {
            rows.push({atlas:'Authored', record, ...observe(method)});
          }
          module.ui_atlas_fixture_dispose();
        }
        const response = await fetch('/fixture/Fonts.rgba');
        if (!response.ok) throw Error('Missing exact Fonts RGBA fixture');
        worldFonts = JSON.parse(module.world_font_fixture(gl, new Uint8Array(await response.arrayBuffer())));
      } finally {
        module.ui_atlas_fixture_dispose();
        gl.drawArrays = drawArrays;
        gl.getExtension('WEBGL_lose_context')?.loseContext();
      }
      return {actual:rows, worldFonts, gpu};
    }, moduleName);
    const receipt = {sources, binaries, backend, gpu, actual, worldFonts, errors, consoleErrors, failedRequests, failedResponses};
    if (process.env.OUT) {
      fs.mkdirSync(process.env.OUT, {recursive:true});
      fs.writeFileSync(path.join(process.env.OUT, 'receipt.json'), JSON.stringify(receipt, null, 2));
    }
    for (const failures of [errors, consoleErrors, failedRequests, failedResponses]) assert.deepEqual(failures, []);
    for (const row of actual) {
      assert.ok(row.maximumDifference <= 1, `UI record matches isolated crop: ${JSON.stringify(row)}`);
      assert.equal(row.restorationDifference, 0, `UI sampling scope restored: ${JSON.stringify(row)}`);
      if (row.method === 'raw') assert.equal(row.actualDraws, 1,
        `Two different records and the whole atlas retain one draw batch: ${JSON.stringify(row)}`);
    }
    for (const row of worldFonts) {
      assert.ok(row.maximumDifference <= 1, `World text matches independent point glyphs: ${JSON.stringify(row)}`);
      assert.equal(row.restored, true, `World font sampler scope restores: ${JSON.stringify(row)}`);
    }
    console.log(`Actual UI atlas crop and material batching passed: ${actual.length} cases, ${worldFonts.length} world font sizes`);
  } finally {
    if (owned) await owned.close();
    await new Promise(resolve => server.close(resolve));
  }
})().catch(error => {console.error(error); process.exitCode = 1;});
