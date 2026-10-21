// Read-only journey against the configured Website backend. No fabricated mod
// rows, subscription changes, comments or party admission are used by this check.
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require(path.resolve(process.env.PW));

async function assertListFrameFits(scene) {
  const boxes = await scene.evaluate(root => {
    const frame = root.querySelector('.dark-cloud-list-frame').getBoundingClientRect();
    const art = root.querySelector('.dark-cloud-native-list-art > .stock-nine-slice:last-child').getBoundingClientRect();
    return {frame: frame.toJSON(), art: art.toJSON()};
  });
  for (const edge of ['left', 'top', 'right', 'bottom']) {
    assert.ok(Math.abs(boxes.frame[edge] - boxes.art[edge]) < 1,
      `Cloud ${edge} artwork follows its own layout box under CSS zoom: ${JSON.stringify(boxes)}`);
  }
}

async function assertSelectedTabVisible(scene) {
  const boxes = await scene.evaluate(root => {
    const nav = root.querySelector('.dark-cloud-tabs');
    return {host:nav.getBoundingClientRect().toJSON(),
      selected:nav.querySelector('[aria-selected="true"]').getBoundingClientRect().toJSON(),
      viewport:window.innerWidth};
  });
  assert.ok(boxes.host.left >= -1 && boxes.host.right <= boxes.viewport + 1,
    `Cloud tab viewport stays onscreen: ${JSON.stringify(boxes)}`);
  assert.ok(boxes.selected.left >= boxes.host.left - 1 && boxes.selected.right <= boxes.host.right + 1,
    `Cloud selected band is visible: ${JSON.stringify(boxes)}`);
}

