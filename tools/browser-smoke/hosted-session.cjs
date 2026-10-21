// Real Website admission and durable guest continuation through the Rust UI.
// Socket hooks observe authority; they never send gameplay or edit saved state.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));

const output = path.resolve(process.env.OUT || '/tmp');
const url = process.env.URL || 'http://127.0.0.1:4181/';
const checks = [];
const errors = [];
const cancelledMedia = [];
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
  page.setDefaultTimeout(30000);
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
  page.on('requestfailed', request => {
    const receipt = `${new URL(request.url()).pathname}: ${request.failure()?.errorText}`;
    if (request.resourceType() === 'media' && request.failure()?.errorText === 'net::ERR_ABORTED') cancelledMedia.push(receipt);
    else errors.push(receipt);
  });
  page.on('websocket', socket => {
    socket.on('framereceived', ({payload}) => {
      const message = JSON.parse(payload.toString());
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
    await page.goto(url, {waitUntil: 'load'});
    await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0
      && document.querySelector('#status').textContent === '', null, {timeout: 60000});
    // MainMenuScene's tutorial decline clears collegeIntroPending for this new wizard.
    await page.locator('.title-prompt:not([hidden]) [data-menu-action="prompt-secondary"]').click();
    await page.locator('[data-menu-action="play"]').click();
    await page.locator('[data-menu-action="new-game"]').click();
    await page.locator('.game-menus[data-menu-screen="create"]').waitFor();
    await page.getByRole('textbox', {name: 'Wizard name', exact: true}).fill('Felix');
    await page.locator('[data-menu-action="element:fire"]').click();
    await page.locator('[data-menu-action="discipline:arcane"]').click();
    await waitUntil(() => state.id && state.welcome, 'real hosted admission');
    await page.locator('.game-menus[data-menu-screen="hidden"]').waitFor({state: 'attached'});
    checks.push('tutorial decline, final character selection and real guest College admission');
    console.log(checks.at(-1));
    const position = () => state.frame.snapshot.participants[state.id].wizard.position;
    const previous = {...position()};
    await page.keyboard.down('KeyA');
    try { await waitUntil(() => Math.hypot(position().x - previous.x, position().y - previous.y) > 35, 'accepted movement'); }
    finally { await page.keyboard.up('KeyA'); }
    console.log('accepted movement observed');
    await waitUntil(() => state.pongs > 0, 'actual ping response');
    await page.waitForFunction(() => [...document.querySelectorAll('[data-mobile-ui-element="diagnostics"]')]
      .some(node => /\d+ FPS/.test(node.textContent) && /\d+ ms/.test(node.textContent)));
    checks.push('accepted keyboard movement and measured FPS/ping');
    console.log(checks.at(-1));
    await page.waitForFunction(() => document.querySelector('#status').textContent === '');
    await page.screenshot({path: path.join(output, 'hosted-college.png')});
    await page.getByRole('button', {name: 'Enter the Boneyard', exact: true}).click();
    await waitUntil(() => state.frame?.snapshot.boneyard && !state.frame.host.loading_run_id
      && !state.frame.host.resume_grace && !state.frame.host.pause, 'hosted Boneyard renderer readiness', 120000);
    const runId = state.frame.snapshot.boneyard.run_id;
    assert.equal(state.loaded.run_id, runId);
    assert.equal(state.loaded.geometry_sha256, state.frame.snapshot.boneyard.geometry_sha256);
    checks.push('actual hosted Boneyard transition and renderer readiness');
    console.log(checks.at(-1));
    await page.keyboard.press('Escape');
    await page.locator('.game-menus[data-menu-screen="pause"]').waitFor();
    await page.screenshot({path: path.join(output, 'hosted-boneyard-pause.png')});
    await page.locator('[data-menu-action="leave"]').click();
    await page.locator('.game-menus[data-menu-screen="title"]').waitFor({timeout: 30000});
    assert.ok(state.leaveReceipts > 0, 'the actual host acknowledged save-before-leave');
    checks.push('save-before-leave reaches title after the local persistence receipt');
    console.log(checks.at(-1));
    if (process.env.ARCHIVE) {
      await page.locator('[data-menu-action="settings"]').click();
      await page.locator('[data-menu-action="settings-page:cloud"]').click();
      await page.locator('[data-menu-action="settings-page:save-transfer"]').click();
      const warnings = [];
      const acceptWarning = async dialog => {warnings.push(dialog.message()); await dialog.accept();};
      page.on('dialog', acceptWarning);
      const downloading = page.waitForEvent('download');
      await page.locator('[data-save-transfer-action="export"]').click();
      const download = await downloading;
      await download.saveAs(process.env.ARCHIVE);
      page.off('dialog', acceptWarning);
      const bytes = fs.readFileSync(process.env.ARCHIVE);
      assert.equal(bytes.subarray(0, 2).toString(), 'PK');
      assert.ok(bytes.length > 1024);
      fs.chmodSync(process.env.ARCHIVE, 0o600);
      fs.writeFileSync(path.join(output, 'archive-receipt.json'), JSON.stringify({
        runId, displayName: 'Felix', filename: download.suggestedFilename(), bytes: bytes.length,
        sha256: require('node:crypto').createHash('sha256').update(bytes).digest('hex'),
        warnings,
      }, null, 2));
      await page.screenshot({path: path.join(output, 'native-save-export.png')});
      checks.push('actual save archive download from the title Settings transfer page');
      console.log(checks.at(-1));
    }
    state.id = null;
    state.frame = null;
    state.welcome = null;
    state.loaded = null;
    await page.reload({waitUntil: 'load'});
    await page.waitForFunction(() => document.querySelector('#status')?.textContent === '', null, {timeout:60000});
    await page.locator('[data-menu-action="play"]').click();
    await page.locator('[data-menu-action="continue"]').click();
    await waitUntil(() => state.welcome && state.frame?.snapshot.boneyard?.run_id === runId
      && !state.frame.host.loading_run_id && !state.frame.host.resume_grace, 'saved hosted Boneyard continuation', 120000);
    await page.locator('.game-menus[data-menu-screen="hidden"]').waitFor({state: 'attached'});
    assert.equal(state.loaded.run_id, runId);
    assert.equal(state.frame.snapshot.participants[state.id].player.selection.name, 'Felix');
    checks.push('browser reload and real saved Boneyard continuation');
    assert.equal(inactiveRejoin.length, 1, 'the ended party falls back through the source inactive-rejoin contract');
    assert.deepEqual(errors, []);
    const receipt = {passed: true, checks, requests, pingResponses: state.pongs,
      checkpoints: state.checkpoints, leaveReceipts: state.leaveReceipts, inactiveRejoin, errors, cancelledMedia};
    fs.writeFileSync(path.join(output, 'hosted-session.json'), JSON.stringify(receipt, null, 2));
    console.log(JSON.stringify(receipt));
  } catch (error) {
    const ui = await page.evaluate(() => ({menu: document.querySelector('.game-menus')?.dataset.menuScreen,
      status: document.querySelector('#status')?.textContent, text: document.body.innerText.slice(-3000)}));
    await page.screenshot({path: path.join(output, 'hosted-session-failure.png')});
    fs.writeFileSync(path.join(output, 'hosted-session-failure.json'), JSON.stringify({error: error.stack,
      checks, errors, cancelledMedia, requests, ui, accepted: {id: state.id, tick: state.frame?.snapshot.ticks,
        wizard: state.frame?.snapshot.participants[state.id]?.wizard, host: state.frame?.host}}, null, 2));
    throw error;
  } finally {
    await browser.close();
  }
})().catch(error => {console.error(error); process.exitCode = 1;});
