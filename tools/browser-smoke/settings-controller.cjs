const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
// Disposable focused controller diagnostic; not full Settings qualification.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const gpu = require('./gpu-ui.cjs');
const output = process.env.OUT || '/tmp';
fs.mkdirSync(output, {recursive: true});

async function reveal(page, id) {
  for (let index = 0; index < 14; index += 1) {
    if (gpu.findWidget(await gpu.readUi(page), id)) return;
    await gpu.wheel(page, 'settings-content', 0, 330);
    await gpu.nextFrame(page);
  }
  throw new Error(`Settings control did not scroll into view: ${id}`);
}

async function focusedControl(page, id) {
  await gpu.waitForUi(page, state => {
    const control = gpu.findWidget(state, id);
    const content = gpu.findWidget(state, 'settings-content');
    if (!control?.focused || !content) return false;
    const row = gpu.bounds(control, {paint: true});
    const viewport = gpu.bounds(content);
    return row.x >= viewport.x && row.y >= viewport.y
      && row.x + row.width <= viewport.x + viewport.width
      && row.y + row.height <= viewport.y + viewport.height;
  }, `fully visible focused Settings control ${id}`);
}
(async () => {
  const owned = await launchOwnedBrowser(chromium,{headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: process.platform === 'darwin'
      ? ['--use-angle=metal', '--ignore-gpu-blocklist']
      : ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist']});
  const browser = owned.browser;
  const checks = [];
  const receipt = {checks, errors: [], pass: false};
  let page;
  try {
    page = await browser.newPage({viewport: {width: 1600, height: 900}, ignoreHTTPSErrors: process.env.LOCAL_TLS === '1'});
    const {errors, ready: errorsReady} = gpu.watchErrors(page);
    receipt.errors = errors;
    await errorsReady;
    await page.goto(process.env.URL || 'http://127.0.0.1:4181/');
    await gpu.ready(page);
    if (gpu.findWidget(await gpu.readUi(page), 'prompt-secondary')) await gpu.click(page, 'prompt-secondary');
    await gpu.click(page, 'settings');
    await gpu.click(page, 'settings-page:controls');
    await gpu.focused(page,'settings-toggle:reverse-hotbar');
    const bindings=['move-up','move-down','move-left','move-right','menu','inventory','skills','chat','cheats',
      ...Array.from({length:8},(_,index)=>`belt-${index}`),'cycle-hotbar'];
    await gpu.installController(page);
    await gpu.controller(page);
    for(const binding of bindings){
      await gpu.controller(page,[13]);
      await focusedControl(page,`bind:${binding}`);
      await gpu.controller(page);
    }
    await gpu.wheel(page,'settings-content',0,-300);
    await gpu.noWidget(page,'bind:cycle-hotbar');
    await gpu.nextFrame(page);
    assert.equal(gpu.findWidget(await gpu.readUi(page),'bind:cycle-hotbar'),undefined,
      'retained controller focus does not undo the user wheel');
    for(const id of [...bindings.slice(0,-1).reverse().map(binding=>`bind:${binding}`),'settings-toggle:reverse-hotbar']){
      await gpu.controller(page,[12]);
      await focusedControl(page,id);
      await gpu.controller(page);
    }
    checks.push('actual controller directions reveal every binding and retain wheel scrolling until focus moves');
    await gpu.assertGpuOnly(page);
    await gpu.settleErrors(page);
    assert.deepEqual(errors, []);
    receipt.behaviorPassed = true;
    console.log(JSON.stringify({checks, errors}));
  } catch (error) {
    receipt.failure = String(error.stack || error);
    if (page && !page.isClosed()) {
      try { await page.screenshot({timeout:120000,path: path.join(output, 'settings-failure.png')}); }
      catch (screenshotError) { receipt.screenshotFailure = String(screenshotError); }
    }
    throw error;
  } finally {
    try { if (page && !page.isClosed()) await gpu.settleErrors(page); }
    catch (error) { receipt.settleFailure = String(error); throw error; }
    finally {
      try {
        receipt.browserCleanup = await owned.close();
        receipt.pass = receipt.behaviorPassed === true && receipt.errors.length === 0 && !receipt.settleFailure;
        console.log(JSON.stringify({browserCleanup: receipt.browserCleanup}));
      } catch (error) {
        receipt.browserCleanup = error.cleanup || {failure: String(error)};
        throw error;
      } finally {
        fs.writeFileSync(path.join(output, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`);
      }
    }
  }
})().catch(error => {console.error(error); process.exitCode = 1;});
