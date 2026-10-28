// One rejected stock image must retire callbacks belonging to cancelled sibling loads.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
const output = process.env.OUT || '/tmp';
fs.mkdirSync(output, {recursive: true});

(async () => {
  const owned = await launchOwnedBrowser(chromium, {headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: process.platform === 'darwin'
      ? ['--use-angle=metal', '--ignore-gpu-blocklist']
      : ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist']});
  const receipt = {pass: false, pageErrors: [], requestFailures: [], failedResponses: [], consoleErrors: []};
  let releaseImage;
  try {
    const page = await owned.browser.newPage({viewport: {width: 1280, height: 800}});
    page.on('pageerror', error => receipt.pageErrors.push(String(error)));
    page.on('requestfailed', request => receipt.requestFailures.push({url: request.url(), failure: request.failure()}));
    page.on('response', response => {
      if (response.status() >= 400) receipt.failedResponses.push({url: response.url(), status: response.status()});
    });
    page.on('console', message => {
      if (message.type() === 'error') receipt.consoleErrors.push({message: message.text(), location: message.location()});
    });
    await page.addInitScript(() => {
      const source = Object.getOwnPropertyDescriptor(HTMLImageElement.prototype, 'src');
      Object.defineProperty(HTMLImageElement.prototype, 'src', {...source, set(value) {
        if (value.endsWith('/Inventory.png')) {
          window.__pendingStockImage = this;
          this.addEventListener('load', () => { window.__pendingStockImageLoaded = true; }, {once: true});
        }
        source.set.call(this, value);
      }});
    });
    let inventoryRequested;
    const inventory = new Promise(resolve => { inventoryRequested = resolve; });
    const release = new Promise(resolve => { releaseImage = resolve; });
    await page.route('**/assets/stock/Inventory.png', async route => {
      inventoryRequested();
      await release;
      await route.continue();
    });
    await page.route('**/assets/stock/Fonts.png', async route => {
      await inventory;
      await route.fulfill({status: 503, contentType: 'text/plain', body: 'Intentional image-load failure'});
    });
    await page.goto(process.env.URL || 'http://127.0.0.1:4181/');
    await page.waitForFunction(() => {
      const read = window.wasmBindings?.gpu_ui_state;
      return read && JSON.parse(read()).error?.includes('Fonts.png');
    }, null, {timeout: 240000});
    receipt.cancelledImage = await page.evaluate(() => ({
      error: JSON.parse(window.wasmBindings.gpu_ui_state()).error,
      onloadDetached: window.__pendingStockImage.onload === null,
      onerrorDetached: window.__pendingStockImage.onerror === null,
    }));
    releaseImage();
    await page.waitForFunction(() => window.__pendingStockImageLoaded === true, null, {timeout: 120000});
    await page.screenshot({timeout: 120000, path: path.join(output, 'image-load-failure.png')});
    assert.equal(receipt.cancelledImage.onloadDetached, true, 'cancelled image load handler is detached');
    assert.equal(receipt.cancelledImage.onerrorDetached, true, 'cancelled image error handler is detached');
    assert.deepEqual(receipt.pageErrors, [], 'late native image events cannot invoke retired Rust closures');
    assert.deepEqual(receipt.requestFailures, []);
    assert.equal(receipt.failedResponses.length, 1);
    assert.ok(receipt.failedResponses[0].url.endsWith('/assets/stock/Fonts.png'));
    assert.equal(receipt.failedResponses[0].status, 503);
    assert.ok(receipt.consoleErrors.every(error =>
      error.message.includes('GPU UI stock art readiness: assets/stock/Fonts.png:')
      || (error.location.url.endsWith('/assets/stock/Fonts.png') && error.message.includes('503'))));
    receipt.behaviorPassed = true;
  } catch (error) {
    receipt.failure = String(error.stack || error);
    throw error;
  } finally {
    releaseImage?.();
    try {
      receipt.browserCleanup = await owned.close();
      receipt.pass = receipt.behaviorPassed === true;
    } finally {
      fs.writeFileSync(path.join(output, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n');
    }
  }
})().catch(error => {console.error(error); process.exitCode = 1;});
