// Real Website directory journey. Reads actual API rows; it never injects listings
// or joins a remote party. PW, URL, CHROME and OUT follow the other browser journeys.
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
    const page = await browser.newPage({viewport: {width: 1600, height: 900}});
    page.on('pageerror', (error) => errors.push(error.message));
    page.on('console', (message) => { if (message.type() === 'error') errors.push(message.text()); });
    const directoryResponses = [];
    page.on('response', (response) => {
      if (new URL(response.url()).pathname.endsWith('/api/game/parties') && response.ok()) {
        directoryResponses.push(response.json());
      }
    });
    await page.goto(process.env.URL || 'http://127.0.0.1:4181/', {waitUntil: 'load'});
    await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0
      && document.getElementById('status').textContent === '', null, {timeout: 60000});
    const prompt = page.locator('.title-prompt:not([hidden])');
    if (await prompt.isVisible()) await page.locator('[data-menu-action="prompt-secondary"]').click();
    await page.locator('[data-menu-action="play"]').click();
    await page.getByRole('button', {name: 'Join party', exact: true}).click();
    const scene = page.getByRole('region', {name: 'Join Party', exact: true});
    await scene.waitFor();
    await scene.locator('[role="list"][aria-busy="false"]').waitFor({timeout: 30000});
    assert.equal(await scene.locator('.join-party-error').isVisible(), false, 'actual directory API completed successfully');
    assert.ok(directoryResponses.length > 0, 'the view requested the configured Website directory');
    const directory = await directoryResponses.at(-1);
    const actualIds = await scene.locator('[data-party-listing]').evaluateAll((rows) => rows.map(row => row.dataset.partyListing).sort());
    assert.deepEqual(actualIds, directory.items.map(row => row.id).sort(), 'only actual broker listings are rendered');
    for (const party of directory.items) {
      const row = scene.locator('[data-party-listing]').filter({has: page.locator('strong', {hasText: party.leader.toUpperCase()})});
      const action = row.locator('button');
      assert.equal(await action.textContent(), party.status === 'playing' ? 'IN GAME' : party.visibility === 'public' ? 'JOIN' : 'REQUEST');
      assert.equal(await action.isDisabled(), party.status === 'playing');
    }
    const code = scene.getByRole('textbox', {name: 'Party ID', exact: true});
    await code.fill('0I-abcd-efgh');
    assert.equal(await code.inputValue(), 'ABCD-EFGH');
    await code.click();
    assert.equal(await code.evaluate((node) => document.activeElement === node), true, 'clicking an editable field does not submit its form');
    assert.equal(await scene.locator('button[data-menu-action="join-code"]').isEnabled(), true);
    await code.fill('0I');
    assert.equal(await code.inputValue(), '');
    assert.equal(await scene.locator('button[data-menu-action="join-code"]').isDisabled(), true);
    await page.screenshot({path: path.join(process.env.OUT || '/tmp', 'join-party-desktop.png')});
    await page.setViewportSize({width: 896, height: 414});
    await page.screenshot({path: path.join(process.env.OUT || '/tmp', 'join-party-landscape.png')});
    await scene.getByRole('button', {name: 'BACK', exact: true}).click();
    await page.locator('.game-menus[data-menu-screen="play"]').waitFor();
    assert.deepEqual(errors, []);
    console.log(JSON.stringify({checks: ['actual directory response rows', 'source listing admission', 'native Party ID edit/paste', 'input focus without form submission', 'incomplete code disabled', 'desktop and landscape layout', 'Back to Play'], errors}));
  } finally {
    await browser.close();
  }
})().catch((error) => { console.error(error); process.exitCode = 1; });
