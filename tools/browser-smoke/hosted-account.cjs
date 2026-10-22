// Real mounted Website forms and Rust account controls. No credential bootstrap.
// Source: pages/{Login,Register}.tsx, lib/auth.tsx and MainMenuScene.tsx.
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));

const url = new URL(process.env.URL || 'https://127.0.0.1:9449/game-stg/');
const mount = url.pathname.replace(/\/$/, '');
const output = path.resolve(process.env.OUT);
// Only initial asset residency uses this override; later behavior waits are unchanged.
const coldStartTimeout = Number(process.env.COLD_START_TIMEOUT_MS || 30000);
assert.ok(Number.isSafeInteger(coldStartTimeout) && coldStartTimeout > 0,
  'COLD_START_TIMEOUT_MS must be a positive integer');
const checks = [];
const state = {requests: [], errors: [], cancelledRequests: [], outsideMount: [],
  policyViolations: [], documents: [], auth: [], identities: [], wasm: new Set()};
let step = 'fresh account preparation';
let account;
let server;
let browser;
let page;

function redact(value) {
  let text = String(value);
  if (account) {
    text = text.replaceAll(account.password, '[private password]');
    text = text.replaceAll(account.email, '[private test email]');
  }
  return text.replace(/eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+/g, '[private token]');
}

function checked(label) {
  checks.push(label);
  console.log(label);
}

function freshAccount() {
  const suffix = `${Date.now().toString(36)}${crypto.randomBytes(2).toString('hex')}`;
  const username = `Forms${suffix}`;
  account = {username, email: `${username.toLowerCase()}@example.invalid`,
    password: `Qa!${crypto.randomBytes(18).toString('base64url')}`};
  const receipt = path.join(output, 'private-account.json');
  assert.equal(fs.existsSync(receipt), false, 'use a fresh output for a fresh form registration');
  fs.writeFileSync(receipt, JSON.stringify(account, null, 2), {mode: 0o600, flag: 'wx'});
  fs.chmodSync(receipt, 0o600);
}

function observe() {
  page.on('pageerror', error => state.errors.push(redact(error.message)));
  page.on('console', message => {
    if (message.type() === 'error') state.errors.push(redact(message.text()));
  });
  page.on('request', request => {
    const target = new URL(request.url());
    if (target.protocol === 'http:' || target.protocol === 'https:') {
      if (target.origin !== url.origin || !target.pathname.startsWith(`${mount}/`)) {
        state.outsideMount.push({origin: target.origin, path: target.pathname,
          resource: request.resourceType()});
      }
    }
  });
  page.on('response', async response => {
    const pathname = new URL(response.url()).pathname;
    const request = response.request();
    if (pathname.endsWith('.wasm')) state.wasm.add(path.basename(pathname));
    if (request.resourceType() === 'document') {
      state.documents.push({path: pathname, status: response.status(),
        csp: response.headers()['content-security-policy'] || null});
    }
    if (pathname.includes('/api/')) {
      state.requests.push({path: pathname, status: response.status(), method: request.method()});
    }
    if (response.status() >= 400) state.errors.push(`${response.status()} ${pathname}`);
    if (pathname === `${mount}/api/auth/me` && response.status() === 200) {
      try {
        const body = await response.json();
        state.auth.push({status: 200, usernameMatches: body.user?.username === account.username});
      } catch (error) {
        state.errors.push(`auth/me response: ${redact(error.message)}`);
      }
    }
  });
  page.on('requestfailed', request => {
    const error = request.failure()?.errorText;
    const receipt = {path: new URL(request.url()).pathname, resource: request.resourceType(), error};
    // Normal document changes cancel title media and in-flight static resources.
    // Keep those receipts visible without treating cancellation as an API failure.
    if (error === 'net::ERR_ABORTED' && !receipt.path.includes('/api/')) {
      state.cancelledRequests.push(receipt);
    } else state.errors.push(redact(`${error} ${receipt.path}`));
  });
}

async function storage() {
  return page.evaluate(scopedKey => ({
    unscopedAbsent: localStorage.getItem('sdr.token') === null,
    scopedPresent: Boolean(localStorage.getItem(scopedKey)),
  }), `sdr.token:${mount}`);
}

