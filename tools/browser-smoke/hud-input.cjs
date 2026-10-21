// Stock HUD bounds and real browser input paths. Uses the same Chrome/PW/URL options as
// smoke.cjs; OUT names an existing directory for the desktop, wide and touch screenshots.
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require(path.resolve(process.env.PW));
const url = process.env.URL || 'http://127.0.0.1:4185/';
const output = process.env.OUT || '/tmp';
const { startNewGame } = require('./menu-journey.cjs');

async function ready(page) {
  await page.goto(url, { waitUntil: 'load' });
  await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0
    && document.getElementById('status').textContent === '');
  await startNewGame(page);
  await page.waitForSelector('.hud-health [class="stock-strip-piece"]');
}

const state = (page) => page.evaluate(() => Array.from(window.wasmBindings.wizard_state()));
async function ticks(page, count) {
  const start = (await state(page))[5];
  await page.waitForFunction((target) => window.wasmBindings.wizard_state()[5] >= target,
    start + count, { timeout: 20000 });
}

async function screenshot(page, name) {
  await page.screenshot({ path: path.join(output, name) });
}

(async () => {
  const browser = await chromium.launch({
    headless: true,
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
    const desktop = await browser.newPage({ viewport: { width: 1600, height: 900 } });
    watch(desktop);
    await ready(desktop);
    const bounds = await desktop.locator('.hud-health, .hud-mana, .hud-backpack, .hud-tome').evaluateAll(
      (elements) => elements.map((element) => {
        const box = element.getBoundingClientRect();
        return [box.x, box.y, box.width, box.height];
      }),
    );
    assert.deepEqual(bounds, [[640, 14.5, 110, 20], [850, 14.5, 110, 20], [730.5, 825, 58, 62], [810.5, 825, 58, 62]]);
    await screenshot(desktop, 'hud-desktop.png');
    await desktop.setViewportSize({ width: 1920, height: 1080 });
    await ticks(desktop, 3);
    assert.equal((await desktop.locator('.hud-backpack').boundingBox()).y, 1005);
    await screenshot(desktop, 'hud-wide.png');

    // Native Gamepad getters are read through web-sys. Prototype-backed samples exercise
    // that browser edge without requiring a physical controller on the build machine.
    await desktop.evaluate(() => {
      window.hudTestPad = { x: 0, y: 1, right: false, connected: true };
      const pad = Object.create(Gamepad.prototype, {
        index: {value: 0},
        connected: { get: () => window.hudTestPad.connected },
        mapping: { value: 'standard' },
        axes: { get: () => [window.hudTestPad.x, window.hudTestPad.y] },
        buttons: { get: () => Array.from({ length: 16 }, (_, index) => Object.create(GamepadButton.prototype, {
          pressed: { value: index === 15 && window.hudTestPad.right },
          value: { value: index === 15 && window.hudTestPad.right ? 1 : 0 },
        })) },
      });
      Object.defineProperty(navigator, 'getGamepads', { value: () => [pad] });
    });
    const beforePad = await state(desktop);
    await ticks(desktop, 70);
    const afterPad = await state(desktop);
    assert.ok(afterPad[1] > beforePad[1] + 25, 'the left stick walks south');
    await desktop.evaluate(() => { window.hudTestPad = { x: -1, y: 0, right: true, connected: true }; });
    await ticks(desktop, 70);
    const afterDpad = await state(desktop);
    assert.ok(afterDpad[0] > afterPad[0] + 25, 'd-pad right wins over stick left');
    await desktop.evaluate(() => { window.hudTestPad = { x: 0.1, y: 0, right: false, connected: true }; });
    await ticks(desktop, 40);
    const driftStart = await state(desktop);
    await ticks(desktop, 40);
    const driftEnd = await state(desktop);
    assert.ok(Math.hypot(driftEnd[0] - driftStart[0], driftEnd[1] - driftStart[1]) < 0.1, 'stick drift stops');
    await desktop.evaluate(() => { window.hudTestPad.connected = false; });
    await desktop.keyboard.down('KeyD');
    await ticks(desktop, 30);
    await desktop.evaluate(() => window.dispatchEvent(new Event('blur')));
    await ticks(desktop, 40);
    const blurStart = await state(desktop);
    await ticks(desktop, 40);
    const blurEnd = await state(desktop);
    assert.ok(Math.hypot(blurEnd[0] - blurStart[0], blurEnd[1] - blurStart[1]) < 0.1, 'blur releases keys');
    await desktop.keyboard.up('KeyD');
    await desktop.close();

    const touch = await browser.newPage({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });
    watch(touch);
    await ready(touch);
    await screenshot(touch, 'hud-touch-portrait.png');
    await touch.setViewportSize({ width: 844, height: 390 });
    await ticks(touch, 3);
    await screenshot(touch, 'hud-touch-landscape.png');
    const joystick = await touch.locator('[data-joystick="movement"]').boundingBox();
    assert.ok(joystick.width >= 90, 'the touch joystick remains usable');
    const client = await touch.context().newCDPSession(touch);
    const point = { x: joystick.x + joystick.width * 0.8, y: joystick.y + joystick.height * 0.7 };
    const beforeTouch = await state(touch);
    await client.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [point] });
    await ticks(touch, 70);
    const afterTouch = await state(touch);
    assert.ok(afterTouch[0] > beforeTouch[0] + 20, 'touch walks east');
    await client.send('Input.dispatchTouchEvent', { type: 'touchCancel', touchPoints: [] });
    await ticks(touch, 40);
    const cancelStart = await state(touch);
    await ticks(touch, 40);
    const cancelEnd = await state(touch);
    assert.ok(Math.hypot(cancelEnd[0] - cancelStart[0], cancelEnd[1] - cancelStart[1]) < 0.1, 'pointer cancellation stops movement');
    assert.equal(await touch.locator('[data-joystick="movement"]').getAttribute('data-active'), null);
    assert.deepEqual(errors, []);
    console.log(JSON.stringify({ checks: ['native HUD bounds', 'wide layout', 'gamepad stick', 'd-pad priority', 'stick drift', 'blur release', 'touch movement', 'pointer cancellation'], errors }));
  } finally {
    await browser.close();
  }
})().catch((error) => { console.error(error); process.exitCode = 1; });
