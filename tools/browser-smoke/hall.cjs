// Production Hall navigation/empty archive. Populated archive acceptance belongs to a real run.
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require(path.resolve(process.env.PW));

(async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
  });
  const errors = [];
  try {
    const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => {
      if (message.type() === 'error') errors.push(message.text());
    });
    await page.goto(process.env.URL || 'http://127.0.0.1:4185/');
    await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0
      && !document.querySelector('#status').textContent, null, { timeout: 60000 });
    const open = async () => {
      await page.getByRole('button', { name: 'Hall of Fame', exact: true }).click();
      await page.locator('.game-menus[data-menu-screen="hall"]').waitFor();
    };
    await open();
    await page.getByRole('status').filter({ hasText: 'No records yet.' }).waitFor();
    assert.equal(await page.locator('.hall-screen').getAttribute('data-hall-entry-count'), '0');
    assert.equal(await page.getByText('Global', { exact: true }).count(), 0);
    if (process.env.OUT) await page.screenshot({ path: path.join(process.env.OUT, 'hall-empty-rust.png') });
    await page.keyboard.press('Escape');
    await page.locator('.game-menus[data-menu-screen="title"]').waitFor();
    await page.setViewportSize({ width: 390, height: 844 });
    await open();
    const empty = await page.getByRole('status').filter({ hasText: 'No records yet.' }).boundingBox();
    assert.ok(empty.x >= 0 && empty.x + empty.width <= 390, 'phone centers the native record canvas after opening');
    const back = page.getByRole('button', { name: 'MAIN MENU', exact: true });
    const bounds = await back.boundingBox();
    assert.ok(bounds.height >= 44 && bounds.x >= 0 && bounds.x + bounds.width <= 390);
    if (process.env.OUT) await page.screenshot({ path: path.join(process.env.OUT, 'phone-hall-empty-rust.png') });
    await back.click();
    await page.locator('.game-menus[data-menu-screen="title"]').waitFor();
    assert.deepEqual(errors, []);
    console.log(JSON.stringify({ passed: true, errors }));
  } finally {
    await browser.close();
  }
})().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
