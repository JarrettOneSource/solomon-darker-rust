const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
// Actual registered-account score publication, without injected state or protocol.
// Source: host/game-host.ts:6863, MainMenuScene.tsx:843 and GameLeaderboardEndpoints.cs.
// The gate approach follows native-session.cjs; all movement uses real keyboard input.
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const {createNativeMessageReader} = require('./native-wire.cjs');
const gpu = require('./gpu-ui.cjs');
const {startNewGame} = require('./menu-journey.cjs');
const {chromium} = require(path.resolve(process.env.PW));

const url = new URL(process.env.URL || 'https://127.0.0.1:9449/game-stg/');
const mount = url.pathname.replace(/\/$/, '');
const output = path.resolve(process.env.OUT);
// Only initial asset residency uses this override; later behavior waits are unchanged.
const coldStartTimeout = Number(process.env.COLD_START_TIMEOUT_MS || 240000);
assert.ok(Number.isSafeInteger(coldStartTimeout) && coldStartTimeout > 0,
  'COLD_START_TIMEOUT_MS must be a positive integer');
const checks = [];
const errors = [];
const failedResources = [];
const pendingPageArt = new Set();
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

async function observe(page) {
  const observation = gpu.watchRequestFailures(page, {errors});
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
  await observation.ready;
}

async function gameTitle(page, username) {
  await page.waitForURL(current => current.origin === url.origin
    && current.pathname === `${mount}/`);
  await gpu.ready(page, {timeout: coldStartTimeout});
  if (gpu.findWidget(await gpu.readUi(page), 'prompt-secondary')) {
    await gpu.click(page, 'prompt-secondary');
    await gpu.noWidget(page, 'prompt-secondary');
  }
  await gpu.surface(page, 'title');
  await gpu.value(page, 'title-account', username || 'Not logged in');
  await gpu.assertGpuOnly(page);
  await settlePageArt(page);
}

async function settlePageArt(page) {
  await page.evaluate(() => document.fonts.ready.then(() => null));
  await waitUntil(() => pendingPageArt.size === 0, 'requested page images and fonts');
  await gpu.settleErrors(page);
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
  await gpu.observeDocumentReplacement(page, () =>
    page.getByRole('navigation').getByRole('link', {name: 'Play', exact: true}).click());
}

async function register(page) {
  const suffix = `${Date.now().toString(36)}${crypto.randomBytes(2).toString('hex')}`;
  account = {username: `Score${suffix}`, email: `score${suffix}@example.invalid`,
    password: `Qa!${crypto.randomBytes(18).toString('base64url')}`};
  const privateFile = path.join(output, 'private-account.json');
  fs.writeFileSync(privateFile, JSON.stringify(account, null, 2), {mode: 0o600, flag: 'wx'});
  fs.chmodSync(privateFile, 0o600);
  await gpu.click(page, 'explore');
  await gpu.widget(page, 'cloud-sign-in', {enabled:true});
  await settlePageArt(page);
  await gpu.observeDocumentReplacement(page, () => gpu.click(page, 'cloud-sign-in'));
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
  await gpu.click(page, 'explore');
  await gpu.widget(page, 'cloud-sign-in', {enabled:true});
  await settlePageArt(page);
  await gpu.observeDocumentReplacement(page, () => gpu.click(page, 'cloud-sign-in'));
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
  const target = await gpu.widget(page, action, {enabled:true});
  hallActivations.push({action, mode, hitTarget: {rect:target.rect, role:target.role, enabled:target.enabled}});
  if (mode === 'keyboard') {
    for (let attempt = 0; attempt < 160; attempt += 1) {
      const snapshot = await gpu.readUi(page);
      const actual = snapshot.widgets.find(widget => widget.focused);
      if (actual?.id === action) break;
      const backwards = actual?.id === 'hall-back'
        || (actual?.id.startsWith('hall-') && !actual.id.startsWith('hall-toggle:')
          && actual.rect[0] > target.rect[0]);
      await page.keyboard.press(backwards ? 'Shift+Tab' : 'Tab');
      await gpu.nextFrame(page);
    }
    await gpu.focused(page, action);
    await page.keyboard.press('Enter');
  } else await gpu.click(page, action);
}

async function revealHallRun(page, runId) {
  await gpu.wheel(page, 'hall-records', 0, -30000);
  for (let attempt = 0; attempt < 45; attempt += 1) {
    const snapshot = await gpu.readUi(page);
    if (snapshot.widgets.some(widget => widget.id.startsWith('hall-toggle:') && widget.value === runId)) return;
    await gpu.wheel(page, 'hall-records', 0, 600);
    await page.waitForTimeout(100);
  }
  throw new Error(`The actual Global Hall did not paint persisted run ${runId}`);
}

