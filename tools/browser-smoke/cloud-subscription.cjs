// Real account subscription and delayed catalog refresh through the mounted Website API.
// The catalog request waits at the transport boundary before reaching the actual backend.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const output = path.resolve(process.env.OUT || '/tmp');
const url = process.env.URL;
const account = JSON.parse(fs.readFileSync(path.join(process.env.PRIVATE_DIR, 'private-accounts.json')))[0];
const errors = [];
const checks = [];
const pending = new Set();
const requests = [];
let subscribedSlug;
let activeMods;

async function waitUntil(predicate, label, timeout = 30000) {
  const until = Date.now() + timeout;
  while (Date.now() < until) {
    if (predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 30));
  }
  throw new Error(`Timed out waiting for ${label}`);
}

(async () => {
  const browser = await chromium.launch({headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
  const context = await browser.newContext({ignoreHTTPSErrors: process.env.LOCAL_TLS === '1',
    viewport: {width: 1280, height: 720}});
  await context.addInitScript(token => localStorage.setItem('sdr.token', token), account.token);
  const page = await context.newPage();
  page.setDefaultTimeout(30000);
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => {if (message.type() === 'error') errors.push(message.text());});
  page.on('request', request => {
    if (new URL(request.url()).pathname.endsWith('/api/mods/subscriptions')) pending.add(request);
  });
  page.on('requestfinished', request => pending.delete(request));
  page.on('requestfailed', request => {
    pending.delete(request);
    if (!(request.resourceType() === 'media' && request.failure()?.errorText === 'net::ERR_ABORTED'))
      errors.push(`${new URL(request.url()).pathname}: ${request.failure()?.errorText}`);
  });
  page.on('response', async response => {
    const pathname = new URL(response.url()).pathname;
    if (pathname.includes('/api/')) requests.push({path: pathname, status: response.status()});
    if (response.status() >= 400) errors.push(`${response.status()} ${pathname}`);
    if (pathname.endsWith('/api/mods/active') && response.ok()) activeMods = (await response.json()).mods;
  });
  let holdRefresh = false;
  let releaseRefresh;
  const refreshGate = new Promise(resolve => {releaseRefresh = resolve;});
  await context.route('**/api/mods/subscriptions', async route => {
    if (holdRefresh) await refreshGate;
    await route.continue();
  });
  try {
    await page.goto(url, {waitUntil: 'load'});
    await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0
      && document.querySelector('#status')?.textContent === '', null, {timeout: 60000});
    if (await page.locator('.title-prompt:not([hidden])').isVisible())
      await page.locator('[data-menu-action="prompt-secondary"]').click();
    await page.getByRole('button', {name: 'Explore the Dark Cloud', exact: true}).click();
    await page.locator('.dark-cloud-rows[aria-busy="false"]').waitFor();
    const subscribe = page.locator('[data-menu-action^="cloud-subscription:subscribe:"]:visible').first();
    subscribedSlug = (await subscribe.getAttribute('data-menu-action')).split(':').at(-1);
    await subscribe.click();
    await waitUntil(() => activeMods?.some(mod => mod.slug === subscribedSlug), 'actual enabled subscription');
    await page.locator(`[data-mod-slug="${subscribedSlug}"] .dark-cloud-row-state.enabled`).waitFor();
    await page.locator('[data-menu-action="cloud-tab:subscribed"]').click();
    const enabled = page.locator(`[data-menu-action="cloud-subscription:disable:${subscribedSlug}"]:visible`).first();
    await enabled.waitFor();
    await waitUntil(() => pending.size === 0, 'immediate catalog completion');
    assert.equal(await enabled.isEnabled(), true);
    checks.push('immediately completed subscription updates the actual enabled UI');
    await page.screenshot({path: path.join(output, 'cloud-subscription-enabled.png')});
    await enabled.click();
    const enable = page.locator(`[data-menu-action="cloud-subscription:enable:${subscribedSlug}"]:visible`).first();
    await enable.waitFor();
    await waitUntil(() => activeMods?.every(mod => mod.slug !== subscribedSlug), 'actual disabled subscription');
    holdRefresh = true;
    await enable.click();
    await waitUntil(() => activeMods?.some(mod => mod.slug === subscribedSlug), 'actual re-enabled subscription');
    await waitUntil(() => pending.size > 0, 'actual delayed catalog refresh in flight');
    checks.push('actual subscription is enabled while its catalog refresh remains in flight');
    await page.keyboard.press('Escape');
    await page.getByRole('button', {name: 'MAIN MENU', exact: true}).click();
    await page.locator('.game-menus[data-menu-screen="title"]').waitFor();
    await page.locator('[data-menu-action="play"]').click();
    await page.locator('[data-menu-action="new-game"]').click();
    const prompt = page.getByRole('dialog', {name: 'Local play is active', exact: true});
    await prompt.waitFor();
    const actualName = activeMods.find(mod => mod.slug === subscribedSlug).name;
    assert.ok((await prompt.innerText()).includes(actualName));
    assert.ok(pending.size > 0, 'the route sees active content before its catalog refresh completes');
    await page.screenshot({path: path.join(output, 'cloud-subscription-routing.png')});
    checks.push('leaving Cloud during catalog refresh preserves active content in the actual New Game route');
    releaseRefresh();
    await waitUntil(() => pending.size === 0, 'actual catalog refresh completes', 60000);
    assert.deepEqual(errors, []);
    const receipt = {passed: true, checks, requests, errors};
    fs.writeFileSync(path.join(output, 'cloud-subscription.json'), JSON.stringify(receipt, null, 2));
    console.log(JSON.stringify(receipt));
  } catch (error) {
    await page.screenshot({path: path.join(output, 'cloud-subscription-failure.png')});
    fs.writeFileSync(path.join(output, 'cloud-subscription-failure.json'), JSON.stringify({error: error.stack,
      checks, requests, errors, pending: pending.size, ui: await page.evaluate(() => ({status: document.querySelector('#status')?.textContent,
        menu: document.querySelector('.game-menus')?.dataset.menuScreen, text: document.body.innerText.slice(-2500)}))}, null, 2));
    throw error;
  } finally {
    try {
      holdRefresh = false;
      releaseRefresh();
      if (subscribedSlug) {
        const cleanup = await context.request.delete(new URL(`api/mods/${encodeURIComponent(subscribedSlug)}/subscription`, url).href,
          {headers: {Authorization: `Bearer ${account.token}`}});
        assert.ok(cleanup.ok(), `local test subscription cleanup failed: ${cleanup.status()}`);
      }
    } finally { await browser.close(); }
  }

})().catch(error => {console.error(error); process.exitCode = 1;});
