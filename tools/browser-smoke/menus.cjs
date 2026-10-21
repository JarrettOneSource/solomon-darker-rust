// Offline title/Create/game/pause/settings/Continue journey. PW, URL, CHROME follow
// smoke.cjs; OUT selects an existing screenshot directory.
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require(path.resolve(process.env.PW));
const { reachCollegeLoadout } = require('./menu-journey.cjs');
const url = process.env.URL || 'http://127.0.0.1:4185/';
const output = process.env.OUT || '/tmp';
const state = (page) => page.evaluate(() => Array.from(window.wasmBindings.wizard_state()));
const screen = (page, name) => page.locator(`.game-menus[data-menu-screen="${name}"]`).waitFor({ state: 'attached', timeout: 20000 });
const shot = (page, name) => page.screenshot({ path: path.join(output, name) });

async function ticks(page, count) {
  const start = (await state(page))[5];
  await page.waitForFunction((target) => window.wasmBindings.wizard_state()[5] >= target,
    start + count, { timeout: 20000 });
}

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
    await screen(page, 'title');
    const prompt = page.locator('.title-prompt:not([hidden])');
    if (await prompt.isVisible()) {
      await prompt.locator('[data-menu-action="prompt-secondary"]').click();
    }
    await shot(page, 'menu-title.png');
    assert.equal((await state(page))[5], 0, 'the title does not advance the world');
    await page.locator('[data-menu-action="play"]').click();
    assert.equal(await page.locator('[data-menu-action="continue"]').isDisabled(), true);
    await page.locator('[data-menu-action="new-game"]').click();
    await screen(page, 'hidden');
    await reachCollegeLoadout(page);
    assert.equal(await page.locator('[data-menu-action="create-back"]').isDisabled(), true,
      'the sole character-selection screen belongs to College admission');
    const wizardName = page.getByRole('textbox', { name: 'Wizard name', exact: true });
    await wizardName.fill('AAAAAAAAAAA');
    assert.equal(await wizardName.inputValue(), 'AAAAAAAAAAA', 'the native 363-unit name is admitted');
    await wizardName.fill('AAAAAAAAAAAA');
    assert.equal(await wizardName.inputValue(), 'AAAAAAAAAAA', 'the native 396-unit edit is rejected without replacing the accepted name');
    assert.equal(await page.locator('.create-validation').textContent(), 'Wizard name is too wide.');
    await wizardName.fill('Solon-Solus');
    assert.equal(await wizardName.inputValue(), 'AAAAAAAAAAA', 'unsupported stock heading glyphs are rejected at edit time');
    await wizardName.fill('');
    assert.equal(await page.locator('.create-validation').textContent(), '', 'an empty draft finalizes as native Genericus');
    assert.equal(await wizardName.getAttribute('aria-invalid'), null);
    await page.waitForFunction(() => !document.querySelector('[data-menu-action="randomize-loadout"]').disabled);
    await wizardName.fill('Felix');
    assert.equal(await page.locator('.create-name-value [data-native-ui-glyph]').count(), 5, 'Create name is painted with the stock heading atlas');
    await page.locator('[data-menu-action="element:ether"]').waitFor({ state: 'visible' });
    await page.waitForTimeout(1100);
    await shot(page, 'menu-create-elements.png');
    await page.locator('[data-menu-action="element:ether"]').click();
    await page.locator('[data-menu-action="discipline:arcane"]').waitFor({ state: 'visible' });
    await page.waitForTimeout(1100);
    await shot(page, 'menu-create-ether.png');
    await page.locator('[data-menu-action="discipline:arcane"]').click();
    await screen(page, 'hidden');
    await page.waitForFunction(() => window.wasmBindings.wizard_state()[6] === 0
      && window.wasmBindings.wizard_state()[7] === -1, null, { timeout: 30000 });
    await ticks(page, 5);
    assert.equal(await page.locator('[role="meter"][aria-label="Health"]').getAttribute('aria-valuenow'), '50');
    assert.equal(await page.locator('[role="meter"][aria-label="Mana"]').getAttribute('aria-valuenow'), '100');
    await shot(page, 'menu-new-game.png');
    await page.keyboard.down('KeyS');
    await ticks(page, 70);
    await page.keyboard.up('KeyS');
    await ticks(page, 35);
    await page.keyboard.press('Escape');
    await screen(page, 'pause');
    await page.waitForTimeout(400);
    const paused = await state(page);
    await page.keyboard.down('KeyS');
    await ticks(page, 30);
    await page.keyboard.up('KeyS');
    const afterPause = await state(page);
    assert.ok(afterPause[5] > paused[5], 'College actors continue while its menu is open');
    assert.deepEqual(afterPause.slice(0, 2), paused.slice(0, 2), 'College pause seals player movement');
    await shot(page, 'menu-pause.png');
    await page.locator('[data-menu-action="pause-settings"]').click();
    await screen(page, 'settings');
    await page.getByLabel('UI SCALE', { exact: true }).evaluate((input) => {
      input.value = '135';
      input.dispatchEvent(new Event('change', { bubbles: true }));
    });
    await shot(page, 'menu-settings-root.png');
    await page.locator('[data-menu-action="settings-page:controls"]').click();
    await page.locator('[data-settings-page="controls"]').waitFor();
    const moveUpBinding = page.locator('[data-menu-action="bind:move-up"]');
    await moveUpBinding.click();
    await page.keyboard.press('Escape');
    await page.waitForFunction(() => document.querySelector('[data-menu-action="bind:move-up"]').dataset.bindingCode === 'Escape');
    assert.equal(await page.locator('[data-menu-action="bind:menu"]').getAttribute('data-binding-code'), 'KeyW', 'Escape rebind swaps the previous Menu control');
    await screen(page, 'settings');
    await moveUpBinding.click();
    await page.keyboard.press('Control+q');
    assert.equal(await moveUpBinding.getAttribute('data-awaiting-binding'), '', 'modified keys do not finish capture');
    const panelBounds = await page.locator('.settings-panel').boundingBox();
    await page.mouse.click(panelBounds.x + 15, panelBounds.y + 15, {button: 'right'});
    assert.equal(await moveUpBinding.getAttribute('data-awaiting-binding'), '', 'movement cannot bind a mouse button');
    await page.keyboard.press('KeyJ');
    await page.waitForFunction(() => document.querySelector('[data-menu-action="bind:move-up"]').dataset.bindingCode === 'KeyJ');
    await page.locator('[data-menu-action="bind:menu"]').click();
    await page.keyboard.press('Escape');
    await page.waitForFunction(() => document.querySelector('[data-menu-action="bind:menu"]').dataset.bindingCode === 'Escape');
    await page.locator('[data-menu-action="bind:move-down"]').click();
    await page.keyboard.press('ArrowUp');
    await page.waitForFunction(() => document.querySelector('[data-menu-action="bind:move-down"]').dataset.bindingCode === 'ArrowUp');
    await shot(page, 'menu-settings-keyboard.png');
    await page.locator('[data-menu-action="settings-back"]').click();
    await page.locator('[data-settings-page="root"]').waitFor();
    await page.locator('[data-menu-action="settings-back"]').click();
    await screen(page, 'pause');
    await page.locator('[data-menu-action="resume"]').click();
    await screen(page, 'hidden');
    await ticks(page, 5);
    const meter = await page.locator('.hud-health').boundingBox();
    assert.ok(Math.abs(meter.width - 148.5) < 0.01, 'UI scale reaches the actual HUD');
    const beforeRebound = await state(page);
    await page.keyboard.down('KeyJ');
    await ticks(page, 60);
    await page.keyboard.up('KeyJ');
    await ticks(page, 40);
    const afterRebound = await state(page);
    assert.ok(afterRebound[1] < beforeRebound[1] - 20, 'the rebound movement key reaches simulation');
    await page.keyboard.down('KeyW');
    await ticks(page, 40);
    await page.keyboard.up('KeyW');
    const afterOldKey = await state(page);
    assert.ok(Math.hypot(afterOldKey[0] - afterRebound[0], afterOldKey[1] - afterRebound[1]) < 0.1, 'the replaced key is released');
    await page.keyboard.down('ArrowUp');
    await ticks(page, 60);
    await page.keyboard.up('ArrowUp');
    await ticks(page, 40);
    assert.ok((await state(page))[1] > afterOldKey[1] + 20, 'an assigned arrow overrides its legacy direction');
    await page.keyboard.press('Escape');
    await screen(page, 'pause');
    await page.locator('[data-menu-action="leave"]').click();
    await screen(page, 'title');
    const retained = await state(page);
    await page.locator('[data-menu-action="play"]').click();
    assert.equal(await page.locator('[data-menu-action="continue"]').isDisabled(), false);
    await page.locator('[data-menu-action="continue"]').click();
    await screen(page, 'hidden');
    assert.ok(Math.hypot((await state(page))[0] - retained[0], (await state(page))[1] - retained[1]) < 0.1, 'Continue returns to the same local world');
    assert.deepEqual(errors, []);
    console.log(JSON.stringify({ checks: ['title clock', 'College admission before the sole loadout', 'stock Create effects', 'new-game HUD', 'College pause seals input while actors advance', 'settings', 'key rebinding', 'HUD scale', 'Continue'], errors }));
  } finally {
    await browser.close();
  }
})().catch((error) => { console.error(error); process.exitCode = 1; });