async function gameTitle(username) {
  await page.waitForURL(current => current.origin === url.origin
    && current.pathname === `${mount}/`);
  await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0
    && document.querySelector('#status')?.textContent === '', null, {timeout: coldStartTimeout});
  const prompt = page.locator('.title-prompt:not([hidden])');
  if (await prompt.isVisible()) {
    await prompt.locator('[data-menu-action="prompt-secondary"]').click();
  }
  await page.locator('.game-menus[data-menu-screen="title"]').waitFor();
  const identity = page.locator('.game-account-name-title');
  await identity.waitFor();
  await page.waitForFunction(expected => document.querySelector('.game-account-name-title')
    ?.getAttribute('data-account-username') === expected, username || 'Not logged in');
  state.identities.push({authenticated: Boolean(username),
    matchesExpected: await identity.getAttribute('data-account-username') === (username || 'Not logged in')});
}

// Current Source forms return to Account; its visible Play link opens the game.
async function returnFromAccount() {
  await page.waitForURL(current => current.origin === url.origin
    && current.pathname === `${mount}/account`);
  await page.evaluate(() => document.fonts.ready.then(() => null));
  await page.getByRole('navigation').getByRole('link', {name: 'Play', exact: true}).click();
}

async function openCloud() {
  await page.locator('[data-menu-action="explore"]').click();
  await page.locator('.game-menus[data-menu-screen="dark-cloud"]').waitFor();
}

async function openLogin() {
  await openCloud();
  await page.locator('[data-menu-action="cloud-sign-in"]').click();
  await page.waitForURL(current => current.pathname === `${mount}/login`);
  await page.getByRole('heading', {name: 'Sign In', exact: true}).waitFor();
  await settledAccountForm();
  assert.deepEqual(await storage(), {unscopedAbsent: true, scopedPresent: false});
}

async function settledAccountForm() {
  // Source Reveal fades the form for 550 ms after IntersectionObserver admission.
  // Visible DOM alone can capture its first almost-transparent animation frame.
  await page.waitForFunction(() => {
    const form = document.querySelector('form');
    const reveal = form?.closest('.reveal-in');
    return reveal && getComputedStyle(reveal).opacity === '1';
  });
}

async function bounded(promise, label, milliseconds) {
  let timeout;
  try {
    return await Promise.race([promise, new Promise((resolve, reject) => {
      timeout = setTimeout(() => reject(new Error(`${label} timed out`)), milliseconds);
    })]);
  } finally {
    clearTimeout(timeout);
  }
}

async function closeBrowser() {
  if (!server) return {started: false};
  const ownedProcess = server.process();
  let mode = 'graceful';
  try {
    await bounded(server.close(), 'owned Chrome close', 15000);
  } catch {
    mode = 'forced';
    await bounded(server.kill(), 'owned Chrome termination', 5000);
  }
  return {mode, exited: ownedProcess.exitCode !== null || ownedProcess.signalCode !== null};
}

