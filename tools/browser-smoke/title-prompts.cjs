// Run against the shipped UI. PW, URL and OUT follow the other browser journeys.
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
    const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => {
      if (message.type() === 'error') errors.push(message.text());
    });
    await page.goto(process.env.URL || 'http://127.0.0.1:4185/');
    await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0
      && !document.querySelector('#status').textContent, null, { timeout: 60000 });
    await startNewGame(page, { name: 'Felix' });
    await page.keyboard.press('Escape');
    await page.locator('.game-menus[data-menu-screen="pause"]').waitFor();
    await page.locator('[data-menu-action="leave"]').click();
    await page.locator('.game-menus[data-menu-screen="title"]').waitFor();
    await page.locator('[data-menu-action="play"]').click();
    await page.locator('[data-menu-action="new-game"]').click();
    const prompt = page.getByRole('dialog', { name: 'Kill character?', exact: true });
    await prompt.waitFor();
    const before = await page.evaluate(() => Array.from(wasmBindings.wizard_state()));
    await page.keyboard.press('Escape');
    await prompt.waitFor({ state: 'hidden' });
    assert.equal(await page.locator('.game-menus').getAttribute('data-menu-screen'), 'play');
    const after = await page.evaluate(() => Array.from(wasmBindings.wizard_state()));
    assert.deepEqual(after, before, 'declining keeps the paused wizard intact');
    await page.locator('[data-menu-action="new-game"]').click();
    await prompt.waitFor();
    await page.setViewportSize({ width: 390, height: 844 });
    await page.waitForFunction(() => document.querySelector('.title-prompt-stage').getBoundingClientRect().width < 390);
    const yes = page.locator('[data-menu-action="prompt-primary"]');
    assert.ok((await yes.boundingBox()).height >= 44, 'phone confirmation remains touchable');
    if (process.env.OUT) {
      await page.screenshot({ path: path.join(process.env.OUT, 'phone-kill-character-rust.png') });
    }
    await yes.click();
    await page.getByRole('textbox', { name: 'Wizard name', exact: true }).waitFor();
    assert.deepEqual(errors, []);
    console.log(JSON.stringify({ passed: true, errors }));
  } finally {
    await browser.close();
  }
})().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
