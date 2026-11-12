// Compare the loader's actual text owner with independent point-sampled glyph crops.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const crypto = require('node:crypto');
const {chromium} = require(path.resolve(process.env.PW));
const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
const directory = path.resolve(process.env.LOADER_DIR);
const backend = process.env.GPU_BACKEND || 'native';
const source = path.join(path.resolve(process.env.FIXTURE_DIR), 'Fonts.rgba');
const pixels = fs.readFileSync(source);
const files = new Map([['/Fonts.rgba', ['application/octet-stream', pixels]]]);
const binaries = [];
for (const name of fs.readdirSync(directory).filter(name => /\.(js|wasm)$/.test(name))) {
  const bytes = fs.readFileSync(path.join(directory, name));
  files.set(`/${name}`, [name.endsWith('.wasm') ? 'application/wasm' : 'text/javascript', bytes]);
  binaries.push({name, bytes:bytes.length, sha256:crypto.createHash('sha256').update(bytes).digest('hex')});
}
(async () => {
  const server = http.createServer((request, response) => {
    const file = request.url === '/' ? ['text/html', '<!doctype html><title>Loader font regression</title><link rel="icon" href="data:,">'] : files.get(request.url);
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
      const module = await import('/solomon_loader.js');
      await module.default();
      const canvas = document.createElement('canvas');
      canvas.width = canvas.height = 256;
      const gl = canvas.getContext('webgl2', {premultipliedAlpha:false, preserveDrawingBuffer:true});
      if (!gl) throw Error('WebGL2 unavailable');
      const debug = gl.getExtension('WEBGL_debug_renderer_info');
      const gpu = {renderer:debug ? gl.getParameter(debug.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER),
        version:gl.getParameter(gl.VERSION), attributes:gl.getContextAttributes()};
      try {
        const response = await fetch('/Fonts.rgba');
        if (!response.ok) throw Error('Missing exact Fonts RGBA bytes');
        const pixels = new Uint8Array(await response.arrayBuffer());
        return {actual:JSON.parse(module.loader_font_fixture(gl, pixels)), gpu};
      } finally {
        gl.getExtension('WEBGL_lose_context')?.loseContext();
      }
    });
    const receipt = {source, sourceSha256:crypto.createHash('sha256').update(pixels).digest('hex'), binaries, backend, gpu, actual,
      errors, consoleErrors, failedRequests, failedResponses};
    if (process.env.OUT) {
      fs.mkdirSync(process.env.OUT, {recursive:true});
      fs.writeFileSync(path.join(process.env.OUT, 'receipt.json'), JSON.stringify(receipt, null, 2));
    }
    for (const failures of [errors, consoleErrors, failedRequests, failedResponses]) assert.deepEqual(failures, []);
    for (const row of actual) {
      assert.deepEqual(row.backgrounds[0], row.backgrounds[1], 'Reference uploads preserve the actual background');
      assert.deepEqual(row.backgrounds[0], row.backgrounds[2], 'Reference uploads preserve the restored background');
      assert.ok(row.backgrounds[0].slice(0, 3).every((channel, index) => Math.abs(channel - [26,51,77][index]) <= 1), 'Background is the declared solid RGB within byte rounding');
      assert.equal(row.backgrounds[0][3], 255, 'Background remains opaque');
      assert.ok(row.maximumDifference <= 1, `Loader stock glyphs use point samples: ${JSON.stringify(row)}`);
      assert.equal(row.restored, true, `Loader sampler scope restores: ${JSON.stringify(row)}`);
    }
    console.log(`Actual loader font crop comparison passed: ${actual.length} sizes`);
  } finally {
    if (owned) await owned.close();
    await new Promise(resolve => server.close(resolve));
  }
})().catch(error => {console.error(error); process.exitCode = 1;});
