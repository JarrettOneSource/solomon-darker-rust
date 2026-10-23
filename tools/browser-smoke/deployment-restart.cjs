// Real durable-save/transport-retirement/publication recovery journey.
// Run only while this isolated gateway has no other active acceptance browsers.
// The coordinator publishes real replacement artifacts after ready-to-publish.json.
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require(path.resolve(process.env.PW));
const { createNativeMessageReader } = require('./native-wire.cjs');
const {reachCollegeLoadout}=require('./menu-journey.cjs');
const ui=require('./gpu-ui.cjs');

const base = path.resolve(process.env.LOCAL_GATEWAY);
const out = path.resolve(process.env.OUT);
const url = new URL(process.env.URL);
assert.equal(url.hostname, '127.0.0.1', 'This operator test owns only the isolated gateway');
const mount = url.pathname.replace(/\/$/, '');
const target = process.env.TARGET_REVISION;
assert.match(target, /^[a-f0-9]{40}$/);
const publicationPath = path.resolve(process.env.PUBLICATION_PATH);
const originalManifest = fs.readFileSync(publicationPath);
const originalPublication = JSON.parse(originalManifest);
const current = originalPublication.gameRevision;
assert.match(current, /^[a-f0-9]{40}$/);
assert.match(originalPublication.deploymentId, /^[a-f0-9]{32}$/);
assert.equal(originalPublication.ready, true);
assert.ok(Number.isSafeInteger(originalPublication.deploymentSequence) && originalPublication.deploymentSequence >= 0);
const publicationSequence = originalPublication.deploymentSequence + 1;
if (process.env.SAME_GAME === '1') assert.equal(current, target);
else assert.notEqual(current, target);
const targetWebsite = process.env.TARGET_WEBSITE_REVISION;
assert.match(targetWebsite, /^[a-f0-9]{40}$/);
const publicationId = crypto.randomUUID().replaceAll('-', '');
const publicationEndpoint = `${mount}/api/game/deployment`;
const adminUrl = new URL(process.env.ADMIN_URL || 'http://127.0.0.1:5230');
assert.equal(adminUrl.hostname, '127.0.0.1');
const publish = publication => {
  const temporary = `${publicationPath}.${publicationId}.tmp`;
  fs.writeFileSync(temporary, JSON.stringify(publication));
  fs.renameSync(temporary, publicationPath);
};
const credentials = JSON.parse(fs.readFileSync(path.join(base, 'private.json')));
const checks = [], errors = [], cancelled = [], events = [], requests = [];
const welcomed = [], checkpoints = [];
let frame, playerId, announcement, account;
let heldRoute = null, armSave = false, documentLoads = 0, socketCloses = 0;
let releaseSave;
const saveReleased = new Promise(resolve => { releaseSave = resolve; });
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
const sleep = duration => new Promise(resolve => setTimeout(resolve, duration));
async function until(predicate, label, timeout = 30000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (await predicate()) return;
    await sleep(50);
  }
  throw new Error(`Timed out waiting for ${label}`);
}
function redact(value) {
  let text = String(value).replaceAll(credentials.supervisor, '[private supervisor]');
  if (account) text = text.replaceAll(account.password, '[private password]')
    .replaceAll(account.email, '[private email]');
  return text.replace(/eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+/g, '[private token]');
}
function checked(label) { checks.push(label); console.log(label); }
async function title(page) {
  await page.waitForURL(value => value.origin === url.origin && value.pathname === `${mount}/`);
  await ui.ready(page);
  if(ui.findWidget(await ui.readUi(page),'prompt-secondary'))await ui.click(page,'prompt-secondary');
  await ui.surface(page,'title');
}
async function accountForm(page) {
  await page.waitForFunction(() => {
    const reveal = document.querySelector('form')?.closest('.reveal-in');
    return reveal && getComputedStyle(reveal).opacity === '1';
  });
}
async function register(page) {
  const suffix = Date.now().toString(36);
  account = { username: `Update${suffix}`, email: `update${suffix}@example.invalid`,
    password: `Qa!${crypto.randomBytes(18).toString('base64url')}` };
  fs.writeFileSync(path.join(out, 'private-account.json'), JSON.stringify(account), { mode: 0o600 });
  const catalogResponse = page.waitForResponse(response => new URL(response.url()).pathname
    === `${mount}/api/mods` && response.request().method() === 'GET');
  await ui.click(page,'dark-cloud');
  const catalog = await catalogResponse;
  assert.equal(catalog.status(), 200);
  await catalog.finished();
  await ui.click(page,'cloud-sign-in');
  await page.waitForURL(value => value.pathname === `${mount}/login`);
  await accountForm(page);
  await page.getByRole('link', { name: 'Create an account', exact: true }).click();
  await page.waitForURL(value => value.pathname === `${mount}/register`);
  await accountForm(page);
  for (const [autocomplete, value] of [['username', account.username], ['email', account.email],
    ['new-password', account.password]]) {
    const input = page.locator(`input[autocomplete="${autocomplete}"]`);
    await input.pressSequentially(value, { delay: 10 });
    await input.press('Tab');
    assert.equal(await input.inputValue(), value);
  }
  const registered = page.waitForResponse(response => new URL(response.url()).pathname
    === `${mount}/api/auth/register` && response.request().method() === 'POST');
  await page.getByRole('button', { name: 'Create account', exact: true }).click();
  assert.equal((await registered).status(), 201);
  await page.waitForURL(value => value.pathname === `${mount}/account`);
  await page.waitForLoadState('networkidle');
  await page.getByRole('link', { name: 'Play', exact: true }).first().click();
  await title(page);
}
async function adminRestart() {
  const response = await fetch(new URL('/admin/deployments/restart', adminUrl), {
    method: 'POST', headers: { authorization: `Bearer ${credentials.supervisor}`,
      'content-type': 'application/json', 'X-Solomon-Deployment-Id': publicationId,
      'X-Solomon-Deployment-Sequence': String(publicationSequence) }, body: JSON.stringify({ targetRevision: target }),
  });
  const body = await response.json();
  return { status: response.status, body };
}

