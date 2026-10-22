// Actual registered-account score publication, without injected state or protocol.
// Source: host/game-host.ts:6863, MainMenuScene.tsx:843 and GameLeaderboardEndpoints.cs.
// The gate approach follows native-session.cjs; all movement uses real keyboard input.
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const {createNativeMessageReader} = require('./native-wire.cjs');
const obsoleteMenuImage = require('./obsolete-menu-image.cjs');
const {chromium} = require(path.resolve(process.env.PW));

const url = new URL(process.env.URL || 'https://127.0.0.1:9449/game-stg/');
const mount = url.pathname.replace(/\/$/, '');
const output = path.resolve(process.env.OUT);
// Only initial asset residency uses this override; later behavior waits are unchanged.
const coldStartTimeout = Number(process.env.COLD_START_TIMEOUT_MS || 30000);
assert.ok(Number.isSafeInteger(coldStartTimeout) && coldStartTimeout > 0,
  'COLD_START_TIMEOUT_MS must be a positive integer');
const checks = [];
const errors = [];
const cancelledMedia = [];
const cancelledImages = [];
const failedResources = [];
const pendingPageArt = new Set();
const resourceChecks = [];
const apiRequests = [];
const registrationRequests = [];
const scoreReceipts = [];
const scorePosts = [];
const hallRequests = [];
const hallActivations = [];
const healthSamples = [];
const approach = [];
const admissions = [];
const welcomes = [];
const commands = [];
const inputSamples = [];
const wasmFiles = new Set();
const state = {frame: null, id: null, loaded: null, userId: null, leaveReceipts: 0, lastInput: null};
let account;
let step = 'browser startup';
let ownedBrowserClosed = false;

function digest(value) {
  return crypto.createHash('sha256').update(value).digest('hex');
}

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

