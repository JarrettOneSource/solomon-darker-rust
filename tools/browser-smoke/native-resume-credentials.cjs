// Two tabs share ordinary browser storage. All admission, rotation and recovery
// comes from the UI; WebSocket frames and storage are observed without changing them.
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const {spawn} = require('node:child_process');
const {chromium} = require(path.resolve(process.env.PW));
const helperPath = process.env.MENU_HELPER
  ? path.resolve(process.env.MENU_HELPER)
  : path.join(__dirname, 'menu-journey.cjs');
const {startNewGame} = require(helperPath);

const output = path.resolve(process.env.OUT || os.tmpdir());
const binary = path.resolve(process.env.BINARY);
const assets = path.resolve(process.env.ASSETS);
const port = Number(process.env.PORT || 4194);
const origin = `http://127.0.0.1:${port}`;
const resumeKey = 'solomon-darker-rust-native-resume-v1';
const working = fs.mkdtempSync(path.join(os.tmpdir(), 'solomon-native-resume-'));
const savePath = path.join(working, 'host.json');
const checks = [];
const browserErrors = [];
const requests = new Set();
const hostLog = [];
let host;

async function waitUntil(predicate, label, timeout = 30000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (await predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for ${label}`);
}

async function startHost() {
  const args = ['--no-open', '--port', String(port), '--save', savePath, '--assets', assets];
  host = spawn(binary, args, {
    cwd: working,
    env: {...process.env, PATH: ''},
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let ready = false;
  let startupError;
  host.on('error', error => { startupError = error; });
  host.stdout.on('data', data => {
    const line = data.toString();
    hostLog.push(line);
    if (line.includes('is running at')) ready = true;
  });
  host.stderr.on('data', data => hostLog.push(data.toString()));
  await waitUntil(() => {
    if (startupError) throw startupError;
    if (host.exitCode !== null) {
      throw new Error(`Host exited ${host.exitCode}: ${hostLog.join('')}`);
    }
    return ready;
  }, 'isolated native host startup');
}

async function stopHost() {
  if (!host || host.exitCode !== null) return;
  host.kill('SIGINT');
  await waitUntil(() => host.exitCode !== null, 'native checkpoint and shutdown');
  assert.equal(host.exitCode, 0, 'the owned native host shuts down cleanly');
}

function observe(page) {
  const state = {frame: null, id: null, token: null, status: null, errors: [], hellos: [], saves: 0};
  page.on('pageerror', error => browserErrors.push(error.message));
  page.on('console', message => {
    if (message.type() === 'error') browserErrors.push(message.text());
  });
  page.on('response', response => {
    if (response.status() >= 400) browserErrors.push(`${response.status()} ${response.url()}`);
  });
  page.on('websocket', socket => {
    socket.on('framereceived', ({payload}) => {
      const message = JSON.parse(payload.toString());
      if (message.type === 'welcome') {
        state.id = message.credentials.player_id;
        state.token = message.credentials.token;
        state.frame = message.frame;
      } else if (message.type === 'snapshot') {
        state.frame = message.frame;
      } else if (message.type === 'session_status') {
        state.status = message;
      } else if (message.type === 'error') {
        state.errors.push({code: message.code, reason: message.reason});
      } else if (message.type === 'save_committed') {
        state.saves += 1;
      }
    });
    socket.on('framesent', ({payload}) => {
      const message = JSON.parse(payload.toString());
      if (message.type === 'hello') {
        // Credentials stay in process memory for equality checks, never in receipts.
        state.hellos.push({resume: message.resume, selection: message.selection});
      }
    });
  });
  return state;
}

async function storageCredentials(page) {
  return page.evaluate(key => {
    const read = storage => {
      const text = storage.getItem(key);
      return text === null ? null : JSON.parse(text);
    };
    return {session: read(sessionStorage), local: read(localStorage)};
  }, resumeKey);
}

function sameCredentials(actual, expected) {
  return actual?.player_id === expected.player_id && actual?.token === expected.token;
}

async function loadTitle(page, state) {
  await page.goto(`${origin}/?host=1`);
  await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0, null,
    {timeout: 120000});
  await waitUntil(() => state.status !== null, 'native title inspection');
  await page.waitForFunction(() => document.querySelector('#status')?.textContent === '');
  await page.locator('.game-menus[data-menu-screen="title"]').waitFor({state: 'attached'});
}

async function continueGame(page) {
  await page.locator('[data-menu-action="play"]').click();
  await page.locator('[data-menu-action="continue"]').click();
}

async function leaveGame(page, state) {
  await page.bringToFront();
  await page.keyboard.press('Escape');
  await page.locator('.game-menus[data-menu-screen="pause"]').waitFor({state: 'attached'});
  const previousSaves = state.saves;
  await page.locator('[data-menu-action="leave"]').click();
  await waitUntil(() => state.saves > previousSaves, 'ordinary Save Before Leave receipt');
  await page.locator('.game-menus[data-menu-screen="title"]').waitFor({state: 'attached'});
}

function digest(file) {
  return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

async function diagnostic(page, state) {
  if (!page) return null;
  const ui = await page.evaluate(() => ({
    menu: document.querySelector('.game-menus')?.dataset.menuScreen,
    status: document.querySelector('#status')?.textContent,
    text: document.body.innerText.slice(-2000),
  })).catch(() => null);
  return {
    id: state.id,
    tick: state.frame?.snapshot.ticks,
    errors: state.errors,
    hellos: state.hellos.map(message => ({
      carriesResume: message.resume !== null && message.resume !== undefined,
      name: message.selection.name,
    })),
    ui,
  };
}

(async () => {
  let browser;
  let first;
  let second;
  let firstState;
  let secondState;
  try {
    fs.mkdirSync(output, {recursive: true});
    await startHost();
    browser = await chromium.launch({
      headless: true,
      executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
    });
    // One context shares localStorage; each page keeps its own sessionStorage.
    const context = await browser.newContext({viewport: {width: 1600, height: 900}});
    await context.route('**/*', route => {
      const url = route.request().url();
      requests.add(url);
      if (new URL(url).origin !== origin) {
        browserErrors.push(`Unbundled request: ${url}`);
        return route.abort();
      }
      return route.continue();
    });
    first = await context.newPage();
    firstState = observe(first);
    await loadTitle(first, firstState);
    assert.equal(firstState.status.can_resume, false, 'the isolated native save is initially empty');
    await startNewGame(first, {name: 'Felix', element: 'fire', discipline: 'body'});
    const originalId = firstState.id;
    const originalToken = firstState.token;
    assert.ok(originalId && originalToken);
    checks.push('ordinary College admission and final Create loadout');
    console.log(checks.at(-1));

    await leaveGame(first, firstState);
    const original = {player_id: originalId, token: originalToken};
    const firstStored = await storageCredentials(first);
    assert.ok(sameCredentials(firstStored.session, original), 'the first tab retains its credential');
    assert.ok(sameCredentials(firstStored.local, original), 'shared storage retains the admitted credential');
    checks.push('Save Before Leave retains the first tab credential');

    second = await context.newPage();
    secondState = observe(second);
    await loadTitle(second, secondState);
    assert.equal(secondState.status.can_resume, true, 'the second tab discovers the shared credential');
    await continueGame(second);
    await waitUntil(() => secondState.id === originalId && secondState.token !== originalToken,
      'ordinary Continue rotates the admitted credential');
    await second.locator('.game-menus[data-menu-screen="hidden"]').waitFor({state: 'attached'});
    const rotated = {player_id: originalId, token: secondState.token};
    // Wire observation precedes the next browser frame. Continue hides its menu
    // when queued, so that alone cannot prove Welcome has reached native storage.
    await waitUntil(async () => {
      const stored = await storageCredentials(second);
      return sameCredentials(stored.session, rotated) && sameCredentials(stored.local, rotated)
        && await second.locator('#status').textContent() === '';
    }, 'the browser applies Welcome and stores the rotated credential');
    const secondStored = await storageCredentials(second);
    assert.ok(sameCredentials(secondStored.session, rotated), 'the newer tab stores its rotated credential');
    assert.ok(sameCredentials(secondStored.local, rotated), 'shared storage advances to the rotated credential');
    checks.push('second tab Continue rotates the same wizard and persists its new credential');
    console.log(checks.at(-1));

    await first.bringToFront();
    await continueGame(first);
    await waitUntil(() => firstState.errors.some(error => error.code === 'invalid_credentials'),
      'stale first-tab Continue rejection');
    await first.locator('.game-menus[data-menu-screen="title"]').waitFor({state: 'attached'});
    await first.locator('[data-menu-action="play"]').click();
    await first.locator('[data-menu-action="continue"]').waitFor({state: 'attached'});
    assert.equal(await first.locator('[data-menu-action="continue"]').isEnabled(), false,
      'Continue is disabled after InvalidCredentials');
    assert.ok(sameCredentials(firstState.hellos.at(-1).resume, original),
      'the failure came from the actual obsolete tab credential');
    const afterRejection = await storageCredentials(first);
    assert.ok(afterRejection.session === null, 'the rejected matching tab credential is removed');
    assert.ok(sameCredentials(afterRejection.local, rotated),
      'rejecting an old tab preserves the newer shared credential');
    const newerStoredAfterRejection = await storageCredentials(second);
    assert.ok(sameCredentials(newerStoredAfterRejection.session, rotated)
      && sameCredentials(newerStoredAfterRejection.local, rotated),
      'the newer tab and shared credential remain unchanged across stale rejection');
    checks.push('stale Continue rejects once, disables Continue and removes only its matching pair');
    await first.screenshot({path: path.join(output, 'rejected-continue.png')});
    await second.screenshot({path: path.join(output, 'newer-tab-credential-preserved.png')});
    console.log(checks.at(-1));

    // Return through ordinary menu navigation so the existing admission helper is reused.
    await first.keyboard.press('Escape');
    await first.locator('.game-menus[data-menu-screen="title"]').waitFor({state: 'attached'});
    const helloCount = firstState.hellos.length;
    await startNewGame(first, {name: 'Novus', element: 'water', discipline: 'arcane'});
    assert.equal(firstState.hellos.length, helloCount + 1);
    assert.ok(firstState.hellos.at(-1).resume === null,
      'fresh New Game does not resend the rejected resume credential');
    assert.notEqual(firstState.id, originalId, 'fresh admission creates a distinct wizard');
    const fresh = firstState.frame.snapshot.participants[firstState.id];
    assert.equal(fresh.player.selection.name, 'Novus');
    assert.equal(fresh.wizard.region, 'Courtyard');
    await waitUntil(() => secondState.frame.snapshot.participants[firstState.id]?.player.selection.name === 'Novus',
      'the newer tab observes the fresh canonical participant');
    const finalSecondStorage = await storageCredentials(second);
    assert.ok(sameCredentials(finalSecondStorage.session, rotated),
      'fresh admission in the first tab preserves the newer tab identity');
    assert.equal(secondState.id, originalId);
    assert.deepEqual(firstState.errors.map(error => error.code), ['invalid_credentials']);
    assert.deepEqual(secondState.errors, []);
    checks.push('fresh New Game sends no rejected resume and completes the real College flow');
    await first.screenshot({path: path.join(output, 'fresh-new-game.png')});
    assert.ok([...requests].some(url => url.endsWith('.wasm')));
    assert.deepEqual(browserErrors, []);

    await context.close();
    await stopHost();
    const save = JSON.parse(fs.readFileSync(savePath, 'utf8'));
    const wasmFiles = fs.readdirSync(assets).filter(file => file.endsWith('.wasm'));
    const receipt = {
      passed: true,
      checks,
      origin,
      clientRevision: process.env.CLIENT_REVISION || null,
      binarySha256: digest(binary),
      indexSha256: digest(path.join(assets, 'index.html')),
      wasm: wasmFiles.map(file => ({file, sha256: digest(path.join(assets, file))})),
      requests: requests.size,
      expectedHostErrors: firstState.errors,
      browserErrors,
      sameWizardOnContinue: secondState.id === originalId,
      tokenRotated: secondState.token !== originalToken,
      newerTabStoragePreservedAtRejection: true,
      newerTabSessionPreservedAfterFreshAdmission: true,
      freshWizardIsDistinct: firstState.id !== originalId,
      nativeSaveBytes: fs.statSync(savePath).size,
      nativeSaveVersion: save.version,
    };
    fs.writeFileSync(path.join(output, 'native-resume-credentials-receipt.json'), JSON.stringify(receipt, null, 2));
    console.log(JSON.stringify(receipt));
  } catch (error) {
    const state = {
      first: await diagnostic(first, firstState),
      second: await diagnostic(second, secondState),
    };
    for (const [name, page] of [['first', first], ['second', second]]) {
      if (page && !page.isClosed()) {
        await page.screenshot({path: path.join(output, `failure-${name}.png`)}).catch(() => {});
      }
    }
    fs.writeFileSync(path.join(output, 'native-resume-credentials-failure.json'), JSON.stringify({
      error: error.stack, checks, browserErrors, state, hostLog,
    }, null, 2));
    throw error;
  } finally {
    if (browser) await browser.close();
    await stopHost();
    fs.rmSync(working, {recursive: true, force: true});
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