(async () => {
  fs.mkdirSync(out, { recursive: true });
  const server = await chromium.launchServer({ headless: true, host: '127.0.0.1',
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=metal', '--ignore-gpu-blocklist'] });
  const browser = await chromium.connect(server.wsEndpoint());
  const page = await browser.newPage({ ignoreHTTPSErrors: true,
    viewport: { width: 1280, height: 720 } });
  page.setDefaultTimeout(30000);
  page.on('pageerror', error => errors.push(redact(error.message)));
  page.on('console', message => { if (message.type() === 'error') errors.push(redact(message.text())); });
  page.on('response', response => {
    const request = response.request(), pathname = new URL(response.url()).pathname;
    if (request.resourceType() === 'document' && pathname === `${mount}/`) documentLoads += 1;
    if (pathname.includes('/api/') || pathname === publicationEndpoint)
      requests.push({ pathname, method: request.method(), status: response.status() });
    if (pathname === `${mount}/api/game/saves/0` && request.method() === 'PUT') {
      events.push({ kind: 'save-response', status: response.status(),
        sha256: hash(request.postDataJSON().document) });
    }
    if (response.status() >= 400) errors.push(`${response.status()} ${pathname}`);
  });
  page.on('requestfailed', request => {
    const row = `${new URL(request.url()).pathname}: ${request.failure()?.errorText}`;
    if (request.failure()?.errorText === 'net::ERR_ABORTED'
      && new URL(request.url()).pathname === publicationEndpoint)
      cancelled.push(row);
    else errors.push(redact(row));
  });
  page.on('websocket', socket => {
    const readMessage = createNativeMessageReader();
    socket.on('close', () => { socketCloses += 1; events.push({ kind: 'socket-close' }); });
    socket.on('framesent', ({ payload }) => {
      const message = JSON.parse(payload.toString());
      if (message.type === 'hosted' && message.command.kind === 'deployment_ready')
        events.push({ kind: 'deployment-ready', sequence: message.command.checkpoint_sequence,
          target: message.command.target_revision });
    });
    socket.on('framereceived', ({ payload }) => {
      const message = readMessage(JSON.parse(payload.toString()));
      if (!message) return;
      if (message.type === 'welcome') {
        playerId = message.credentials.player_id;
        frame = message.frame;
        welcomed.push({ id: playerId, name: frame.snapshot.participants[playerId].player.selection.name,
          kind: message.hosted.session_kind });
      } else if (message.type === 'snapshot') frame = message.frame;
      else if (message.type === 'error') errors.push(`host ${message.code}: ${message.reason}`);
      else if (message.type === 'hosted' && message.message.kind === 'save_checkpoint') {
        checkpoints.push({ sequence: message.message.sequence, sha256: hash(message.message.save) });
      } else if (message.type === 'hosted' && message.message.kind === 'deployment_restart') {
        announcement = message.message;
        events.push({ kind: 'deployment-announced', sequence: announcement.checkpoint_sequence,
          target: announcement.target_revision, publication: announcement.deployment_id });
      }
    });
  });
  await page.route(`**${mount}/api/game/saves/0`, async route => {
    if (armSave && route.request().method() === 'PUT') {
      heldRoute = { sha256: hash(route.request().postDataJSON().document) };
      events.push({ kind: 'save-held', ...heldRoute });
      await saveReleased;
    }
    await route.continue();
  });
  let changedManifest = false;
  try {
    await page.goto(url.href, { waitUntil: 'load' });
    await title(page);
    await register(page);
    assert.equal(await page.locator('meta[name="solomon-build-revision"]').getAttribute('content'), current);
    await ui.click(page,'play');await ui.click(page,'new-game');
    await ui.waitForUi(page,snapshot=>snapshot.surface==='create'||ui.findWidget(snapshot,'college-intro')||ui.findWidget(snapshot,'panel:dialogue'),'ordinary fresh College admission',{timeout:120000});
    if((await ui.readUi(page)).surface!=='create')await reachCollegeLoadout(page);
    await ui.fill(page,'create-name','UpdateQA');await ui.click(page,'element:fire');await ui.click(page,'discipline:arcane');
    await until(() => welcomed.length === 1
      && frame?.snapshot.participants[playerId]?.player.selection.name === 'UpdateQA', 'real College selection');
    const wizardName = frame.snapshot.participants[playerId].player.selection.name;
    await ui.click(page, (await ui.byLabel(page, 'Enter the Boneyard', {role:'button'})).id);
    await until(() => frame?.snapshot.boneyard && !frame.host.loading_run_id
      && !frame.host.resume_grace && !frame.host.pause && frame.snapshot.ticks > 100,
      'actual Boneyard entry and authority ticks');
    const runId = frame.snapshot.boneyard.run_id;
    const savedPlayerId = playerId;
    checked('registered account enters a real Boneyard on the current published game');
    const loads = documentLoads, closes = socketCloses;
    armSave = true;
    const restart = adminRestart();
    await until(() => announcement && heldRoute, 'real deployment announcement and held save PUT');
    assert.equal(announcement.target_revision, target);
    assert.equal(announcement.deployment_id, publicationId);
    assert.equal(announcement.deployment_sequence, publicationSequence);
    assert.ok(announcement.checkpoint_sequence > 0);
    assert.match((await ui.widget(page,'deployment/update')).label,/Saving your game before the update/);
    await ui.assertGpuOnly(page);
    await sleep(1000);
    assert.equal(events.filter(row => row.kind === 'deployment-ready').length, 0);
    assert.equal(socketCloses, closes);
    assert.equal(documentLoads, loads);
    await page.screenshot({ path: path.join(out, 'saving.png') });
    checked('the update waits for the actual held account save before acknowledging or closing');
    armSave = false;
    releaseSave();
    await until(() => events.some(row => row.kind === 'deployment-ready'), 'saved deployment ACK');
    const result = await restart;
    assert.equal(result.status, 200);
    assert.equal(result.body.deploymentId, publicationId);
    assert.equal(result.body.deploymentSequence, publicationSequence);
    await until(() => socketCloses === closes + 1, 'server-owned deployment socket close');
    assert.match((await ui.widget(page,'deployment/update')).label,/Your game is saved/);
    const ack = events.findIndex(row => row.kind === 'deployment-ready');
    assert.ok(events.slice(0, ack).some(row => row.kind === 'save-response'
      && row.status === 200 && row.sha256 === heldRoute.sha256));
    assert.equal(events[ack].sequence, announcement.checkpoint_sequence);
    await page.screenshot({ path: path.join(out, 'saved.png') });
    checked('the committed save precedes the exact ACK and the update screen survives socket closure');
    const wrong = '1d682fa13c9427a056691238e4051992451f6f96';
    assert.notEqual(wrong, current);
    assert.notEqual(wrong, target);
    const candidate = {revision: targetWebsite, gameRevision: target,
      deploymentId: publicationId, deploymentSequence: publicationSequence, ready: true};
    for (const [label, publication] of [
      ['the old healthy publication', originalPublication],
      ['a mismatching target game', {...candidate, gameRevision: wrong}],
      ['an older publication sequence', {...candidate, deploymentSequence: publicationSequence - 1}],
      ['the pending replacement', {...candidate, ready: false}],
    ]) {
      publish(publication);
      changedManifest = true;
      const polls = requests.filter(row => row.pathname === publicationEndpoint).length;
      await until(() => requests.filter(row => row.pathname === publicationEndpoint).length >= polls + 3,
        `three actual polls of ${label}`);
      assert.equal(documentLoads, loads);
      checked(`${label} cannot authorize an active-session reload`);
    }
    publish(originalPublication);
    changedManifest = false;
    fs.writeFileSync(path.join(out, 'ready-to-publish.json'), JSON.stringify({current, target,
      publication: candidate, wizardName, runId, checkpointSequence: announcement.checkpoint_sequence, result}));
    console.log('READY_TO_PUBLISH');
    await until(() => documentLoads > loads, 'coordinator publication and automatic reload', 180000);
    await title(page);
    assert.equal(await page.locator('meta[name="solomon-build-revision"]').getAttribute('content'), target);
    await ui.click(page,'play');
    await ui.click(page,'continue');
    await until(() => welcomed.length === 2, 'normal Continue after actual replacement');
    assert.equal(welcomed[1].name, wizardName);
    assert.equal(welcomed[1].id, savedPlayerId);
    await until(() => frame.snapshot.boneyard?.run_id === runId, 'the saved signed run continuation');
    await page.screenshot({ path: path.join(out, 'continued.png') });
    checked('the actual target artifact reloads automatically and Continue restores the saved wizard');
    assert.deepEqual(errors, []);
    fs.writeFileSync(path.join(out, 'receipt.json'), JSON.stringify({ passed: true, current, target,
      checks, events, requests, welcomed, checkpoints, cancelled, errors }, null, 2));
  } catch (error) {
    await page.screenshot({ path: path.join(out, 'failure.png') });
    fs.writeFileSync(path.join(out, 'failure.json'), JSON.stringify({ error: redact(error.stack),
      checks, events, requests, welcomed, checkpoints, announcement, cancelled, errors,
      ui: await ui.readUi(page) }, null, 2));
    throw error;
  } finally {
    releaseSave();
    if (changedManifest) publish(originalPublication);
    let timer;
    try { await Promise.race([server.close(), new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error('Chrome close timed out')), 15000);
    })]); } catch { await server.kill(); } finally { clearTimeout(timer); }
  }
})().catch(error => { console.error(redact(error.stack)); process.exitCode = 1; });
