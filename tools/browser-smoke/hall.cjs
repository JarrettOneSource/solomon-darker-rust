// Read actual archive/global rows through the displayed GPU Hall widgets.
const assert = require('node:assert/strict');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const gpu = require('./gpu-ui.cjs');
(async () => {
  const browser = await chromium.launch({headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
  const checks = [];
  let page;
  try {
    page = await browser.newPage({viewport: {width: 1600, height: 900}, ignoreHTTPSErrors: process.env.LOCAL_TLS === '1'});
    const {errors, ready: errorsReady} = gpu.watchErrors(page);
    await errorsReady;
    await page.goto(process.env.URL || 'http://127.0.0.1:4181/');
    await gpu.ready(page);
    if (gpu.findWidget(await gpu.readUi(page), 'prompt-secondary')) await gpu.click(page, 'prompt-secondary');
    await gpu.click(page, 'hall');
    await gpu.surface(page, 'hall');
    await gpu.widget(page, 'hall-records');
    await gpu.value(page, 'hall-status', 'No records yet.');
    checks.push('ordinary empty local archive');
    await page.screenshot({path: path.join(process.env.OUT || '/tmp', 'hall-empty-rust.png')});
    if (gpu.findWidget(await gpu.readUi(page), 'hall-scope:global')) {
      await gpu.click(page, 'hall-scope:global');
      for (const board of ['awesomeness', 'wave', 'kills', 'time']) {
        const id = `hall-board:${board}`;
        if (gpu.findWidget(await gpu.readUi(page), id)) await gpu.click(page, id);
      }
      await gpu.click(page, 'hall-scope:local');
      checks.push('actual hosted scope and available global boards');
    }
    await page.keyboard.press('Escape');
    await gpu.surface(page, 'title');
    await page.setViewportSize({width: 390, height: 844});
    await gpu.click(page, 'hall');
    await gpu.surface(page, 'hall');
    const back = gpu.bounds(await gpu.widget(page, 'hall-back'));
    assert.ok(back.height >= 44 && back.x >= 0 && back.x + back.width <= 390);
    await gpu.wheel(page, 'hall-records', 200, 300);
    await page.screenshot({path: path.join(process.env.OUT || '/tmp', 'phone-hall-empty-rust.png')});
    await gpu.click(page, 'hall-back');
    await gpu.surface(page, 'title');
    await gpu.assertGpuOnly(page);
    await gpu.settleErrors(page);
    assert.deepEqual(errors, []);
    checks.push('native Hall canvas scroll and compact touch target');
    console.log(JSON.stringify({checks, errors}));
  } finally {
    try { if (page && !page.isClosed()) await gpu.settleErrors(page); }
    finally { await browser.close(); }
  }
})().catch(error => {console.error(error); process.exitCode = 1;});