async function waitUntil(predicate, label, timeout = 30000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (await predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for ${label}`);
}

async function bounded(promise, label, milliseconds) {
  let timer;
  try {
    return await Promise.race([promise, new Promise((resolve, reject) => {
      timer = setTimeout(() => reject(new Error(`${label} timed out`)), milliseconds);
    })]);
  } finally {
    clearTimeout(timer);
  }
}

function participant() {
  return state.frame?.snapshot.participants[state.id];
}

function observeFrame(frame) {
  state.frame = frame;
  if (!frame.snapshot.boneyard || !participant()) return;
  const player = participant();
  const previous = healthSamples.at(-1);
  if (!previous || frame.snapshot.ticks - previous.tick >= 100
    || previous.lifeState !== player.player.combat.life_state
    || previous.phase !== frame.snapshot.run.phase) {
    healthSamples.push({tick: frame.snapshot.ticks, phase: frame.snapshot.run.phase,
      lifeState: player.player.combat.life_state, health: player.player.combat.current_health,
      enemies: frame.snapshot.boneyard.enemies.actors.length,
      elapsedTicks: player.score.elapsed_ticks, kills: player.score.monsters_killed,
      awesomeness: player.score.awesomeness});
  }
}

function observe(page) {
  page.on('request', request => {
    if (['image', 'font'].includes(request.resourceType())) pendingPageArt.add(request);
    if (new URL(request.url()).pathname === `${mount}/api/auth/register`
      && request.method() === 'POST') {
      const submitted = request.postDataJSON();
      registrationRequests.push({usernameMatches: submitted.username === account.username,
        emailMatches: submitted.email === account.email,
        passwordMatches: submitted.password === account.password,
        usernameLength: submitted.username?.length});
    }
  });
  page.on('requestfinished', request => pendingPageArt.delete(request));
  page.on('pageerror', error => errors.push(redact(error.message)));
  page.on('console', message => {
    if (message.type() === 'error') errors.push(redact(message.text()));
  });
  page.on('response', async response => {
    const pathname = new URL(response.url()).pathname;
    const request = response.request();
    if (pathname.endsWith('.wasm')) wasmFiles.add(path.basename(pathname));
    if (pathname.includes('/api/')) {
      apiRequests.push({path: pathname, method: request.method(), status: response.status()});
    }
    if (response.status() >= 400) errors.push(`${response.status()} ${pathname}`);
    try {
      if (pathname === `${mount}/api/auth/me` && response.status() === 200) {
        const body = await response.json();
        if (body.user?.username === account.username) state.userId = body.user.id;
      }
      if (pathname === `${mount}/api/game/leaderboards`) {
        const body = await response.json();
        if (request.method() === 'POST') {
          const submitted = request.postDataJSON();
          scorePosts.push({status: response.status(), receiptSha256: digest(submitted.receipt), body});
        } else {
          hallRequests.push({status: response.status(), board: body.board,
            entries: body.items?.map(entry => ({runId: entry.runId,
              accountUsername: entry.accountUsername, wizardName: entry.wizardName})) || [],
            error: body.message});
        }
      }
    } catch (error) {
      errors.push(`response observation ${pathname}: ${redact(error.message)}`);
    }
  });
  page.on('requestfailed', request => {
    pendingPageArt.delete(request);
    failedResources.push({path: new URL(request.url()).pathname,
      type: request.resourceType(), failure: request.failure()?.errorText,
      step, pagePath: new URL(page.url()).pathname});
    const check = (async () => {
      const cancelled = await obsoleteMenuImage(page, request);
      if (cancelled) { cancelledImages.push(cancelled); return; }
      const failure = `${new URL(request.url()).pathname}: ${request.failure()?.errorText}`;
      if (request.resourceType() === 'media' && request.failure()?.errorText === 'net::ERR_ABORTED') {
        cancelledMedia.push(failure);
      } else errors.push(redact(failure));
    })();
    resourceChecks.push(check);
  });
  page.on('websocket', socket => {
    const read = createNativeMessageReader();
    socket.on('framesent', ({payload}) => {
      const message = JSON.parse(payload.toString());
      if (message.type === 'hosted' && message.command.kind === 'hello') {
        const hello = message.command.hello;
        admissions.push({path: new URL(socket.url()).pathname,
          hasInitialSave: hello.save !== null, cheatsEnabled: hello.cheats_enabled,
          submitRuns: hello.online_preferences.submitRuns});
      } else if (message.type === 'hosted' && message.command.kind === 'set_cheats') {
        commands.push({kind: 'set_cheats', enabled: message.command.enabled});
      } else if (message.type === 'command') {
        commands.push({kind: message.command.kind, sequence: message.sequence});
      } else if (message.type === 'input') {
        state.lastInput = {sequence: message.sequence, targetTick: message.target_tick,
          movement: message.input.movement};
        inputSamples.push({...state.lastInput, observedAtMs: Date.now()});
      }
    });
    socket.on('framereceived', ({payload}) => {
      const message = read(JSON.parse(payload.toString()));
      if (!message) return;
      if (message.type === 'welcome') {
        state.id = message.credentials.player_id;
        welcomes.push({sessionKind: message.hosted.session_kind,
          cheatsEnabled: message.hosted.cheats_enabled,
          mods: message.hosted.content.mods.length, observer: message.hosted.observer});
        observeFrame(message.frame);
      } else if (message.type === 'snapshot') {
        observeFrame(message.frame);
      } else if (message.type === 'boneyard_loaded') {
        state.loaded = message.boneyard;
      } else if (message.type === 'error') {
        errors.push(`host ${message.code}: ${message.reason}`);
      } else if (message.type === 'hosted') {
        if (message.message.kind === 'leaderboard_receipt') {
          const receipt = message.message.receipt;
          const payload = JSON.parse(Buffer.from(receipt.split('.')[0], 'base64url').toString());
          scoreReceipts.push({sha256: digest(receipt), payload});
        } else if (message.message.kind === 'save_before_leave') {
          state.leaveReceipts += 1;
        }
      }
    });
  });
}

async function gameTitle(page, username) {
  await page.waitForURL(current => current.origin === url.origin
    && current.pathname === `${mount}/`);
  await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0
    && document.querySelector('#status')?.textContent === '', null, {timeout: coldStartTimeout});
  const prompt = page.locator('.title-prompt:not([hidden])');
  if (await prompt.isVisible()) {
    await prompt.locator('[data-menu-action="prompt-secondary"]').click();
  }
  await page.locator('.game-menus[data-menu-screen="title"]').waitFor();
  await page.waitForFunction(expected => document.querySelector('.game-account-name-title')
    ?.getAttribute('data-account-username') === expected, username || 'Not logged in');
  await settlePageArt(page);
}

async function settlePageArt(page) {
  await page.evaluate(() => document.fonts.ready.then(() => null));
  await waitUntil(() => pendingPageArt.size === 0, 'requested page images and fonts');
}

async function settledAccountForm(page) {
  await page.waitForFunction(() => {
    const reveal = document.querySelector('form')?.closest('.reveal-in');
    return reveal && getComputedStyle(reveal).opacity === '1';
  });
}

// Current Website forms return to Account; Play opens the independent game artifact.
// Source: pages/{Register,Login}.tsx and components/Shell.tsx.
async function returnFromAccount(page) {
  await page.waitForURL(current => current.origin === url.origin
    && current.pathname === `${mount}/account`);
  await page.evaluate(() => document.fonts.ready.then(() => null));
  await page.getByRole('navigation').getByRole('link', {name: 'Play', exact: true}).click();
}

async function register(page) {
  const suffix = `${Date.now().toString(36)}${crypto.randomBytes(2).toString('hex')}`;
  account = {username: `Score${suffix}`, email: `score${suffix}@example.invalid`,
    password: `Qa!${crypto.randomBytes(18).toString('base64url')}`};
  const privateFile = path.join(output, 'private-account.json');
  fs.writeFileSync(privateFile, JSON.stringify(account, null, 2), {mode: 0o600, flag: 'wx'});
  fs.chmodSync(privateFile, 0o600);
  await page.locator('[data-menu-action="explore"]').click();
  await page.locator('[data-menu-action="cloud-sign-in"]').waitFor({state: 'visible'});
  await settlePageArt(page);
  await page.locator('[data-menu-action="cloud-sign-in"]').click();
  await page.waitForURL(current => current.pathname === `${mount}/login`);
  await settledAccountForm(page);
  await page.getByRole('link', {name: 'Create an account', exact: true}).click();
  await page.waitForURL(current => current.pathname === `${mount}/register`);
  await settledAccountForm(page);
  for (const [autocomplete, value] of [['username', account.username],
    ['email', account.email], ['new-password', account.password]]) {
    const input = page.locator(`input[autocomplete="${autocomplete}"]`);
    await input.pressSequentially(value, {delay: 15});
    await input.press('Tab');
    assert.equal(await input.inputValue(), value);
  }
  const registered = page.waitForResponse(response => new URL(response.url()).pathname
    === `${mount}/api/auth/register` && response.request().method() === 'POST');
  await page.getByRole('button', {name: 'Create account', exact: true}).click();
  assert.equal((await registered).status(), 201);
  assert.deepEqual(registrationRequests, [{usernameMatches: true, emailMatches: true,
    passwordMatches: true, usernameLength: account.username.length}]);
  await returnFromAccount(page);
  await gameTitle(page, account.username);
  await waitUntil(() => state.userId !== null, 'actual authenticated account ID');
}

async function signIn(page) {
  await page.locator('[data-menu-action="explore"]').click();
  await page.locator('[data-menu-action="cloud-sign-in"]').waitFor({state: 'visible'});
  await settlePageArt(page);
  await page.locator('[data-menu-action="cloud-sign-in"]').click();
  await page.waitForURL(current => current.pathname === `${mount}/login`);
  await settledAccountForm(page);
  for (const [autocomplete, value] of [['username', account.username],
    ['current-password', account.password]]) {
    const input = page.locator(`input[autocomplete="${autocomplete}"]`);
    await input.pressSequentially(value, {delay: 15});
    await input.press('Tab');
    assert.equal(await input.inputValue(), value);
  }
  const signedIn = page.waitForResponse(response => new URL(response.url()).pathname
    === `${mount}/api/auth/login` && response.request().method() === 'POST');
  await page.getByRole('button', {name: 'Sign in', exact: true}).click();
  assert.equal((await signedIn).status(), 200);
  await returnFromAccount(page);
  await gameTitle(page, account.username);
  await waitUntil(() => state.userId !== null, 'actual signed-in account ID');
}

async function activateHall(page, action, mode) {
  const button = page.locator(`[data-menu-action="${action}"]`);
  await button.waitFor({state: 'visible'});
  const hitTarget = await button.evaluate(button => {
    const bounds = button.getBoundingClientRect();
    const hit = document.elementFromPoint(bounds.x + bounds.width / 2,
      bounds.y + bounds.height / 2);
    return {buttonClass: button.className, hitClass: hit?.className,
      hitAction: hit?.closest('[data-menu-action]')?.getAttribute('data-menu-action')};
  });
  hallActivations.push({action, mode, hitTarget});
  if (mode === 'keyboard') await button.press('Enter');
  else await button.click();
}

async function globalHall(page, runId, mode) {
  await page.locator('[data-menu-action="hall"]').click();
  await activateHall(page, 'hall-scope:global', mode);
  for (const board of ['awesomeness', 'wave', 'kills', 'time']) {
    if (board !== 'awesomeness') {
      await activateHall(page, `hall-board:${board}`, mode);
    }
    await waitUntil(() => hallRequests.some(request => request.status === 200
      && request.board === board && request.entries.some(entry => entry.runId === runId)),
    `persisted actual Global Hall ${board} entry`);
    await page.waitForFunction(({runId, board}) => {
      const hall = document.querySelector('.hall-screen');
      return hall?.dataset.hallScope === 'global' && hall?.dataset.hallBoard === board
        && [...document.querySelectorAll('.hall-row')].some(row => row.dataset.hallRun === runId);
    }, {runId, board});
    await page.screenshot({path: path.join(output, `persisted-global-hall-${board}.png`)});
  }
  await activateHall(page, 'hall-scope:local', mode);
  await page.waitForFunction(() => document.querySelector('.hall-screen')?.dataset.hallScope === 'local');
}

async function approachSolomon(page) {
  const world = state.frame.snapshot.boneyard;
  const start = participant().wizard.position;
  const gates = new Map();
  for (const leaf of world.gate_leaves) {
    const pair = gates.get(leaf.fence_eid) || [];
    pair.push(leaf.hinge);
    gates.set(leaf.fence_eid, pair);
  }
  const gate = [...gates.values()].filter(pair => pair.length === 2)
    .map(pair => ({x: (pair[0].x + pair[1].x) / 2, y: (pair[0].y + pair[1].y) / 2}))
    .sort((a, b) => Math.hypot(a.x - start.x, a.y - start.y)
      - Math.hypot(b.x - start.x, b.y - start.y))[0];
  assert.ok(gate, 'actual entrance has its authored gate');
  const direction = world.arena_transition.entry_side === 'North' ? 1 : -1;
  const targets = [{x: gate.x, y: gate.y - direction * 75},
    {x: gate.x, y: gate.y + direction * 150}, world.encounter.position];
  let waypoint = 0;
  const deadline = Date.now() + 90000;
  try {
    while (state.frame.snapshot.boneyard.encounter.phase === 'Digging' && Date.now() < deadline) {
      const at = participant().wizard.position;
      const target = targets[waypoint];
      const dx = target.x - at.x;
      const dy = target.y - at.y;
      const close = waypoint === targets.length - 1
        ? Math.hypot(dx, dy) < 90 : Math.abs(dx) <= 4 && Math.abs(dy) <= 10;
      if (close) {
        if (waypoint < targets.length - 1) waypoint += 1;
        else await page.waitForTimeout(50);
        continue;
      }
      // Recenter outside the swinging leaves if an earlier contact pushed us
      // beside the opening. Source's entrance navigator separates approach/crossing.
      if (waypoint === 1 && Math.abs(dx) > 40 && (at.y - gate.y) * direction < 45) {
        const away = direction > 0 ? 'KeyW' : 'KeyS';
        await keyPulse(page, away, 160);
        approach.push({tick: state.frame.snapshot.ticks, position: participant().wizard.position,
          target: targets[0], waypoint, recovery: 'back away before recentering'});
        if ((gate.y - participant().wizard.position.y) * direction >= 70) waypoint = 0;
        continue;
      }
      const horizontal = Math.abs(dx) > (waypoint < targets.length - 1 ? 4 : 8);
      const distance = Math.abs(horizontal ? dx : dy);
      const key = horizontal ? (dx < 0 ? 'KeyA' : 'KeyD') : (dy < 0 ? 'KeyW' : 'KeyS');
      const pulseMs = Math.max(30, Math.min(160, distance / 118.75 * 1000));
      const pulse = await keyPulse(page, key, pulseMs);
      approach.push({tick: state.frame.snapshot.ticks, position: participant().wizard.position,
        target, waypoint, key, pulseMs, ...pulse});
    }
    assert.notEqual(state.frame.snapshot.boneyard.encounter.phase, 'Digging');
  } finally {
    for (const key of ['KeyA', 'KeyD', 'KeyW', 'KeyS']) await page.keyboard.up(key);
  }
}

async function keyPulse(page, key, milliseconds) {
  const before = state.lastInput?.sequence || 0;
  const started = Date.now();
  await page.keyboard.down(key);
  try {
    await waitUntil(() => state.lastInput?.sequence > before
      && (state.lastInput.movement.x !== 0 || state.lastInput.movement.y !== 0),
    'ordinary keyboard movement sampled by the client');
    await page.waitForTimeout(Math.max(0, milliseconds - (Date.now() - started)));
  } finally {
    await page.keyboard.up(key);
  }
  await waitUntil(() => state.lastInput?.sequence > before
    && state.lastInput.movement.x === 0 && state.lastInput.movement.y === 0,
  'ordinary key release sampled by the client');
  const released = state.lastInput.sequence;
  await waitUntil(() => state.frame.input_acknowledgements[state.id] >= released,
    'authority observation after the actual key release');
  return {releasedSequence: released, acknowledgedSequence: state.frame.input_acknowledgements[state.id]};
}

async function main() {
  fs.mkdirSync(output, {recursive: true});
  const receipt = {passed: false, url: url.href,
    driverSha256: digest(fs.readFileSync(__filename)),
    sourceRevision: process.env.SOURCE_REV || 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83',
    scope: 'Real registration, unmodified hosted gameplay, passive combat death, signed score POST and Global Hall',
    checks, errors, cancelledMedia, cancelledImages, failedResources, apiRequests, registrationRequests,
    admissions, welcomes, commands, inputSamples,
    scoreReceipts, scorePosts, hallRequests, hallActivations, healthSamples};
  let browserServer;
  let browser;
  let page;
  try {
    browserServer = await chromium.launchServer({headless: true, host: '127.0.0.1',
      executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
    browser = await chromium.connect(browserServer.wsEndpoint());
    page = await browser.newPage({ignoreHTTPSErrors: process.env.LOCAL_TLS === '1',
      viewport: {width: 1280, height: 720}});
    page.setDefaultTimeout(30000);
    page.setDefaultNavigationTimeout(30000);
    observe(page);
    await page.goto(url.href, {waitUntil: 'load'});
    await gameTitle(page, null);
    if (process.env.PUBLISHED_SCORE) {
      // A separate new-browser data/pointer check reads a genuinely published run.
      // Credentials are used only by the normal mounted login form, never admission.
      const publishedPath = path.resolve(process.env.PUBLISHED_SCORE);
      const publishedBytes = fs.readFileSync(publishedPath);
      const published = JSON.parse(publishedBytes);
      assert.equal(published.scorePosts[0].status, 201);
      const entry = published.scorePosts[0].body;
      account = JSON.parse(fs.readFileSync(path.join(path.dirname(publishedPath), 'private-account.json')));
      assert.equal(entry.accountUsername, account.username);
      const mode = process.env.HALL_ACTIVATION || 'pointer';
      assert.ok(mode === 'pointer' || mode === 'keyboard');
      receipt.scope = `Separate persisted-score check with ordinary ${mode} Hall activation`;
      receipt.publishedBaseline = {path: publishedPath, sha256: digest(publishedBytes),
        runId: entry.runId, accountUsername: entry.accountUsername, elapsedTicks: entry.elapsedTicks};
      step = 'real fresh-browser sign-in for persisted Hall';
      await signIn(page);
      checked('a fresh browser signs into the actual account through the unchanged mounted form');
      step = `persisted Global Hall via ordinary ${mode}`;
      await globalHall(page, entry.runId, mode);
      await Promise.all(resourceChecks);
      assert.deepEqual(errors, []);
      checked(`the persisted run appears on all four Global boards via ordinary ${mode} activation`);
      receipt.passed = true;
      return;
    }
    step = 'fresh mounted account registration';
    await register(page);
    receipt.accountUsername = account.username;
    receipt.accountUserId = state.userId;
    checked('unchanged mounted registration creates a fresh real authenticated account');

    step = 'ordinary score preferences';
    await page.locator('[data-menu-action="settings"]').click();
    await page.locator('[data-menu-action="settings-page:cloud"]').click();
    const submit = page.locator('[data-setting="submit-runs"]');
    if (await submit.getAttribute('aria-pressed') !== 'true') await submit.click();
    assert.equal(await submit.getAttribute('aria-pressed'), 'true');
    assert.equal(await page.locator('[data-setting="cheats"]').getAttribute('aria-pressed'), 'false');
    await page.locator('[data-menu-action="settings-back"]').click();
    await page.locator('[data-menu-action="settings-back"]').click();
    await page.locator('.game-menus[data-menu-screen="title"]').waitFor();
    checked('ordinary Settings show Submit Runs enabled and cheats disabled');

    step = 'fresh eligible Global admission';
    await page.locator('[data-menu-action="play"]').click();
    await page.locator('[data-menu-action="new-game"]').click();
    await page.getByRole('textbox', {name: 'Wizard name', exact: true}).fill('Score Witness');
    // Allow the existing UI frame to accept the typed name before changing panels.
    await page.waitForTimeout(100);
    await page.locator('[data-menu-action="element:fire"]').click();
    await page.locator('[data-menu-action="discipline:arcane"]').click();
    await waitUntil(() => welcomes.length === 1, 'real Global Welcome');
    await page.locator('.game-menus[data-menu-screen="hidden"]').waitFor({state: 'attached'});
    assert.equal(admissions.length, 1);
    assert.deepEqual(admissions[0], {path: `${mount}/game-hub`, hasInitialSave: false,
      cheatsEnabled: false, submitRuns: true});
    assert.deepEqual(welcomes[0], {sessionKind: 'GlobalHub', cheatsEnabled: false, mods: 0, observer: false});
    receipt.wizardName = participant().player.selection.name;
    checked('fresh Global authority admits the authenticated wizard without saves, cheats or mods');

    step = 'actual Boneyard readiness and entrance';
    await page.getByRole('button', {name: 'Enter the Boneyard', exact: true}).click();
    await waitUntil(() => state.frame?.snapshot.boneyard && !state.frame.host.loading_run_id
      && !state.frame.host.resume_grace && !state.frame.host.pause, 'real Boneyard readiness', 120000);
    const runId = state.frame.snapshot.boneyard.run_id;
    receipt.runId = runId;
    assert.equal(state.loaded.run_id, runId);
    assert.equal(state.frame.snapshot.boneyard.tutorial, null);
    assert.equal(participant().score.elapsed_ticks, null);
    await approachSolomon(page);
    await page.screenshot({path: path.join(output, 'ordinary-boneyard-encounter.png')});
    checked('actual keyboard movement through the authored gate wakes Solomon');

    step = 'ordinary enemy combat death';
    await waitUntil(() => state.frame.snapshot.run.phase === 'GameOver', 'actual combat death', 180000);
    await page.locator('.game-over-overlay:not([hidden])').waitFor();
    // Source archives the portrait/elapsed score at death age 300, after Game Over starts.
    await waitUntil(() => participant().score.elapsed_ticks !== null
      && participant().score.portrait_heading_index !== null, 'Source post-death score archive');
    receipt.completedScore = {...participant().score};
    receipt.completedCombat = {lifeState: participant().player.combat.life_state,
      health: participant().player.combat.current_health};
    assert.ok(healthSamples.some(sample => sample.enemies > 0 && sample.health > 0));
    assert.ok(healthSamples.some(sample => sample.health < healthSamples[0].health));
    assert.ok(receipt.completedScore.elapsed_ticks > 0);
    await page.screenshot({path: path.join(output, 'actual-combat-game-over.png')});
    checked('real enemies deplete health and complete the untouched Boneyard run');

    step = 'actual signed receipt and existing API publication';
    await waitUntil(() => scoreReceipts.length > 0 && scorePosts.length > 0,
      'actual LeaderboardReceipt and POST');
    assert.equal(scoreReceipts.length, 1);
    assert.equal(scorePosts.length, 1);
    const published = scorePosts[0];
    assert.equal(published.status, 201, 'the real existing API creates this fresh score');
    assert.equal(published.receiptSha256, scoreReceipts[0].sha256);
    assert.equal(scoreReceipts[0].payload.runId, runId);
    assert.equal(scoreReceipts[0].payload.userId, state.userId);
    assert.equal(published.body.runId, runId);
    assert.equal(published.body.accountUsername, account.username);
    assert.equal(published.body.elapsedTicks, receipt.completedScore.elapsed_ticks);
    assert.equal(published.body.awesomeness, receipt.completedScore.awesomeness);
    await Promise.all(resourceChecks);
    assert.deepEqual(errors, []);
    checked('the real signed completion receipt is POSTed once and the unchanged API returns 201');

    step = 'ordinary post-run continuation and Global Hall';
    const continueButton = page.locator('.boneyard-game-over[data-input-ready="true"]');
    await continueButton.waitFor();
    await continueButton.click();
    await page.locator('.game-menus[data-menu-screen="create"]').waitFor();
    await page.locator('[data-menu-action="element:fire"]').click();
    await page.locator('[data-menu-action="discipline:arcane"]').click();
    await waitUntil(() => state.frame.snapshot.boneyard === null
      && state.frame.snapshot.run.phase === 'Hub', 'ordinary post-run College');
    await page.locator('.game-menus[data-menu-screen="hidden"]').waitFor({state: 'attached'});
    await page.keyboard.press('Escape');
    await page.locator('.game-menus[data-menu-screen="pause"] [data-menu-action="leave"]').click();
    await page.locator('.game-menus[data-menu-screen="title"]').waitFor();
    assert.ok(state.leaveReceipts > 0);
    await globalHall(page, runId, 'pointer');
    assert.equal(commands.some(command => command.kind === 'set_cheats'), false);
    await Promise.all(resourceChecks);
    assert.deepEqual(errors, []);
    checked('normal continuation and save-before-leave reach a Global Hall row backed by the actual API');
    receipt.passed = true;
  } catch (error) {
    receipt.failure = {step, message: redact(error.message), stack: redact(error.stack)};
    if (page) {
      receipt.failureUi = await page.evaluate(() => ({
        status: document.querySelector('#status')?.textContent,
        menu: document.querySelector('.game-menus')?.dataset.menuScreen,
        text: document.body.innerText.slice(-1800),
        acceptedTick: window.wasmBindings?.wizard_state()[5],
      })).catch(() => null);
      await page.screenshot({path: path.join(output, 'failure.png')}).catch(() => {});
    }
    throw error;
  } finally {
    receipt.beforeCleanup = {errors: [...errors], pendingPageArt: [...pendingPageArt]
      .map(request => new URL(request.url()).pathname)};
    receipt.wasmFiles = [...wasmFiles];
    receipt.lastAuthority = state.frame ? {tick: state.frame.snapshot.ticks,
      run: state.frame.snapshot.run, host: state.frame.host,
      position: participant()?.wizard.position} : null;
    fs.writeFileSync(path.join(output, 'solomon-approach.json'), JSON.stringify(approach, null, 2) + '\n');
    fs.writeFileSync(path.join(output, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n');
    step = 'browser cleanup';
    if (browser) {
      await bounded(browser.close(), 'browser connection close', 5000).catch(error => {
        receipt.connectionCloseError = error.message;
      });
    }
    if (browserServer) {
      try {
        await bounded(browserServer.close(), 'Chrome close', 15000);
        receipt.cleanup = 'graceful';
      } catch (error) {
        receipt.cleanupError = error.message;
        await bounded(browserServer.kill(), 'owned Chrome kill', 5000).catch(error => {
          receipt.cleanupError += `; ${error.message}`;
        });
        receipt.cleanup = 'forced';
      }
      receipt.browserClosed = browserServer.process().exitCode !== null
        || browserServer.process().signalCode !== null;
      ownedBrowserClosed = receipt.browserClosed;
    }
    fs.writeFileSync(path.join(output, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n');
    console.log(JSON.stringify({passed: receipt.passed, checks, failure: receipt.failure,
      wasmFiles: receipt.wasmFiles, browserClosed: receipt.browserClosed}));
  }
}

// Some Playwright remote-server sockets outlive their already-exited Chrome child.
// End this owned command only after the receipt confirms that child is closed.
main().then(() => {
  if (ownedBrowserClosed) process.exit(0);
}).catch(error => {
  console.error(redact(error.stack || error.message));
  if (ownedBrowserClosed) process.exit(1);
  process.exitCode = 1;
});
