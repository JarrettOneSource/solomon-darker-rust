// Source Settings page/capture/preference journey from the actual title. PW, URL,
// CHROME and OUT follow menus.cjs. No simulation state is injected.
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require(path.resolve(process.env.PW));

const url = process.env.URL || 'http://127.0.0.1:4181/';
const output = process.env.OUT || '/tmp';
const action = (page, id) => page.locator(`[data-menu-action="${id}"]`);
const setting = (page, id) => page.locator(`[data-setting="${id}"]`);
const currentPage = (page, id) => page.locator(`.game-settings-dialog[data-settings-page="${id}"]`);

(async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
  });
  const errors = [];
  try {
    const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
    page.on('pageerror', (error) => errors.push(error.message));
    page.on('console', (message) => { if (message.type() === 'error') errors.push(message.text()); });
    page.on('response', (response) => { if (response.status() >= 400) errors.push(`${response.status()} ${response.url()}`); });
    await page.goto(url, { waitUntil: 'load' });
    await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0
      && document.getElementById('status').textContent === '', null, { timeout: 60000 });
    const prompt = page.locator('.title-prompt:not([hidden])');
    if (await prompt.isVisible()) await action(page, 'prompt-secondary').click();
    await action(page, 'settings').click();
    await currentPage(page, 'root').waitFor();
    assert.equal(await page.evaluate(() => document.activeElement.dataset.setting), 'sound');
    assert.equal(await currentPage(page, 'root').locator('[data-native-ui-font="control-panel"]').count() > 10, true);
    assert.deepEqual(await currentPage(page, 'root').boundingBox().then(({x, y, width, height}) => ({x, y, width, height})),
      {x: 500, y: 100, width: 600, height: 700}, 'the native shell is centered at its authored desktop bounds');
    await setting(page, 'sound').evaluate((input) => {
      input.value = '37';
      input.dispatchEvent(new Event('input', {bubbles: true}));
    });
    await page.waitForFunction(() => document.querySelector('[data-setting="sound"]').getAttribute('aria-valuetext') === '37%');
    await page.screenshot({ path: path.join(output, 'settings-root.png') });

    await action(page, 'settings-page:cloud').click();
    await currentPage(page, 'cloud').waitFor();
    assert.equal(await setting(page, 'online').getAttribute('aria-pressed'), 'true');
    await setting(page, 'global-chat').click();
    assert.equal(await setting(page, 'activity').isDisabled(), true);
    assert.equal(await setting(page, 'activity').getAttribute('aria-pressed'), 'false');
    await setting(page, 'online').click();
    for (const id of ['activity', 'global-chat', 'shared-hub', 'submit-runs']) {
      assert.equal(await setting(page, id).isDisabled(), true, `${id} follows online admission`);
      assert.equal(await setting(page, id).getAttribute('aria-pressed'), 'false');
    }
    await setting(page, 'online').click();
    assert.equal(await setting(page, 'shared-hub').getAttribute('aria-pressed'), 'true', 'dependent values remain stored while disabled');
    assert.equal(await setting(page, 'global-chat').getAttribute('aria-pressed'), 'false');
    await setting(page, 'global-chat').click();
    assert.equal(await setting(page, 'activity').getAttribute('aria-pressed'), 'true', 'stored activity preference returns with its parent');
    await page.screenshot({ path: path.join(output, 'settings-cloud.png') });
    await page.keyboard.press('Escape');
    await currentPage(page, 'root').waitFor();

    await action(page, 'settings-page:controls').click();
    await currentPage(page, 'controls').waitFor();
    assert.equal(await page.locator('[data-binding-action]').count(), 18);
    const moveUp = action(page, 'bind:move-up');
    await moveUp.click();
    await page.keyboard.press('Escape');
    await page.waitForFunction(() => document.querySelector('[data-binding-action="moveUp"]').dataset.bindingCode === 'Escape');
    assert.equal(await action(page, 'bind:menu').getAttribute('data-binding-code'), 'KeyW');
    await moveUp.click();
    await action(page, 'settings-back').click();
    assert.equal(await moveUp.getAttribute('data-awaiting-binding'), null, 'footer Back cancels capture before leaving Controls');
    assert.equal(await currentPage(page, 'controls').isVisible(), true);
    await action(page, 'bind:cheats').click();
    await page.keyboard.press('F2');
    await page.waitForFunction(() => document.querySelector('[data-binding-action="openCheats"]').dataset.bindingCode === 'F2');
    await page.screenshot({ path: path.join(output, 'settings-controls.png') });
    await action(page, 'settings-back').click();
    await currentPage(page, 'root').waitFor();

    await action(page, 'settings-page:performance').click();
    await currentPage(page, 'performance').waitFor();
    const lighting = await setting(page, 'lighting').getAttribute('aria-pressed');
    await setting(page, 'lighting').click();
    assert.notEqual(await setting(page, 'lighting').getAttribute('aria-pressed'), lighting);
    assert.equal(await page.getByText('Fixed for synchronized multiplayer presentation.', {exact: true}).isVisible(), true);
    await page.screenshot({ path: path.join(output, 'settings-performance.png') });
    await action(page, 'settings-back').click();
    await currentPage(page, 'root').waitFor();
    assert.equal(await setting(page, 'sound').inputValue(), '37', 'range edits survive page changes');
    await action(page, 'settings-back').click();
    await page.locator('.game-menus[data-menu-screen="title"]').waitFor();
    await action(page, 'settings').click();
    await currentPage(page, 'root').waitFor();
    assert.equal(await setting(page, 'sound').inputValue(), '37');
    await action(page, 'settings-page:controls').click();
    await currentPage(page, 'controls').waitFor();
    assert.equal(await action(page, 'bind:cheats').getAttribute('data-binding-code'), 'F2');
    await action(page, 'settings-back').click();
    await action(page, 'settings-back').click();
    await page.reload({waitUntil: 'load'});
    await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0
      && document.getElementById('status').textContent === '', null, { timeout: 60000 });
    if (await prompt.isVisible()) await action(page, 'prompt-secondary').click();
    await action(page, 'settings').click();
    await currentPage(page, 'root').waitFor();
    assert.equal(await setting(page, 'sound').inputValue(), '37', 'actual stored preferences survive reload');
    await page.setViewportSize({width: 896, height: 414});
    await page.screenshot({path: path.join(output, 'settings-landscape.png')});
    const content = currentPage(page, 'root').locator('.game-settings-content');
    assert.equal(await content.evaluate((node) => node.scrollHeight > node.clientHeight), true, 'small native settings retains its scrolling content');
    assert.deepEqual(errors, []);
    console.log(JSON.stringify({checks: ['native shell and bitmap labels', 'continuous range', 'online dependency preservation', '17 controls and OpenCheats', 'Escape swap and footer capture cancellation', 'performance policy', 'page Back versus Done', 'actual preference reload', 'small native scroll'], errors}));
  } finally {
    await browser.close();
  }
})().catch((error) => { console.error(error); process.exitCode = 1; });
