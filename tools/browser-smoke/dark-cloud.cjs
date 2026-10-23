// Read actual Website content through GPU catalog/detail/modal/input controls.
const assert = require('node:assert/strict');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const gpu = require('./gpu-ui.cjs');

async function revealTab(page, id) {
  for (let attempt = 0; attempt < 8; attempt += 1) {
    if (gpu.findWidget(await gpu.readUi(page), id)) return;
    await gpu.wheel(page, 'cloud-tabs', 240, 0);
    await gpu.nextFrame(page);
  }
  throw new Error(`Cloud tab did not scroll into view: ${id}`);
}
(async () => {
  const browser = await chromium.launch({headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
  const checks = [];
  let page;
  try {
    page = await browser.newPage({viewport: {width: 1600, height: 900}, ignoreHTTPSErrors: process.env.LOCAL_TLS === '1'});
    const {errors} = gpu.watchErrors(page);
    const catalogs = [];
    const details = [];
    page.on('response', response => {
      const pathname = new URL(response.url()).pathname;
      if (pathname.endsWith('/api/mods') && response.ok()) catalogs.push(response.json());
      else if (/\/api\/mods\/[^/]+$/.test(pathname) && response.ok()) details.push(response.json());
    });
    await page.goto(process.env.URL || 'http://127.0.0.1:4181/');
    await gpu.ready(page);
    if (gpu.findWidget(await gpu.readUi(page), 'prompt-secondary')) await gpu.click(page, 'prompt-secondary');
    await gpu.click(page, 'explore');
    await gpu.surface(page, 'dark-cloud');
    await gpu.waitForUi(page, () => catalogs.length > 0, 'actual catalog response');
    const catalog = await catalogs.at(-1);
    const actual = new Map(catalog.items.map(item => [item.slug, item]));
    await gpu.waitForUi(page, s => !gpu.findWidget(s, 'cloud-list-status')?.value.includes('CONSULTING'), 'accepted catalog');
    const visible = (await gpu.readUi(page)).widgets.filter(w => w.id.startsWith('cloud-select:mod:'));
    for (const row of visible) assert.ok(actual.has(row.id.slice('cloud-select:mod:'.length)), 'rendered mod is real catalog content');
    await gpu.assertGpuOnly(page);
    await page.screenshot({path: path.join(process.env.OUT || '/tmp', 'dark-cloud-catalog.png')});
    checks.push('actual catalog and stock GPU scene');
    await gpu.click(page, 'cloud-search');
    await gpu.fill(page, 'dark-cloud-search-input', 'no-source-mod-can-match-this-93c1');
    await gpu.focused(page, 'dark-cloud-search-input');
    await page.keyboard.press('Enter');
    await gpu.noWidget(page, 'dark-cloud-search-input');
    await gpu.waitForUi(page, s => gpu.findWidget(s, 'cloud-list-status')?.value.includes('0 MATCHES'), 'no matching real content');
    await gpu.click(page, 'cloud-clear-query');
    await gpu.click(page, 'cloud-sort');
    await gpu.click(page, 'cloud-sort:name');
    await gpu.focused(page, 'cloud-sort');
    checks.push('search edit/Enter/clear, sort, modal return focus');
    const firstMod = (await gpu.readUi(page)).widgets.find(w => w.id.startsWith('cloud-select:mod:'));
    if (firstMod) {
      const slug = firstMod.id.slice('cloud-select:mod:'.length);
      await gpu.click(page, firstMod.id, {clickCount: 2});
      await gpu.widget(page, 'cloud-detail-title');
      await gpu.waitForUi(page, () => details.length > 0, 'actual detail');
      const detail = await details.at(-1);
      assert.equal((await gpu.widget(page, 'cloud-detail-title')).value, actual.get(slug).name);
      if (detail.screenshots.length > 1) {
        await gpu.click(page, 'cloud-gallery-next');
        await gpu.value(page, 'cloud-gallery-image', `2 / ${detail.screenshots.length}`);
        await page.keyboard.press('ArrowLeft');
        await gpu.value(page, 'cloud-gallery-image', `1 / ${detail.screenshots.length}`);
        checks.push('actual screenshot gallery navigation');
      }
      await page.screenshot({path: path.join(process.env.OUT || '/tmp', 'dark-cloud-detail.png')});
      await page.keyboard.press('Escape');
      await gpu.noWidget(page, 'cloud-detail-title');
      await gpu.focused(page, firstMod.id);
      checks.push('real row double activation, detail receipt, modal Escape and restored row focus');
    }
    await page.keyboard.press('Escape');
    await gpu.surface(page, 'pause');
    await page.waitForTimeout(400);
    await page.keyboard.press('Escape');
    await gpu.surface(page, 'pause');
    await gpu.click(page, 'pause-settings');
    await gpu.surface(page, 'settings');
    await gpu.click(page, 'settings-back');
    await gpu.surface(page, 'dark-cloud');
    await page.setViewportSize({width: 896, height: 414});
    await gpu.nextFrame(page);
    await page.screenshot({path: path.join(process.env.OUT || '/tmp', 'dark-cloud-landscape.png')});
    await page.setViewportSize({width: 390, height: 844});
    await gpu.nextFrame(page);
    await revealTab(page, 'cloud-tab:layouts');
    await gpu.click(page, 'cloud-tab:layouts');
    await gpu.widget(page, 'dark-cloud-layout-code');
    await gpu.fill(page, 'dark-cloud-layout-code', '0I-abcd-efgh');
    await gpu.value(page, 'dark-cloud-layout-code', 'ABCD-EFGH');
    assert.equal((await gpu.widget(page, 'cloud-layout-publish')).enabled, false, 'guest cannot publish a saved layout');
    await page.screenshot({path: path.join(process.env.OUT || '/tmp', 'dark-cloud-portrait.png')});
    await gpu.click(page, 'cloud-menu');
    await gpu.surface(page, 'pause');
    await gpu.click(page, 'leave');
    await gpu.surface(page, 'title');
    await gpu.assertGpuOnly(page);
    await gpu.settleErrors(page);
    assert.deepEqual(errors, []);
    checks.push('source Cloud pause/settings return, compact landscape, portrait tabs/layout edit/admission');
    console.log(JSON.stringify({checks, actualMods: actual.size, errors}));
  } catch (error) {
    if (page) await page.screenshot({path: path.join(process.env.OUT || '/tmp', 'dark-cloud-failure.png')});
    throw error;
  } finally {
    try { if (page && !page.isClosed()) await gpu.settleErrors(page); }
    finally { await browser.close(); }
  }
})().catch(error => {console.error(error); process.exitCode = 1;});
