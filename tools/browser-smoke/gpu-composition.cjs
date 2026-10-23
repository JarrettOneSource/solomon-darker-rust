// Runs the actual Rust Renderer, not a JavaScript copy of its submission path.
// PROBE_DIR is wasm-bindgen --target web output for the graphics composition_probe example.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const http = require('node:http');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const directory = path.resolve(process.env.PROBE_DIR);
const files = new Map([
  ['/composition_probe.js', ['text/javascript', fs.readFileSync(path.join(directory, 'composition_probe.js'))]],
  ['/composition_probe_bg.wasm', ['application/wasm', fs.readFileSync(path.join(directory, 'composition_probe_bg.wasm'))]],
]);

(async () => {
  const server = http.createServer((request, response) => {
    if (request.url === '/') {
      response.writeHead(200, {'content-type':'text/html'});
      response.end('<!doctype html><title>Rust GPU composition regression</title>');
      return;
    }
    const file = files.get(request.url);
    response.writeHead(file ? 200 : 404, {'content-type':file?.[0] || 'text/plain'});
    response.end(file?.[1] || 'Not found');
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  let browser;
  try {
    browser = await chromium.launch({headless:true,
      executablePath:process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
    const page = await browser.newPage();
    const errors = [];
    page.on('pageerror', error => errors.push(String(error)));
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    const actual = await page.evaluate(async () => {
      const module = await import('/composition_probe.js');
      await module.default();
      const canvas = document.createElement('canvas');
      canvas.width = 5; canvas.height = 4;
      const gl = canvas.getContext('webgl2', {premultipliedAlpha:false, preserveDrawingBuffer:true});
      if (!gl) throw Error('WebGL2 is unavailable');
      return JSON.parse(module.composition_probe(gl));
    });
    const expected = {
      overlap:[64,128,0,192], border:[128,0,0,128], screen:[64,128,0,208],
      outside:[0,0,0,255], faded:[32,64,0,195],
      outerInterior:[0,0,0,255], outerExterior:[0,0,255,255], restored:[128,0,0,191],
      innerLeft:[0,0,255,255], innerClear:[0,0,0,255], innerExterior:[0,0,0,255],
    };
    const receipt = {actual, expected, errors};
    if (process.env.OUT) {
      fs.mkdirSync(process.env.OUT, {recursive:true});
      fs.writeFileSync(path.join(process.env.OUT, 'receipt.json'), JSON.stringify(receipt, null, 2));
    }
    assert.deepEqual(errors, []);
    for (const key of ['allocationRestored','nestedRestored','screenRestored']) assert.equal(actual[key], true, key);
    for (const [name, rgba] of Object.entries(expected)) {
      assert.ok(rgba.every((channel, index) => Math.abs(channel - actual[name][index]) <= 1),
        `${name}: hand-derived SourceOver/group result ${JSON.stringify(receipt)}`);
    }
    console.log('Actual Rust nested GPU composition, projection and group opacity passed');
  } finally {
    if (browser) await browser.close();
    await new Promise(resolve => server.close(resolve));
  }
})().catch(error => {console.error(error); process.exitCode = 1});
