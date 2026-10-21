// Controller-only New Game and modal handoffs, then a real touch menu journey.
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require(path.resolve(process.env.PW));
const { leaveCollegeOffice } = require('./menu-journey.cjs');
const url = process.env.URL || 'http://127.0.0.1:4185/';
const output = process.env.OUT || '/tmp';
async function screen(page, name) {
  try {
    await page.locator(`.game-menus[data-menu-screen="${name}"]`).waitFor({ state: 'attached', timeout: 10000 });
  } catch (error) {
    const actual = await page.evaluate(() => ({ screen: document.querySelector('.game-menus')?.dataset.menuScreen,
      focusedAction: document.activeElement?.getAttribute('data-menu-action'), pad: window.menuTestPad,
      wizard: Array.from(window.wasmBindings.wizard_state()),
      input: Array.from(window.wasmBindings.gameplay_input_state()),
      dialogue: document.querySelector('[data-dialogue-key]')?.dataset.dialogueKey }));
    await page.screenshot({path: path.join(output, `menu-input-failure-${name}.png`)});
    throw new Error(`Expected ${name}: ${JSON.stringify(actual)}`, {cause: error});
  }
}

async function ready(page) {
  await page.goto(url, { waitUntil: 'load' });
  await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0
    && document.getElementById('status').textContent === '', null, { timeout: 60000 });
}

async function pad(page, buttons = [], x = 0, y = 0) {
  await page.evaluate(({buttons, x, y}) => { window.menuTestPad = {buttons, x, y}; }, {buttons, x, y});
  // One completed game frame consumes the edge. Holding across several slow frames can
  // legitimately cross the source's direction-repeat delay under fleet load.
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(resolve)));
}

async function focusMenuAction(page, action) {
  for (let presses = 0; presses < 4; presses += 1) {
    if (await page.evaluate(() => document.activeElement?.dataset.menuAction) === action) return;
    await pad(page);
    await pad(page, [13]);
  }
  assert.equal(await page.evaluate(() => document.activeElement?.dataset.menuAction), action);
}

async function controllerCollegeLoadout(page) {
  const panel = page.locator('.player-panels:not([hidden])');
  await panel.locator('[data-player-panel="dialogue"]').waitFor({timeout: 120000});
  await pad(page);
  await pad(page, [0]);
  await panel.locator('[data-panel-action="dialogue-choice:0"]').waitFor();
  // The retained Done button still owns focus after speech. Cycle to the real first
  // question rather than confirming Done and skipping the conversation.
  for (let presses = 0; presses < 5; presses += 1) {
    if (await page.evaluate(() => document.activeElement?.dataset.panelAction) === 'dialogue-choice:0') break;
    await pad(page);
    await pad(page, [5]);
  }
  assert.equal(await page.evaluate(() => document.activeElement?.dataset.panelAction), 'dialogue-choice:0');
  await pad(page);
  await pad(page, [0]);
  await panel.locator('[data-dialogue-key="ARCH_Q1_0"]').waitFor();
  await pad(page);
  await pad(page, [1]);
  await panel.locator('[data-dialogue-key="ARCH_DISMISS_0"]').waitFor();
  await pad(page);
  await pad(page, [1]);
  await page.locator('.player-panels[hidden]').waitFor({state: 'attached'});
  await pad(page);
  await leaveCollegeOffice(page, (x, y) => pad(page, [], x, y));
}

async function touchCollegeLoadout(page) {
  const panel = page.locator('.player-panels:not([hidden])');
  await panel.locator('[data-player-panel="dialogue"]').waitFor({timeout: 120000});
  await panel.locator('[data-panel-action="dialogue-next"]').tap();
  await panel.locator('[data-panel-action="dialogue-choice:0"]').tap();
  await panel.locator('[data-dialogue-key="ARCH_Q1_0"]').waitFor();
  await panel.locator('[data-panel-action="dialogue-next"]').tap();
  await panel.locator('[data-panel-action="dialogue-choice:0"]').waitFor();
  await panel.locator('[data-panel-action="dialogue-next"]').tap();
  await panel.locator('[data-dialogue-key="ARCH_DISMISS_0"]').waitFor();
  await panel.locator('[data-panel-action="dialogue-next"]').tap();
  await page.locator('.player-panels[hidden]').waitFor({state: 'attached'});
  const joystick = await page.locator('[data-joystick="movement"]').boundingBox();
  const x = joystick.x + joystick.width / 2;
  const y = joystick.y + joystick.height / 2;
  const input = await page.context().newCDPSession(page);
  await input.send('Input.dispatchTouchEvent', {type: 'touchStart', touchPoints: [{x, y, id: 1}]});
  try {
    await leaveCollegeOffice(page, (dx, dy) => input.send('Input.dispatchTouchEvent', {
      type: 'touchMove',
      touchPoints: [{x: x + dx * joystick.width * 0.4, y: y + dy * joystick.height * 0.4, id: 1}],
    }));
  } finally {
    await input.send('Input.dispatchTouchEvent', {type: 'touchEnd', touchPoints: []});
    await input.detach();
  }
}

async function settledCollege(page) {
  await page.waitForFunction(() => window.wasmBindings.wizard_state()[6] === 0
    && window.wasmBindings.wizard_state()[7] === -1, null, {timeout: 30000});
}

