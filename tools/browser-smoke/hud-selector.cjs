// Real HUD opening, selection, cancellation and phone target sizing.
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require(path.resolve(process.env.PW));
const { startNewGame } = require('./menu-journey.cjs');

(async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
  });
  const errors = [];
  try {
    const page = await browser.newPage({ viewport: { width: 1600, height: 900 }, hasTouch: true });
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
    await page.goto(process.env.URL || 'http://127.0.0.1:4185/');
    await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0
      && !document.querySelector('#status').textContent, null, { timeout: 60000 });
    await startNewGame(page);
    const open = async () => {
      await page.locator('[data-hud-action="skill:12"]').click();
      await page.getByRole('dialog', { name: 'Select Primary Attack', exact: true }).waitFor();
    };
    await open();
    const option = page.locator('[data-panel-action="hud-select-skill:8"]');
    assert.equal(await option.getAttribute('aria-pressed'), 'true');
    const before = await page.evaluate(() => Array.from(wasmBindings.wizard_state()));
    await page.keyboard.down('KeyS');
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    await page.keyboard.up('KeyS');
    const after = await page.evaluate(() => Array.from(wasmBindings.wizard_state()));
    assert.deepEqual(after.slice(0, 2), before.slice(0, 2), 'the selector seals player movement');
    if (process.env.OUT) await page.screenshot({ path: path.join(process.env.OUT, 'hud-primary-selector-rust.png') });
    await option.click();
    await page.locator('.player-panels[hidden]').waitFor({ state: 'attached' });
    await open();
    await page.keyboard.press('Escape');
    await page.locator('.player-panels[hidden]').waitFor({ state: 'attached' });
    await page.setViewportSize({ width: 390, height: 844 });
    await open();
    const bounds = await option.boundingBox();
    assert.ok(bounds.width >= 44 && bounds.height >= 44 && bounds.x >= 0 && bounds.x + bounds.width <= 390);
    if (process.env.OUT) await page.screenshot({ path: path.join(process.env.OUT, 'phone-hud-primary-selector-rust.png') });
    await option.tap();
    await page.locator('.player-panels[hidden]').waitFor({ state: 'attached' });
    assert.deepEqual(errors, []);
    console.log(JSON.stringify({ passed: true, errors }));
  } finally {
    await browser.close();
  }
})().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
