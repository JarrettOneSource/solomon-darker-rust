// Exercises the actual offline map loader, scenery renderer, walking and map replacement.
const assert = require('node:assert/strict');
const { mkdir, writeFile } = require('node:fs/promises');
const path = require('node:path');
const { chromium } = require(path.resolve(process.env.PW));
const { startNewGame } = require('./menu-journey.cjs');

const output = process.env.OUT || path.resolve('boneyard-smoke');
const url = process.env.URL || 'http://127.0.0.1:4181/';

(async () => {
  await mkdir(output, { recursive: true });
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  });
  const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
  const errors = [];
  const receipt = { maps: [], errors };
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => {
    if (message.type() === 'error') errors.push(message.text());
  });
  try {
    await page.goto(url, { waitUntil: 'load' });
    await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0, null, { timeout: 60000 });
    await startNewGame(page);
    for (const index of [7, 0]) {
      await page.evaluate(async index => window.wasmBindings.enter_boneyard(index), index);
      await page.waitForTimeout(500);
      const state = () => page.evaluate(() => Array.from(window.wasmBindings.wizard_state()));
      const entered = await state();
      await page.screenshot({ path: path.join(output, `map-${index}-entry.png`) });
      await page.keyboard.down('s');
      await page.waitForTimeout(3000);
      await page.keyboard.up('s');
      const walked = await state();
      await page.screenshot({ path: path.join(output, `map-${index}-walk.png`) });
      assert.ok(walked[1] > entered[1] + 50, `map ${index}: native world must walk south`);
      assert.ok(walked[5] > entered[5], `map ${index}: clock must advance`);
      receipt.maps.push({ index, entered, walked });
    }
    assert.deepEqual(errors, []);
    assert.equal(await page.locator('#status').innerText(), '');
    receipt.status = 'passed';
  } catch (error) {
    receipt.status = 'failed';
    receipt.failure = error.stack;
    await page.screenshot({ path: path.join(output, 'failure.png') });
    process.exitCode = 1;
  } finally {
    await writeFile(path.join(output, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`);
    await browser.close();
  }
  console.log(JSON.stringify({ status: receipt.status, maps: receipt.maps.length, errors, failure: receipt.failure }));
})().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
