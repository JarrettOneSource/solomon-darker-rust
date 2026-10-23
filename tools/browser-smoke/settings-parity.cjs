const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
// Real GPU Settings pages, stored preferences and native keyboard/mouse capture.
const assert = require('node:assert/strict');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const gpu = require('./gpu-ui.cjs');

async function reveal(page, id) {
  for (let index = 0; index < 14; index += 1) {
    if (gpu.findWidget(await gpu.readUi(page), id)) return;
    await gpu.wheel(page, 'settings-content', 0, 330);
    await gpu.nextFrame(page);
  }
  throw new Error(`Settings control did not scroll into view: ${id}`);
}
(async () => {
  const owned = await launchOwnedBrowser(chromium,{headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
  const browser = owned.browser;
  const checks = [];
  let page;
  try {
    page = await browser.newPage({viewport: {width: 1600, height: 900}, ignoreHTTPSErrors: process.env.LOCAL_TLS === '1'});
    const {errors, ready: errorsReady} = gpu.watchErrors(page);
    await errorsReady;
    await page.goto(process.env.URL || 'http://127.0.0.1:4181/');
    await gpu.ready(page);
    if (gpu.findWidget(await gpu.readUi(page), 'prompt-secondary')) await gpu.click(page, 'prompt-secondary');
    await gpu.click(page, 'settings');
    await gpu.focused(page, 'settings-range:sound');
    assert.deepEqual(gpu.bounds(await gpu.widget(page, 'settings-panel')), {x: 500, y: 100, width: 600, height: 700});
    await gpu.setSlider(page, 'settings-range:sound', 37);
    await gpu.value(page, 'settings-range:sound', '37');
    await gpu.setSlider(page, 'settings-range:music', 61);
    await gpu.value(page, 'settings-range:music', '61');
    checks.push('native centered panel and actual keyboard slider');
    await page.screenshot({path: path.join(process.env.OUT || '/tmp', 'settings-root.png')});
    await gpu.click(page, 'settings-page:cloud');
    await gpu.value(page, 'settings-toggle:online', 'true');
    await gpu.click(page, 'settings-toggle:global-chat');
    await gpu.waitForUi(page, s => gpu.findWidget(s, 'settings-toggle:activity')?.enabled === false, 'activity parent admission');
    await gpu.click(page, 'settings-toggle:online');
    await gpu.waitForUi(page, s => ['activity', 'global-chat', 'shared-hub', 'submit-runs']
      .every(id => gpu.findWidget(s, `settings-toggle:${id}`)?.enabled === false), 'offline child admission');
    await gpu.click(page, 'settings-toggle:online');
    await gpu.value(page, 'settings-toggle:shared-hub', 'true');
    await gpu.value(page, 'settings-toggle:global-chat', 'false');
    await gpu.click(page, 'settings-toggle:global-chat');
    await gpu.value(page, 'settings-toggle:activity', 'true');
    checks.push('dependent online choices retain stored values');
    await page.screenshot({path: path.join(process.env.OUT || '/tmp', 'settings-cloud.png')});
    await page.keyboard.press('Escape');
    await gpu.widget(page, 'settings-range:sound');
    await gpu.click(page, 'settings-page:controls');
    await gpu.click(page, 'bind:move-up');
    await page.keyboard.press('Escape');
    await gpu.value(page, 'bind:move-up', 'Escape');
    await gpu.value(page, 'bind:menu', 'KeyW');
    await gpu.click(page, 'bind:move-up');
    await gpu.click(page, 'settings-back');
    await gpu.waitForUi(page, s => gpu.findWidget(s, 'bind:move-up')?.selected === false, 'Back cancels capture first');
    await gpu.click(page, 'bind:cheats');
    await page.keyboard.press('F2');
    await gpu.value(page, 'bind:cheats', 'F2');
    await reveal(page, 'bind:belt-0');
    await gpu.click(page, 'bind:belt-0');
    const belt = gpu.center(gpu.bounds(await gpu.widget(page, 'bind:belt-0')));
    await page.mouse.click(belt.x, belt.y, {button: 'right'});
    await gpu.value(page, 'bind:belt-0', 'Mouse2');
    checks.push('Escape rebind swaps duplicates, Back cancels capture, F2 and mouse belt capture');
    await page.screenshot({path: path.join(process.env.OUT || '/tmp', 'settings-controls.png')});
    await gpu.click(page, 'settings-back');
    await gpu.click(page, 'settings-page:performance');
    const before = (await gpu.widget(page, 'settings-toggle:lighting')).value;
    await gpu.click(page, 'settings-toggle:lighting');
    await gpu.value(page, 'settings-toggle:lighting', before === 'true' ? 'false' : 'true');
    await gpu.focused(page, 'settings-toggle:lighting');
    await gpu.setSlider(page, 'settings-range:light-quality', 47);
    await gpu.value(page, 'settings-range:light-quality', '47');
    await reveal(page, 'settings-toggle:enhanced-effects');
    await gpu.widget(page, 'settings-toggle:enhanced-effects');
    await page.screenshot({path: path.join(process.env.OUT || '/tmp', 'settings-performance.png')});
    checks.push('performance, quality and actual enhanced-effects authority');
    await gpu.click(page, 'settings-back');
    await gpu.click(page, 'settings-back');
    await gpu.surface(page, 'title');
    await gpu.click(page, 'settings');
    await gpu.value(page, 'settings-range:sound', '37');
    await gpu.click(page, 'settings-back');
    await gpu.settleErrors(page);
    await gpu.observeDocumentReplacement(page, () => page.reload());
    await gpu.ready(page);
    if (gpu.findWidget(await gpu.readUi(page), 'prompt-secondary')) await gpu.click(page, 'prompt-secondary');
    await gpu.click(page, 'settings');
    await gpu.value(page, 'settings-range:sound', '37');
    await page.setViewportSize({width: 390, height: 844});
    await gpu.nextFrame(page);
    const panel = gpu.bounds(await gpu.widget(page, 'settings-panel'));
    assert.ok(panel.x >= 0 && panel.x + panel.width <= 390);
    await page.screenshot({path: path.join(process.env.OUT || '/tmp', 'phone-settings.png')});
    await gpu.assertGpuOnly(page);
    await gpu.settleErrors(page);
    assert.deepEqual(errors, []);
    checks.push('accepted preferences survive actual reload; compact GPU panel');
    console.log(JSON.stringify({checks, errors}));
  } finally {
    try { if (page && !page.isClosed()) await gpu.settleErrors(page); }
    finally { console.log(JSON.stringify({browserCleanup:await owned.close()})); }
  }
})().catch(error => {console.error(error); process.exitCode = 1;});