async function globalHall(page, runId, mode) {
  await gpu.click(page, 'hall');
  await activateHall(page, 'hall-scope:global', mode);
  await gpu.value(page, 'hall-scope:global', 'true');
  for (const board of ['awesomeness', 'wave', 'kills', 'time']) {
    if (board !== 'awesomeness') await activateHall(page, `hall-board:${board}`, mode);
    await waitUntil(() => hallRequests.some(request => request.status === 200
      && request.board === board && request.entries.some(entry => entry.runId === runId)),
    `persisted actual Global Hall ${board} entry`);
    await gpu.waitForUi(page, snapshot => gpu.findWidget(snapshot, `hall-board:${board}`)?.selected === true,
      `painted selected Global ${board} board`);
    await revealHallRun(page, runId);
    await page.screenshot({path: path.join(output, `persisted-global-hall-${board}.png`)});
  }
  await activateHall(page, 'hall-scope:local', mode);
  await gpu.value(page, 'hall-scope:local', 'true');
  await gpu.assertGpuOnly(page);
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
    checks, errors, failedResources, apiRequests, registrationRequests,
    admissions, welcomes, commands, inputSamples,
    scoreReceipts, scorePosts, hallRequests, hallActivations, healthSamples};
  let browserOwner;
  let browser;
  let page;
  try {
    browserOwner = await launchOwnedBrowser(chromium, {headless: true,
      executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
    browser = browserOwner.browser;
    page = await browser.newPage({ignoreHTTPSErrors: process.env.LOCAL_TLS === '1',
      viewport: {width: 1280, height: 720}});
    page.setDefaultTimeout(30000);
    page.setDefaultNavigationTimeout(30000);
    await observe(page);
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
      await gpu.settleErrors(page);
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
    await gpu.click(page, 'settings');
    await gpu.click(page, 'settings-page:cloud');
    const submit = 'settings-toggle:submit-runs';
    if ((await gpu.widget(page, submit)).value !== 'true') await gpu.click(page, submit);
    await gpu.value(page, submit, 'true');
    await gpu.value(page, 'settings-toggle:cheats', 'false');
    await gpu.click(page, 'settings-back');
    await gpu.click(page, 'settings-back');
    await gpu.surface(page, 'title');
    checked('ordinary Settings show Submit Runs enabled and cheats disabled');

    step = 'fresh eligible Global admission';
    await startNewGame(page, {name:'Score Witness', element:'fire', discipline:'arcane'});
    await waitUntil(() => welcomes.length === 1, 'real Global Welcome');
    await gpu.surface(page, 'hidden');
    assert.equal(admissions.length, 1);
    assert.deepEqual(admissions[0], {path: `${mount}/game-hub`, hasInitialSave: false,
      cheatsEnabled: false, submitRuns: true});
    assert.deepEqual(welcomes[0], {sessionKind: 'GlobalHub', cheatsEnabled: false, mods: 0, observer: false});
    receipt.wizardName = participant().player.selection.name;
    checked('fresh Global authority admits the authenticated wizard without saves, cheats or mods');

    step = 'actual Boneyard readiness and entrance';
    await gpu.click(page, 'hud:map');
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
    await gpu.widget(page, 'game-over:continue');
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
    await gpu.settleErrors(page);
    assert.deepEqual(errors, []);
    checked('the real signed completion receipt is POSTed once and the unchanged API returns 201');

    step = 'ordinary post-run continuation and Global Hall';
    await gpu.click(page, 'game-over:continue');
    await gpu.surface(page, 'create');
    await gpu.click(page, 'element:fire');
    await gpu.click(page, 'discipline:arcane');
    await waitUntil(() => state.frame.snapshot.boneyard === null
      && state.frame.snapshot.run.phase === 'Hub', 'ordinary post-run College');
    await gpu.surface(page, 'hidden');
    await page.keyboard.press('Escape');
    await gpu.surface(page, 'pause');
    await gpu.click(page, 'leave');
    await gpu.surface(page, 'title');
    assert.ok(state.leaveReceipts > 0);
    await globalHall(page, runId, 'pointer');
    assert.equal(commands.some(command => command.kind === 'set_cheats'), false);
    await gpu.settleErrors(page);
    assert.deepEqual(errors, []);
    checked('normal continuation and save-before-leave reach a Global Hall row backed by the actual API');
    receipt.passed = true;
  } catch (error) {
    receipt.failure = {step, message: redact(error.message), stack: redact(error.stack)};
    if (page) {
      receipt.failureUi = await gpu.readUi(page).catch(() => null);
      await page.screenshot({path: path.join(output, 'failure.png')}).catch(() => {});
    }
    throw error;
  } finally {
    try {
      if (page && !page.isClosed()) await gpu.settleErrors(page);
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
      let cleanupFailure;
      if (browserOwner) {
        try {receipt.cleanup = await browserOwner.close();}
        catch (error) {
          receipt.cleanup = error.cleanup;
          receipt.cleanupError = redact(error.stack || error.message);
          receipt.passed = false;
          cleanupFailure = error;
        }
        receipt.browserClosed = Boolean(receipt.cleanup
          && (receipt.cleanup.exitCode !== null || receipt.cleanup.signalCode !== null));
        ownedBrowserClosed = receipt.browserClosed;
      }
      fs.writeFileSync(path.join(output, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n');
      console.log(JSON.stringify({passed: receipt.passed, checks, failure: receipt.failure,
        wasmFiles: receipt.wasmFiles, browserClosed: receipt.browserClosed}));
      if (cleanupFailure) throw cleanupFailure;
    }
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