(async () => {
  fs.mkdirSync(output, {recursive: true});
  let failure;
  try {
    freshAccount();
    server = await chromium.launchServer({headless: true, host: '127.0.0.1',
      executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
    browser = await chromium.connect(server.wsEndpoint());
    const context = await browser.newContext({ignoreHTTPSErrors: process.env.LOCAL_TLS === '1',
      viewport: {width: 1280, height: 720}});
    page = await context.newPage();
    page.setDefaultTimeout(30000);
    page.setDefaultNavigationTimeout(30000);
    await page.exposeBinding('recordAccountPolicyViolation', (source, violation) => {
      state.policyViolations.push(violation);
    });
    await context.addInitScript(() => {
      document.addEventListener('securitypolicyviolation', event => {
        window.recordAccountPolicyViolation({directive: event.effectiveDirective,
          disposition: event.disposition, blockedPath: event.blockedURI.startsWith('http')
            ? new URL(event.blockedURI).pathname : event.blockedURI});
      });
    });
    observe();
    step = 'guest Rust title';
    await page.goto(url.href, {waitUntil: 'load'});
    await gameTitle(null);
    assert.deepEqual(await storage(), {unscopedAbsent: true, scopedPresent: false});
    state.renderer = await page.evaluate(() => {
      const gl = document.querySelector('canvas').getContext('webgl2');
      const info = gl.getExtension('WEBGL_debug_renderer_info');
      return info ? gl.getParameter(info.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER);
    });
    await page.screenshot({path: path.join(output, 'guest-title.png')});
    checked('fresh Rust title is a guest with both account credential namespaces absent');

    step = 'mounted Source login and registration';
    await openLogin();
    await page.screenshot({path: path.join(output, 'mounted-login.png'), fullPage: true});
    await page.getByRole('link', {name: 'Create an account', exact: true}).click();
    await page.waitForURL(current => current.pathname === `${mount}/register`);
    await page.getByRole('heading', {name: 'Create an Account', exact: true}).waitFor();
    await settledAccountForm();
    await page.screenshot({path: path.join(output, 'mounted-registration.png'), fullPage: true});
    await page.locator('input[autocomplete="username"]').fill(account.username);
    await page.locator('input[autocomplete="email"]').fill(account.email);
    await page.locator('input[autocomplete="new-password"]').fill(account.password);
    const registered = page.waitForResponse(response => new URL(response.url()).pathname
      === `${mount}/api/auth/register` && response.request().method() === 'POST');
    await page.getByRole('button', {name: 'Create account', exact: true}).click();
    assert.equal((await registered).status(), 201, 'real Source registration form response');
    await returnFromAccount();
    await gameTitle(account.username);
    assert.deepEqual(await storage(), {unscopedAbsent: true, scopedPresent: true});
    assert.ok(state.auth.some(receipt => receipt.status === 200 && receipt.usernameMatches));
    await page.screenshot({path: path.join(output, 'registered-title.png')});
    checked('unchanged mounted registration form creates a real account and returns the authenticated Rust title');

    step = 'native Cloud sign out';
    await openCloud();
    const accountMenu = page.getByRole('button', {
      name: `You are signed in as ${account.username}.`, exact: true,
    });
    assert.equal(await accountMenu.getAttribute('aria-label'), `You are signed in as ${account.username}.`);
    await page.screenshot({path: path.join(output, 'registered-cloud.png')});
    await accountMenu.click();
    const signedOutDocument = page.waitForEvent('framenavigated', frame => frame === page.mainFrame());
    await page.locator('[data-menu-action="cloud-sign-out"]').click();
    await signedOutDocument;
    await gameTitle(null);
    assert.deepEqual(await storage(), {unscopedAbsent: true, scopedPresent: false});
    await page.screenshot({path: path.join(output, 'signed-out-title.png')});
    checked('actual native Cloud Sign Out returns a guest and removes only the scoped credential');

    step = 'mounted Source re-login';
    await openLogin();
    await page.locator('input[autocomplete="username"]').fill(account.username);
    await page.locator('input[autocomplete="current-password"]').fill(account.password);
    const previousAuth = state.auth.length;
    const signedIn = page.waitForResponse(response => new URL(response.url()).pathname
      === `${mount}/api/auth/login` && response.request().method() === 'POST');
    await page.getByRole('button', {name: 'Sign in', exact: true}).click();
    assert.equal((await signedIn).status(), 200, 'real Source login form response');
    await returnFromAccount();
    await gameTitle(account.username);
    assert.deepEqual(await storage(), {unscopedAbsent: true, scopedPresent: true});
    assert.ok(state.auth.slice(previousAuth).some(receipt => receipt.status === 200 && receipt.usernameMatches));
    await page.screenshot({path: path.join(output, 'signed-in-title.png')});
    checked('unchanged mounted login form authenticates the same account and restores the Rust identity');

    step = 'mount and CSP isolation';
    assert.ok(state.documents.some(document => document.path === `${mount}/login` && document.status === 200));
    assert.ok(state.documents.every(document => document.csp?.includes("script-src 'self'")),
      'all actual documents retain the deployment CSP');
    assert.deepEqual(state.outsideMount, [], 'all application requests stay inside the staging mount');
    assert.deepEqual(state.policyViolations, [], 'actual pages have no CSP violations');
    assert.deepEqual(state.errors, [], 'actual forms and native client have no API/runtime failures');
    checked('all actual requests remain mounted, unscoped storage stays untouched, and CSP/API/runtime errors are empty');
  } catch (error) {
    failure = {step, error: redact(error.stack || error.message)};
    if (page) await page.screenshot({path: path.join(output, 'failure.png')}).catch(() => {});
  } finally {
    let cleanup;
    try {
      cleanup = await closeBrowser();
    } catch (error) {
      cleanup = {error: redact(error.message)};
    }
    const receipt = {url: url.href, sourceRevision: process.env.SOURCE_REV || null,
      viewport: {width: 1280, height: 720}, checks, failure, ...state,
      wasm: [...state.wasm], cleanup};
    fs.writeFileSync(path.join(output, 'receipt.json'), JSON.stringify(receipt, null, 2));
    console.log(JSON.stringify({checks: checks.length, failure, cleanup, output}));
    if (failure || cleanup.exited === false || cleanup.error) process.exitCode = 1;
  }
})();