(async () => {
  const browser = await chromium.launch({ headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
  });
  const errors = [];
  const watch = (page) => {
    page.on('pageerror', (error) => errors.push(error.message));
    page.on('console', (message) => { if (message.type() === 'error') errors.push(message.text()); });
    page.on('response', (response) => { if (response.status() >= 400) errors.push(`${response.status()} ${response.url()}`); });
  };
  try {
    const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
    watch(page);
    await ready(page);
    await page.evaluate(() => {
      window.menuTestPad = {buttons: [], x: 0, y: 0};
      const gamepad = Object.create(Gamepad.prototype, {
        index: {value: 0}, connected: {value: true}, mapping: {value: 'standard'},
        axes: {get: () => [window.menuTestPad.x, window.menuTestPad.y]},
        buttons: {get: () => Array.from({length: 16}, (_, i) => Object.create(GamepadButton.prototype, {
          pressed: {value: window.menuTestPad.buttons.includes(i)},
          value: {value: window.menuTestPad.buttons.includes(i) ? 1 : 0},
        }))},
      });
      Object.defineProperty(navigator, 'getGamepads', {value: () => [gamepad]});
    });
    await pad(page);
    if (await page.locator('.title-prompt:not([hidden])').isVisible()) {
      await pad(page, [1]);
      await page.locator('.title-prompt[hidden]').waitFor({state: 'attached'});
      await pad(page);
    }
    await focusMenuAction(page, 'settings');
    await pad(page);
    await pad(page, [0]);
    await screen(page, 'settings');
    await pad(page);
    assert.equal(await page.evaluate(() => Boolean(document.activeElement.closest('.game-settings-backdrop'))), true, 'title settings owns controller focus');
    await pad(page, [1]);
    await screen(page, 'title');
    await pad(page);
    await pad(page, [0]);
    await screen(page, 'play');
    await page.waitForTimeout(400);
    await screen(page, 'play');
    await pad(page);
    await pad(page, [0]);
    await screen(page, 'hidden');
    await controllerCollegeLoadout(page);
    await screen(page, 'create');
    assert.equal(await page.locator('[data-menu-action="create-back"]').isDisabled(), true);
    await pad(page, [1]);
    await screen(page, 'create');
    await pad(page);
    await page.locator('[data-menu-action="element:earth"]').waitFor({state: 'visible'});
    await pad(page, [0]);
    await page.locator('[data-menu-action="discipline:arcane"]').waitFor({state: 'visible', timeout: 8000});
    await pad(page);
    await pad(page, [0]);
    await screen(page, 'hidden');
    await pad(page);
    await settledCollege(page);
    await pad(page, [9]);
    await screen(page, 'pause');
    await pad(page);
    await pad(page, [13]);
    assert.equal(await page.evaluate(() => document.activeElement.dataset.menuAction), 'pause-settings');
    await pad(page);
    await pad(page, [0]);
    await screen(page, 'settings');
    await pad(page);
    assert.equal(await page.evaluate(() => Boolean(document.activeElement.closest('.game-settings-backdrop'))), true);
    await pad(page, [1]);
    await screen(page, 'pause');
    await pad(page);
    await pad(page, [1]);
    await screen(page, 'hidden');
    await page.close();

    const touch = await browser.newPage({ viewport: {width: 390, height: 844}, hasTouch: true, isMobile: true });
    watch(touch);
    await ready(touch);
    const prompt = touch.locator('.title-prompt:not([hidden])');
    if (await prompt.isVisible()) {
      await prompt.locator('[data-menu-action="prompt-secondary"]').tap();
    }
    await touch.screenshot({path: path.join(output, 'menu-touch-title.png')});
    await touch.locator('[data-menu-action="play"]').tap();
    await touch.locator('[data-menu-action="new-game"]').tap();
    await screen(touch, 'hidden');
    await touchCollegeLoadout(touch);
    await touch.getByRole('textbox', {name: 'Wizard name', exact: true}).fill('Felix');
    await touch.locator('[data-menu-action="element:water"]').waitFor({state: 'visible'});
    const element = await touch.locator('[data-menu-action="element:air"]').boundingBox();
    assert.ok(element.width >= 43.99 && element.height >= 43.99, 'small element art retains a finger-sized target');
    await touch.screenshot({path: path.join(output, 'menu-touch-create.png')});
    await touch.locator('[data-menu-action="element:water"]').tap();
    await touch.locator('[data-menu-action="discipline:body"]').tap();
    await screen(touch, 'hidden');
    await settledCollege(touch);
    await touch.locator('.hud-menu').tap();
    await screen(touch, 'pause');
    await touch.waitForTimeout(350);
    const resume = await touch.locator('[data-menu-action="resume"]').boundingBox();
    assert.ok(resume.height >= 44, 'pause rows remain usable on a phone');
    await touch.screenshot({path: path.join(output, 'menu-touch-pause.png')});
    await touch.locator('[data-menu-action="resume"]').tap();
    await screen(touch, 'hidden');
    assert.deepEqual(errors, []);
    console.log(JSON.stringify({checks: ['controller College admission and sole loadout', 'locked loadout Back', 'neutral handoff', 'controller pause', 'spatial focus', 'modal focus ownership', 'touch title/admission/Create', 'phone pause targets'], errors}));
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
