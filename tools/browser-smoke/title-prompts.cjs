// The real retained wizard and its native kill-confirmation modal.
const assert = require('node:assert/strict');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const gpu = require('./gpu-ui.cjs');
const {startNewGame} = require('./menu-journey.cjs');
(async () => {
  const browser = await chromium.launch({headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
  let page;
  try {
    page = await browser.newPage({viewport: {width: 1600, height: 900}, ignoreHTTPSErrors: process.env.LOCAL_TLS === '1'});
    const {errors} = gpu.watchErrors(page);
    await page.goto(process.env.URL || 'http://127.0.0.1:4181/');
    await gpu.ready(page);
    await startNewGame(page, {name: 'Felix'});
    await page.keyboard.press('Escape');
    await gpu.surface(page, 'pause');
    await gpu.click(page, 'leave');
    await gpu.surface(page, 'title');
    await gpu.click(page, 'play');
    await gpu.click(page, 'new-game');
    await gpu.widget(page, 'prompt-primary');
    const before = await page.evaluate(() => Array.from(window.wasmBindings.wizard_state()));
    await page.keyboard.press('Escape');
    await gpu.noWidget(page, 'prompt-primary');
    await gpu.surface(page, 'play');
    const after = await page.evaluate(() => Array.from(window.wasmBindings.wizard_state()));
    assert.deepEqual(after, before, 'declining keeps the paused wizard intact');
    await gpu.click(page, 'new-game');
    await page.setViewportSize({width: 390, height: 844});
    await gpu.nextFrame(page);
    const yes = gpu.bounds(await gpu.widget(page, 'prompt-primary'));
    assert.ok(yes.height >= 44 && yes.x >= 0 && yes.x + yes.width <= 390);
    await page.screenshot({path: path.join(process.env.OUT || '/tmp', 'phone-kill-character-rust.png')});
    await gpu.click(page, 'prompt-primary');
    await gpu.noWidget(page, 'prompt-primary');
    await gpu.assertGpuOnly(page);
    await gpu.settleErrors(page);
    assert.deepEqual(errors, []);
    console.log(JSON.stringify({checks: ['real saved wizard', 'decline retains wizard', 'native phone prompt', 'explicit replacement intent'], errors}));
  } finally {
    try { if (page && !page.isClosed()) await gpu.settleErrors(page); }
    finally { await browser.close(); }
  }
})().catch(error => {console.error(error); process.exitCode = 1;});
