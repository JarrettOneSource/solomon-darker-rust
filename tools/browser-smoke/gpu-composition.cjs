// Runs the actual Rust Renderer, not a JavaScript copy of its submission path.
// PROBE_DIR is wasm-bindgen --target web output for the graphics composition_probe example.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const http = require('node:http');
const crypto = require('node:crypto');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
const directory = path.resolve(process.env.PROBE_DIR);
// Original atlas rectangles, retained separately from the renderer under test.
const stockRecords = require('./fixtures/stock-records.json');
const files = new Map([
  ['/composition_probe.js', ['text/javascript', fs.readFileSync(path.join(directory, 'composition_probe.js'))]],
  ['/composition_probe_bg.wasm', ['application/wasm', fs.readFileSync(path.join(directory, 'composition_probe_bg.wasm'))]],
  ['/stock-records.json', ['application/json', JSON.stringify(stockRecords)]],
  ['/allocation-blue.png', ['image/png', fs.readFileSync(path.join(__dirname, 'fixtures/allocation-blue.png'))]],
]);
for (const atlas of new Set(stockRecords.map(record => record.atlas))) {
  files.set(`/stock/${atlas}.png`, ['image/png', fs.readFileSync(
    path.join(__dirname, '../../web/assets/stock', `${atlas}.png`))]);
}