(async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
  });
  const errors = [];
  const failedRequests = [];
  const cancelledMedia = [];
  let page;
  try {
    page = await browser.newPage({
      viewport: {width: 1600, height: 900},
      ignoreHTTPSErrors: process.env.LOCAL_TLS === '1',
    });
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => {if (message.type() === 'error') errors.push(message.text());});
    page.on('requestfailed', request => {
      const receipt = {path: new URL(request.url()).pathname, failure: request.failure()};
      if (request.resourceType() === 'media' && request.failure()?.errorText === 'net::ERR_ABORTED') cancelledMedia.push(receipt);
      else failedRequests.push(receipt);
    });
    page.on('response', response => {if (response.status() >= 400) failedRequests.push({path: new URL(response.url()).pathname, status: response.status()});});
    const catalogs = [];
    page.on('response', response => {
      if (new URL(response.url()).pathname.endsWith('/api/mods') && response.ok()) catalogs.push(response.json());
    });
    await page.goto(process.env.URL || 'http://127.0.0.1:4181/', {waitUntil: 'load'});
    await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0
      && document.getElementById('status').textContent === '', null, {timeout: 60000});
    if (await page.locator('.title-prompt:not([hidden])').isVisible()) await page.locator('[data-menu-action="prompt-secondary"]').click();
    await page.getByRole('button', {name: 'Explore the Dark Cloud', exact: true}).click();
    const scene = page.locator('.dark-cloud-scene:not([hidden])');
    await scene.waitFor();
    await scene.locator('.dark-cloud-rows[aria-busy="false"]').waitFor({timeout: 30000});
    assert.ok(catalogs.length > 0, 'the real catalog was requested');
    const pages = await Promise.all(catalogs);
    const actual = [...new Set(pages.flatMap(result => result.items.map(item => item.slug)))].sort();
    assert.deepEqual(await scene.locator('[data-mod-slug]').evaluateAll(rows => rows.map(row => row.dataset.modSlug).sort()), actual);
    assert.equal(await scene.locator('[data-native-ui-font="heading"]').count() > 0, true, 'stock heading glyphs are present');
    await assertListFrameFits(scene);
    await assertSelectedTabVisible(scene);
    await page.screenshot({path: path.join(process.env.OUT || '/tmp', 'dark-cloud-catalog.png')});
    await scene.getByRole('button', {name: 'Search', exact: true}).click();
    const search = page.getByRole('dialog', {name: 'SEARCH THE DARK CLOUD', exact: true});
    const input = search.locator('#dark-cloud-search-input');
    await input.fill('no-source-mod-can-match-this-93c1');
    await input.click();
    assert.equal(await input.evaluate(node => document.activeElement === node), true, 'editing does not submit the search form');
    await input.press('Enter');
    await search.waitFor({state: 'hidden'});
    assert.equal(await scene.locator('[data-mod-slug]').count(), 0);
    await scene.locator('[data-menu-action="cloud-clear-query"]:visible').click();
    await scene.getByRole('button', {name: 'Sort', exact: true}).click();
    await page.getByRole('dialog', {name: 'SORT MODS BY...', exact: true}).getByRole('button', {name: 'NAME', exact: true}).click();
    const detailChecks = [];
    if (actual.length > 0) {
      const first = scene.locator('[data-mod-slug]').first();
      const slug = await first.getAttribute('data-mod-slug');
      const detailResponse = page.waitForResponse(response => new URL(response.url()).pathname.endsWith(`/api/mods/${slug}`) && response.ok());
      await first.locator('.dark-cloud-row-main').dblclick();
      const detail = await (await detailResponse).json();
      const dialog = page.locator('dialog.dark-cloud-detail[open]');
      await dialog.waitFor();
      await page.waitForFunction(() => !document.querySelector('.dark-cloud-versions .dark-cloud-section-heading')?.textContent.includes('LOADING'));
      assert.equal(await dialog.locator('.dark-cloud-gallery-thumbnails button').count(), detail.screenshots.length > 1 ? detail.screenshots.length : 0);
      assert.equal(await dialog.locator('.dark-cloud-detail-subscription').evaluate(node => Boolean(node.closest('aside.dark-cloud-detail-summary'))), true);
      if (detail.screenshots.length > 1) {
        await dialog.getByRole('button', {name: 'NEXT IMAGE', exact: true}).click();
        await page.waitForFunction(() => document.querySelector('.dark-cloud-gallery .dark-cloud-section-heading')?.textContent.includes('2 /'));
        await page.keyboard.press('ArrowLeft');
        await page.waitForFunction(() => document.querySelector('.dark-cloud-gallery .dark-cloud-section-heading')?.textContent.includes('1 /'));
        detailChecks.push('actual gallery navigation');
      }
      await page.screenshot({path: path.join(process.env.OUT || '/tmp', 'dark-cloud-detail.png')});
      await page.keyboard.press('Escape');
      await dialog.waitFor({state: 'hidden'});
      detailChecks.push('real catalog double activation', 'actual detail/version/comments receipts', 'native modal Escape');
    }
    await page.keyboard.press('Escape');
    await page.locator('.game-menus[data-menu-screen="pause"]').waitFor();
    await page.waitForFunction(() => document.querySelector('.pause-stage')?.style.opacity === '1');
    await page.keyboard.press('Escape');
    assert.equal(await page.locator('.game-menus').getAttribute('data-menu-screen'), 'pause', 'source Cloud menu has no keyboard Escape action');
    await page.getByRole('button', {name: 'GAME SETTINGS', exact: true}).click();
    await page.locator('.game-settings-backdrop:not([hidden])').waitFor();
    await page.locator('[data-menu-action="settings-back"]').click();
    await page.locator('.game-menus[data-menu-screen="dark-cloud"]').waitFor();
    await page.setViewportSize({width: 896, height: 414});
    await scene.locator('.dark-cloud-heading > button').waitFor({state: 'hidden'});
    // Allow measured artwork to repaint after the compact-height rule hides the
    // account control; a zero-width hidden control must not enter glyph layout.
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    assert.deepEqual(errors, [], 'compact landscape keeps the native text loop alive');
    assert.ok(await scene.locator('h1 .native-ui-glyph').count() > 0);
    await assertListFrameFits(scene);
    await assertSelectedTabVisible(scene);
    await page.screenshot({path: path.join(process.env.OUT || '/tmp', 'dark-cloud-landscape.png')});
    await page.setViewportSize({width: 390, height: 844});
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    await assertListFrameFits(scene);
    await assertSelectedTabVisible(scene);
    await page.screenshot({path: path.join(process.env.OUT || '/tmp', 'dark-cloud-portrait.png')});
    await page.keyboard.press('Escape');
    await page.getByRole('button', {name: 'MAIN MENU', exact: true}).click();
    await page.locator('.game-menus[data-menu-screen="title"]').waitFor();
    assert.deepEqual(errors, []);
    assert.deepEqual(failedRequests, []);
    console.log(JSON.stringify({checks: ['actual catalog rows', 'stock typography', 'search input/submit/clear', 'sort sheet', ...detailChecks, 'source Cloud pause/settings/return', 'desktop, landscape and portrait frame sizing'], detailAvailable: actual.length > 0, errors, cancelledMedia}));
  } catch (error) {
    if (page) {
      await page.screenshot({path: path.join(process.env.OUT || '/tmp', 'dark-cloud-failure.png')});
      console.error(JSON.stringify({errors, failedRequests, cancelledMedia, status: await page.locator('#status').textContent()}));
    }
    throw error;
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
