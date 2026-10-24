const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
// Real account subscription and delayed catalog refresh through the mounted Website API.
// The catalog request waits at the transport boundary before reaching the actual backend.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const gpu = require('./gpu-ui.cjs');
const output = path.resolve(process.env.OUT || '/tmp');
const url = process.env.URL;
const website = new URL(process.env.WEBSITE_URL || url);
const mount = website.pathname.replace(/\/$/, '');
const account = JSON.parse(fs.readFileSync(path.join(process.env.PRIVATE_DIR, 'private-accounts.json')))[0];
const errors = [];
const checks = [];
const pending = new Set();
const requests = [];
let subscribedSlug;
let activeMods;
let authenticatedToken;

async function waitUntil(predicate, label, timeout = 30000) {
  const until = Date.now() + timeout;
  while (Date.now() < until) {
    if (predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 30));
  }
  throw new Error(`Timed out waiting for ${label}`);
}

(async () => {
  const owned = await launchOwnedBrowser(chromium,{headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
  const browser = owned.browser;
  const context = await browser.newContext({ignoreHTTPSErrors: process.env.LOCAL_TLS === '1',
    viewport: {width: 1280, height: 720}});
  const page = await context.newPage();
  page.setDefaultTimeout(30000);
  await gpu.watchRequestFailures(page, {errors}).ready;
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => {if (message.type() === 'error') errors.push(message.text());});
  page.on('request', request => {
    if (new URL(request.url()).pathname.endsWith('/api/mods/subscriptions')) pending.add(request);
  });
  page.on('requestfinished', request => pending.delete(request));
  page.on('requestfailed', request => {
    pending.delete(request);
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
    await page.goto(new URL(`${mount}/login`, website).href);
    await page.locator('input[autocomplete="username"]').fill(account.username);
    await page.locator('input[autocomplete="current-password"]').fill(account.password);
    const authentication = page.waitForResponse(response =>
      new URL(response.url()).pathname.endsWith('/api/auth/login')
      && response.request().method() === 'POST' && response.ok());
    await page.getByRole('button', {name: 'Sign in', exact: true}).click();
    authenticatedToken = (await (await authentication).json()).token;
    assert.equal(typeof authenticatedToken, 'string', 'the ordinary sign-in returns its auth token');
    await page.waitForURL(target => target.pathname === `${mount}/account`);
    checks.push('the retained real account signs in through the ordinary Website form');
    await gpu.settleErrors(page);
    await gpu.observeDocumentReplacement(page, () => page.goto(url, {waitUntil: 'load'}));
    await gpu.ready(page);
    if (gpu.findWidget(await gpu.readUi(page), 'prompt-secondary')) {
      await gpu.click(page, 'prompt-secondary');
      await gpu.noWidget(page, 'prompt-secondary');
    }
    await gpu.click(page, 'explore');
    const ready = await gpu.waitForUi(page, snapshot => snapshot.widgets.some(widget =>
      widget.id.startsWith('cloud-subscription:subscribe:') && widget.enabled), 'actual unsubscribed catalog row');
    const subscribe = ready.widgets.find(widget => widget.id.startsWith('cloud-subscription:subscribe:') && widget.enabled);
    subscribedSlug = subscribe.id.split(':').at(-1);
    await gpu.click(page, subscribe.id);
    await waitUntil(() => activeMods?.some(mod => mod.slug === subscribedSlug), 'actual enabled subscription');
    await gpu.click(page, 'cloud-tab:subscribed');
    const enabled = `cloud-subscription:disable:${subscribedSlug}`;
    await gpu.widget(page, enabled);
    await waitUntil(() => pending.size === 0, 'immediate catalog completion');
    await gpu.widget(page, enabled, {enabled: true});
    checks.push('completed subscription and content download update the actual enabled UI');
    await page.screenshot({path: path.join(output, 'cloud-subscription-enabled.png')});
    await gpu.click(page, enabled);
    const enable = `cloud-subscription:enable:${subscribedSlug}`;
    await gpu.widget(page, enable);
    await waitUntil(() => activeMods?.every(mod => mod.slug !== subscribedSlug), 'actual disabled subscription');
    holdRefresh = true;
    await gpu.click(page, enable);
    await waitUntil(() => activeMods?.some(mod => mod.slug === subscribedSlug), 'actual re-enabled subscription');
    await waitUntil(() => pending.size > 0, 'actual delayed catalog refresh in flight');
    checks.push('actual subscription is enabled while its catalog refresh remains in flight');
    await page.keyboard.press('Escape');
    await gpu.surface(page, 'pause');
    await gpu.click(page, 'leave');
    await gpu.surface(page, 'title');
    await gpu.click(page, 'play');
    await gpu.click(page, 'new-game');
    await gpu.surface(page, 'routing');
    const prompt = await gpu.widget(page, 'routing/dialog');
    const actualName = activeMods.find(mod => mod.slug === subscribedSlug).name;
    assert.equal(prompt.label, 'LOCAL PLAY IS ACTIVE');
    assert.ok(prompt.value.includes(actualName));
    await gpu.assertGpuOnly(page);
    assert.ok(pending.size > 0, 'the route sees active content before its catalog refresh completes');
    await page.screenshot({path: path.join(output, 'cloud-subscription-routing.png')});
    checks.push('leaving Cloud during catalog refresh preserves active content in the actual New Game route');
    releaseRefresh();
    await waitUntil(() => pending.size === 0, 'actual catalog refresh completes', 60000);
    await gpu.settleErrors(page);
    assert.deepEqual(errors, []);
    const receipt = {passed: true, checks, requests, errors};
    fs.writeFileSync(path.join(output, 'cloud-subscription.json'), JSON.stringify(receipt, null, 2));
    console.log(JSON.stringify(receipt));
  } catch (error) {
    await gpu.settleErrors(page);
    await page.screenshot({path: path.join(output, 'cloud-subscription-failure.png')});
    fs.writeFileSync(path.join(output, 'cloud-subscription-failure.json'), JSON.stringify({error: error.stack,
      checks, requests, errors, pending: pending.size, ui: await gpu.readUi(page)}, null, 2));
    throw error;
  } finally {
    try {
      await gpu.settleErrors(page);
      holdRefresh = false;
      releaseRefresh();
      if (subscribedSlug) {
        const cleanup = await context.request.delete(new URL(`api/mods/${encodeURIComponent(subscribedSlug)}/subscription`, url).href,
          {headers: {Authorization: `Bearer ${authenticatedToken}`}});
        assert.ok(cleanup.ok(), `local test subscription cleanup failed: ${cleanup.status()}`);
      }
    } finally { console.log(JSON.stringify({browserCleanup:await owned.close()})); }
  }

})().catch(error => {console.error(error); process.exitCode = 1;});