(async () => {
  const server = http.createServer((request, response) => {
    if (request.url === '/') {
      response.writeHead(200, {'content-type':'text/html'});
      response.end('<!doctype html><title>Rust GPU composition regression</title><link rel="icon" href="data:,">');
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
      args:[`--use-angle=${process.env.GPU_BACKEND || 'swiftshader'}`,'--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
    const browser = owned.browser;
    const page = await browser.newPage();
    const errors = [], consoleErrors = [], failedRequests = [], failedResponses = [];
    page.on('pageerror', error => errors.push(String(error)));
    page.on('console', message => {if (message.type() === 'error') consoleErrors.push(message.text());});
    page.on('requestfailed', request => failedRequests.push({url:request.url(), error:request.failure()}));
    page.on('response', response => {if (response.status() >= 400) failedResponses.push({url:response.url(), status:response.status()});});
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    const actual = await page.evaluate(async () => {
      const module = await import('/composition_probe.js');
      await module.default();
      const canvas = document.createElement('canvas');
      canvas.width = 5; canvas.height = 4;
      const gl = canvas.getContext('webgl2', {premultipliedAlpha:false, preserveDrawingBuffer:true});
      if (!gl) throw Error('WebGL2 is unavailable');
      const debug = gl.getExtension('WEBGL_debug_renderer_info');
      const gpu = {renderer:debug ? gl.getParameter(debug.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER),
        version:gl.getParameter(gl.VERSION)};
      const actual = {gpu, ...JSON.parse(module.composition_probe(gl)), sampling:JSON.parse(module.sampling_probe(gl)),
        spread:JSON.parse(module.spread_shadow_probe(gl))};
      let batchDraws = 0;
      const drawArrays = gl.drawArrays.bind(gl);
      gl.drawArrays = (...args) => {batchDraws++; return drawArrays(...args)};
      actual.indexedBatch = {...JSON.parse(module.indexed_batch_probe(gl)), draws:batchDraws};
      gl.drawArrays = drawArrays;
      actual.spritePrograms = JSON.parse(module.sprite_program_probe(gl));
      actual.atlasCrops = JSON.parse(module.atlas_crop_probe(gl));
      let atlasDraws = 0;
      gl.drawArrays = (...args) => {atlasDraws++; return drawArrays(...args)};
      actual.atlasBatch = {...JSON.parse(module.atlas_batch_probe(gl)), draws:atlasDraws};
      gl.drawArrays = drawArrays;
      const allocationImage = new Image();
      allocationImage.src = '/allocation-blue.png';
      await allocationImage.decode();
      const resources = [], restore = [];
      for (const kind of ['Texture', 'Framebuffer']) {
        const create = gl[`create${kind}`].bind(gl), destroy = gl[`delete${kind}`].bind(gl);
        restore.push(() => {gl[`create${kind}`] = create; gl[`delete${kind}`] = destroy;});
        gl[`create${kind}`] = (...args) => {
          const handle = create(...args);
          if (handle) resources.push({kind, handle, deletes:0});
          return handle;
        };
        gl[`delete${kind}`] = handle => {
          const resource = resources.find(resource => resource.kind === kind && resource.handle === handle);
          if (resource) resource.deletes++;
          return destroy(handle);
        };
      }
      try {
        actual.allocations = JSON.parse(module.allocation_binding_probe(gl, allocationImage, new Image()));
        actual.unboundAllocations = JSON.parse(module.allocation_unbound_probe(gl, allocationImage, new Image()));
        actual.allocationResources = resources.map(({kind, handle, deletes}) =>
          ({kind, deletes, live:gl[`is${kind}`](handle)}));
      } finally {
        for (const callback of restore) callback();
      }
      actual.stockRecords = [];
      const pages = new Map();
      for (const record of await (await fetch('/stock-records.json')).json()) {
        if (!pages.has(record.atlas)) {
          const bitmap = await createImageBitmap(await (await fetch(`/stock/${record.atlas}.png`)).blob());
          const source = document.createElement('canvas');
          source.width = bitmap.width; source.height = bitmap.height;
          const context = source.getContext('2d', {willReadFrequently:true});
          context.drawImage(bitmap, 0, 0); bitmap.close();
          pages.set(record.atlas, new Uint8Array(context.getImageData(0, 0, source.width, source.height).data));
        }
        for (const desiredScale of [0.7, 1.2, 3.5]) {
          const scale = Math.min(desiredScale, 230 / Math.max(record.source[2], record.source[3]));
          const mode = record.uv === 'native' ? 1 : 0;
          const images = [mode, mode | 2].map(mode => module.stock_record_probe(
            gl, record.width, record.height, pages.get(record.atlas),
            Uint32Array.from(record.source), mode, scale));
          let maximumDifference = 0;
          for (let index = 0; index < images[0].length; index++) {
            maximumDifference = Math.max(maximumDifference, Math.abs(images[0][index] - images[1][index]));
          }
          actual.stockRecords.push({atlas:record.atlas, record:record.record, scale, maximumDifference});
        }
      }
      actual.largeShadows = [];
      for (const scale of [1,2]) {
        const width=256*scale,height=192*scale,sigma=12*scale;
        canvas.width=width; canvas.height=height;
        const alpha=module.large_shadow_probe(gl,scale);
        const mask=document.createElement('canvas');mask.width=width;mask.height=height;
        const m=mask.getContext('2d');m.fillStyle='#fff';m.fillRect(40*scale,56*scale,176*scale,80*scale);
        const reference=document.createElement('canvas');reference.width=width;reference.height=height;
        const r=reference.getContext('2d');r.filter=`blur(${sigma}px)`;r.drawImage(mask,0,0);
        r.filter='none';r.globalCompositeOperation='destination-out';r.fillRect(48*scale,64*scale,160*scale,64*scale);
        const expected=r.getImageData(0,0,width,height).data;
        let delta=0,worst=null;
        for(let i=0;i<alpha.length;i++) {
          const difference=Math.abs(alpha[i]-expected[i*4+3]);
          if(difference>delta){delta=difference;worst={x:i%width,y:Math.floor(i/width),actual:alpha[i],expected:expected[i*4+3]};}
        }
        actual.largeShadows.push({scale,width,height,sigma,delta,worst});
      }
      return actual;
    });
    const expected = {
      overlap:[64,128,0,192], border:[128,0,0,128], screen:[64,128,0,208],
      outside:[0,0,0,255], faded:[32,64,0,195],
      outerInterior:[0,0,0,255], outerExterior:[0,0,255,255], restored:[128,0,0,191],
      innerLeft:[0,0,255,255], innerClear:[0,0,0,255], innerExterior:[0,0,0,255],
    };
    const binaries = ['composition_probe.js', 'composition_probe_bg.wasm'].map(file => ({
      file, sha256:crypto.createHash('sha256').update(fs.readFileSync(path.join(directory, file))).digest('hex'),
    }));
    const receipt = {binaries, actual, expected, errors, consoleErrors, failedRequests, failedResponses};
    if (process.env.OUT) {
      fs.mkdirSync(process.env.OUT, {recursive:true});
      fs.writeFileSync(path.join(process.env.OUT, 'receipt.json'), JSON.stringify(receipt, null, 2));
    }
    for (const failures of [errors, consoleErrors, failedRequests, failedResponses]) assert.deepEqual(failures, []);
    assert.equal(actual.allocations.length, 16);
    for (const row of actual.allocations) {
      assert.deepEqual(row.pixels, [255,0,0,255,255,0,0,255,0,255,0,255], `Allocation preserves both red draws: ${JSON.stringify(row)}`);
      for (const field of ['textureRestored', 'framebufferRestored', 'samplerRestored', 'viewportRestored']) {
        assert.equal(row[field], true, `${field}: ${JSON.stringify(row)}`);
      }
      assert.equal(row.rejected, row.kind.endsWith('-error'), `Allocation outcome: ${JSON.stringify(row)}`);
      assert.deepEqual(row.errors, [], `Allocation leaves clean GL state: ${JSON.stringify(row)}`);
    }
    assert.equal(actual.unboundAllocations.length, 8);
    for (const row of actual.unboundAllocations) {
      for (const field of ['unboundBefore', 'textureRestored', 'framebufferRestored', 'samplerRestored']) {
        assert.equal(row[field], true, `${field}: ${JSON.stringify(row)}`);
      }
      assert.equal(row.rejected, row.kind.endsWith('-error'), `Unbound allocation outcome: ${JSON.stringify(row)}`);
      assert.deepEqual(row.errors, [], `Unbound allocation GL state: ${JSON.stringify(row)}`);
    }
    for (const row of actual.allocationResources) {
      assert.equal(row.deletes, 1, `Allocation owns each handle through retirement: ${JSON.stringify(row)}`);
      assert.equal(row.live, false, `Allocation retires successful and rejected handles: ${JSON.stringify(row)}`);
    }
    assert.equal(actual.stockRecords.length, stockRecords.length * 3);
    for (const row of actual.stockRecords) {
      assert.ok(row.maximumDifference <= 1, `isolated stock record: ${JSON.stringify(row)}`);
    }
    assert.deepEqual(actual.atlasBatch, {
      firstEdge:[255,0,0,255], secondEdge:[0,255,0,255],
      unboundedJoin:[159,96,0,255], draws:1,
    });
    for (const row of actual.atlasCrops) {
      assert.ok(row.maximumDifference <= 1, `isolated atlas crop: ${JSON.stringify(row)}`);
      assert.equal(row.restored, true, `atlas crop state restoration: ${JSON.stringify(row)}`);
    }
    assert.equal(actual.spritePrograms.length, 12);
    for (const row of actual.spritePrograms) {
      assert.ok(row.maxChannelDifference <= 1, `sprite/effect equivalence: ${JSON.stringify(row)}`);
      assert.equal(row.restored, true, `sprite state restoration: ${JSON.stringify(row)}`);
    }
    assert.deepEqual(actual.indexedBatch, {
      red:[255,0,0,255], green:[0,255,0,255], outside:[0,0,0,255], capture:[0,255,255,255],
      clipRestored:true, clipDisabled:true, draws:2,
    });
    for (const key of ['allocationRestored','nestedRestored','screenRestored']) assert.equal(actual[key], true, key);
    for (const [name, rgba] of Object.entries(expected)) {
      assert.ok(rgba.every((channel, index) => Math.abs(channel - actual[name][index]) <= 1),
        `${name}: hand-derived SourceOver/group result ${JSON.stringify(receipt)}`);
    }
    const sampling = actual.sampling;
    for (const name of ['linear','nested','restored']) {
      assert.ok([230,0,26,255].every((channel, i) => Math.abs(channel-sampling[name][i])<=1), name);
    }
    assert.deepEqual(sampling.nearest, [255,0,0,255]);
    assert.deepEqual(sampling.parameters, [9729,9729,33071,33071]);
    for (const name of ['nestedRestored','unbound','deleted']) assert.equal(sampling[name], true, name);
    const spread = actual.spread;
    for (const [name, expected] of Object.entries({exterior:[0,0,255,255], interior:[0,0,0,0],
      antialias:[0,0,128,128], restored:[255,0,0,255], nestedInk:[0,255,0,255], throughHole:[0,255,0,255]})) {
      assert.ok(expected.every((channel,i)=>Math.abs(channel-spread[name][i])<=1),
        `spread ${name}: ${JSON.stringify(spread)}`);
    }
    for (const row of actual.largeShadows) assert.ok(row.delta<=9,
      `dense native Gaussian at scale${row.scale}: ${JSON.stringify(row)}`);
    console.log('Actual Rust nested GPU composition, projection and group opacity passed');
  } finally {
    if (owned) await owned.close();
    await new Promise(resolve => server.close(resolve));
  }
})().catch(error => {console.error(error); process.exitCode = 1});
