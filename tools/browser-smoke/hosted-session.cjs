// Real Website admission and durable guest continuation through the Rust UI.
// Socket hooks observe authority; they never send gameplay or edit saved state.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const gpu = require('./gpu-ui.cjs');
const {startNewGame} = require('./menu-journey.cjs');
const {createNativeMessageReader} = require('./native-wire.cjs');

const output = path.resolve(process.env.OUT || '/tmp');
const url = process.env.URL || 'http://127.0.0.1:4181/';
const checks = [];
const errors = [];
const inactiveRejoin = [];
const requests = [];
const state = {id: null, frame: null, welcome: null, loaded: null, pongs: 0, checkpoints: 0, leaveReceipts: 0};

async function waitUntil(predicate, label, timeout = 30000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for ${label}`);
}

(async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=metal', '--ignore-gpu-blocklist'],
  });
  const page = await browser.newPage({
    ignoreHTTPSErrors: process.env.LOCAL_TLS === '1',
    viewport: {width: 1280, height: 720},
  });
  fs.mkdirSync(output, {recursive:true});
  page.setDefaultTimeout(30000);
  await gpu.observeGpu(page);
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error') return;
    // Source game-bootstrap deliberately falls back after an inactive-party 404.
    if (message.location().url.endsWith('/api/game/rejoin') && message.text().includes('404')) return;
    errors.push(message.text());
  });
  page.on('response', response => {
    const pathname = new URL(response.url()).pathname;
    if (pathname.includes('/api/')) requests.push({path: pathname, status: response.status()});
    if (response.status() === 404 && pathname.endsWith('/api/game/rejoin')) inactiveRejoin.push(pathname);
    else if (response.status() >= 400) errors.push(`${response.status()} ${pathname}`);
  });
  const observed = gpu.watchRequestFailures(page, {errors});
  page.on('websocket', socket => {
    const read = createNativeMessageReader();
    socket.on('framereceived', ({payload}) => {
      const message = read(JSON.parse(payload.toString()));
      if (!message) return;
      if (message.type === 'welcome') {
        state.id = message.credentials.player_id;
        state.frame = message.frame;
        state.welcome = message.hosted;
      } else if (message.type === 'snapshot') state.frame = message.frame;
      else if (message.type === 'boneyard_loaded') state.loaded = message.boneyard;
      else if (message.type === 'pong') state.pongs++;
      else if (message.type === 'error') errors.push(`host ${message.code}: ${message.reason}`);
      else if (message.type === 'hosted' && message.message.kind === 'save_checkpoint') state.checkpoints++;
      else if (message.type === 'hosted' && message.message.kind === 'save_before_leave') state.leaveReceipts++;
    });
  });
  try {
    await observed.ready;
    await page.goto(url, {waitUntil:'domcontentloaded'});
    await gpu.ready(page);
    await gpu.assertGpuOnly(page);
    await startNewGame(page, {name:'Felix', element:'fire'});
    await waitUntil(() => state.id && state.welcome, 'real hosted admission');
    await gpu.surface(page, 'hidden');
    checks.push('tutorial decline, final character selection and real guest College admission');
    console.log(checks.at(-1));
    const position = () => state.frame.snapshot.participants[state.id].wizard.position;
    const previous = {...position()};
    await page.keyboard.down('KeyA');
    try { await waitUntil(() => Math.hypot(position().x - previous.x, position().y - previous.y) > 35, 'accepted movement'); }
    finally { await page.keyboard.up('KeyA'); }
    console.log('accepted movement observed');
    await waitUntil(() => state.pongs > 0, 'actual ping response');
    await gpu.waitForUi(page, state => {
      const label = gpu.findWidget(state, 'hud:diagnostics')?.label || '';
      return /\d+ FPS/.test(label) && /\d+ ms/.test(label);
    }, 'measured GPU HUD FPS and ping');
    checks.push('accepted keyboard movement and measured FPS/ping');
    console.log(checks.at(-1));
    await gpu.noWidget(page, 'runtime:status');
    await page.screenshot({path: path.join(output, 'hosted-college.png')});
    await gpu.click(page, 'hud:map');
    await waitUntil(() => state.frame?.snapshot.boneyard && !state.frame.host.loading_run_id
      && !state.frame.host.resume_grace && !state.frame.host.pause, 'hosted Boneyard renderer readiness', 120000);
    const runId = state.frame.snapshot.boneyard.run_id;
    assert.equal(state.loaded.run_id, runId);
    assert.equal(state.loaded.geometry_sha256, state.frame.snapshot.boneyard.geometry_sha256);
    checks.push('actual hosted Boneyard transition and renderer readiness');
    console.log(checks.at(-1));
    await page.keyboard.press('Escape');
    await gpu.surface(page, 'pause');
    await page.screenshot({path: path.join(output, 'hosted-boneyard-pause.png')});
    await gpu.click(page, 'leave');
    await gpu.surface(page, 'title', {timeout:30000});
    assert.ok(state.leaveReceipts > 0, 'the actual host acknowledged save-before-leave');
    checks.push('save-before-leave reaches title after the local persistence receipt');
    console.log(checks.at(-1));
    if (process.env.ARCHIVE) {
      await gpu.click(page, 'settings');
      await gpu.click(page, 'settings-page:cloud');
      await gpu.click(page, 'settings-page:save-transfer');
      const warnings = [];
      const acceptWarning = async dialog => {warnings.push(dialog.message()); await dialog.accept();};
      page.on('dialog', acceptWarning);
      const downloading = page.waitForEvent('download');
      await gpu.click(page, 'save-export');
      const exported = await Promise.race([
        downloading.then(download => ({download})),
        gpu.widget(page, 'save-export-confirm').then(() => ({confirmation:true})),
      ]);
      let exportWarningUi = null;
      if (exported.confirmation) {
        exportWarningUi = await gpu.readUi(page);
        await gpu.click(page, 'save-export-confirm');
      }
      const download = exported.download || await downloading;
      await download.saveAs(process.env.ARCHIVE);
      page.off('dialog', acceptWarning);
      const bytes = fs.readFileSync(process.env.ARCHIVE);
      assert.equal(bytes.subarray(0, 2).toString(), 'PK');
      assert.ok(bytes.length > 1024);
      fs.chmodSync(process.env.ARCHIVE, 0o600);
      fs.writeFileSync(path.join(output, 'archive-receipt.json'), JSON.stringify({
        runId, displayName: 'Felix', filename: download.suggestedFilename(), bytes: bytes.length,
        sha256: require('node:crypto').createHash('sha256').update(bytes).digest('hex'),
        warnings, exportWarningUi,
      }, null, 2));
      await page.screenshot({path: path.join(output, 'native-save-export.png')});
      checks.push('actual save archive download from the title Settings transfer page');
      console.log(checks.at(-1));
    }
    state.id = null;
    state.frame = null;
    state.welcome = null;
    state.loaded = null;
    await gpu.observeDocumentReplacement(page, () => page.reload({waitUntil: 'load'}));
    await gpu.ready(page);
    await gpu.click(page, 'play');
    await gpu.click(page, 'continue');
    await waitUntil(() => state.welcome && state.frame?.snapshot.boneyard?.run_id === runId
      && !state.frame.host.loading_run_id && !state.frame.host.resume_grace, 'saved hosted Boneyard continuation', 120000);
    await gpu.surface(page, 'hidden');
    assert.equal(state.loaded.run_id, runId);
    assert.equal(state.frame.snapshot.participants[state.id].player.selection.name, 'Felix');
    checks.push('browser reload and real saved Boneyard continuation');
    assert.equal(inactiveRejoin.length, 1, 'the ended party falls back through the source inactive-rejoin contract');
    await gpu.settleErrors(page);
    assert.deepEqual(errors, []);
    const receipt = {passed: true, checks, requests, pingResponses: state.pongs,
      checkpoints: state.checkpoints, leaveReceipts: state.leaveReceipts, inactiveRejoin, errors, gpu:await gpu.assertGpuOnly(page), submissions:await gpu.gpuDraws(page)};
    fs.writeFileSync(path.join(output, 'hosted-session.json'), JSON.stringify(receipt, null, 2));
    console.log(JSON.stringify(receipt));
  } catch (error) {
    await gpu.settleErrors(page);
    const ui = await gpu.readUi(page);
    await page.screenshot({path: path.join(output, 'hosted-session-failure.png')});
    fs.writeFileSync(path.join(output, 'hosted-session-failure.json'), JSON.stringify({error: error.stack,
      checks, errors, requests, ui, accepted: {id: state.id, tick: state.frame?.snapshot.ticks,
        wizard: state.frame?.snapshot.participants[state.id]?.wizard, host: state.frame?.host}}, null, 2));
    throw error;
  } finally {
    await browser.close();
  }
})().catch(error => {console.error(error); process.exitCode = 1;});
