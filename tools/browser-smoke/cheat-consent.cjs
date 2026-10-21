// MainMenuScene settings and private-College consent through ordinary browser UI.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const output = path.resolve(process.env.OUT);
const checks = [], errors = [], requests = [], commands = [], welcomes = [];
const leaveRequests = [], leaveReceipts = [];
let frame, playerId;

async function until(predicate, label) {
  const deadline = Date.now() + 30000;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for ${label}`);
}

(async () => {
  fs.mkdirSync(output, {recursive: true});
  const server = await chromium.launchServer({headless: true, host: '127.0.0.1',
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
  const browser = await chromium.connect(server.wsEndpoint());
  const page = await browser.newPage({ignoreHTTPSErrors: process.env.LOCAL_TLS === '1',
    viewport: {width: 1280, height: 720}});
  page.setDefaultTimeout(30000);
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => {if (message.type() === 'error') errors.push(message.text());});
  page.on('response', response => {
    const pathname = new URL(response.url()).pathname;
    if (pathname.includes('/api/')) requests.push({path: pathname, status: response.status()});
    if (response.status() >= 400) errors.push(`${response.status()} ${pathname}`);
  });
  page.on('requestfailed', request => {
    if (!(request.resourceType() === 'media' && request.failure()?.errorText === 'net::ERR_ABORTED'))
      errors.push(`${new URL(request.url()).pathname}: ${request.failure()?.errorText}`);
  });
  page.on('websocket', socket => {
    socket.on('framesent', ({payload}) => {
      const message = JSON.parse(payload.toString());
      if (message.type === 'hosted' && message.command.kind === 'set_cheats')
        commands.push({enabled: message.command.enabled});
      if (message.type === 'save_before_leave') leaveRequests.push(message.request_id);
    });
    socket.on('framereceived', ({payload}) => {
      const message = JSON.parse(payload.toString());
      if (message.type === 'welcome') {
        frame = message.frame;
        playerId = message.credentials.player_id;
        welcomes.push({kind: message.hosted.session_kind, cheats: message.hosted.cheats_enabled});
      } else if (message.type === 'snapshot') frame = message.frame;
      else if (message.type === 'error') errors.push(`host ${message.code}: ${message.reason}`);
      else if (message.type === 'hosted' && message.message.kind === 'save_before_leave')
        leaveReceipts.push(message.message.request_id);
    });
  });
  const settings = () => page.evaluate(() => JSON.parse(
    localStorage.getItem('solomon-darker-rust-settings-v1:/game-stg')));
  const cloudSettings = async () => {
    await page.locator('[data-menu-action="settings"]:visible, [data-menu-action="pause-settings"]:visible').click();
    await page.locator('[data-menu-action="settings-page:cloud"]').click();
  };
  const cheats = () => page.locator('[data-menu-action="settings-toggle:cheats"]');
  try {
    await page.goto(process.env.URL, {waitUntil: 'load'});
    await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0
      && document.querySelector('#status')?.textContent === '');
    await page.locator('.title-prompt:not([hidden]) [data-menu-action="prompt-secondary"]').click();
    await cloudSettings();
    const activity = page.locator('[data-menu-action="settings-toggle:activity"]');
    const initiallyEnabled = (await activity.getAttribute('aria-pressed')) === 'true';
    await activity.click();
    await page.waitForFunction(expected => JSON.parse(localStorage.getItem(
      'solomon-darker-rust-settings-v1:/game-stg'))?.enable_activity_messages === expected,
    !initiallyEnabled);
    await page.reload({waitUntil: 'load'});
    await page.waitForFunction(() => document.querySelector('#status')?.textContent === '');
    await cloudSettings();
    assert.equal(await activity.getAttribute('aria-pressed'), String(!initiallyEnabled));
    assert.equal((await settings()).enable_cheats, false);
    checks.push('a changed title preference commits and survives a real reload');
    await page.locator('[data-menu-action="settings-back"]').click();
    await page.locator('[data-menu-action="settings-back"]').click();
    await page.locator('[data-menu-action="play"]').click();
    await page.locator('[data-menu-action="new-game"]').click();
    await page.getByRole('textbox', {name: 'Wizard name', exact: true}).fill('Consent');
    await page.locator('[data-menu-action="element:fire"]').click();
    await page.locator('[data-menu-action="discipline:arcane"]').click();
    await until(() => welcomes.length === 1 && frame?.snapshot.participants[playerId], 'actual Global Hub admission');
    assert.equal(welcomes[0].kind, 'GlobalHub');
    await page.locator('.game-menus[data-menu-screen="hidden"]').waitFor({state: 'attached'});
    await page.keyboard.press('Escape');
    await cloudSettings();
    const beforePrompt = commands.length;
    await cheats().click();
    const consent = page.getByRole('dialog', {name: 'Cheats require a private College', exact: true});
    await consent.waitFor();
    assert.equal((await settings()).enable_cheats, false);
    assert.equal(commands.length, beforePrompt);
    await consent.getByRole('button', {name: 'CANCEL', exact: true}).click();
    await consent.waitFor({state: 'hidden'});
    assert.equal(await cheats().getAttribute('aria-pressed'), 'false');
    assert.equal((await settings()).enable_cheats, false);
    checks.push('Global cheat request waits for consent and Cancel preserves accepted preferences');
    await cheats().click();
    await consent.waitFor();
    await page.screenshot({path: path.join(output, 'global-cheat-consent.png')});
    await consent.getByRole('button', {name: 'LEAVE & CONTINUE', exact: true}).click();
    await page.locator('.game-menus[data-menu-screen="title"]').waitFor();
    assert.equal((await settings()).enable_cheats, true);
    assert.deepEqual(errors, []);
    assert.equal(commands.length, beforePrompt, 'Global College must never receive enabled cheats');
    assert.equal(leaveRequests.length, 1);
    assert.deepEqual(leaveReceipts, leaveRequests);
    checks.push('consent commits the preference and leaves through the real save receipt');
    await page.locator('[data-menu-action="play"]').click();
    await page.locator('[data-menu-action="continue"]').click();
    await until(() => welcomes.length === 2 && welcomes[1].kind === 'PrivateCollege', 'private College continuation');
    assert.equal(welcomes[1].cheats, true);
    assert.equal(frame.snapshot.participants[playerId].player.selection.name, 'Consent');
    checks.push('Last Game continues the same wizard in an actual private College with cheats enabled');
    await page.screenshot({path: path.join(output, 'private-cheat-college.png')});
    assert.deepEqual(errors, []);
    const receipt = {passed: true, checks, errors, requests, commands, welcomes, leaveRequests, leaveReceipts};
    fs.writeFileSync(path.join(output, 'cheat-consent.json'), JSON.stringify(receipt, null, 2));
    console.log(JSON.stringify(receipt));
  } catch (error) {
    await page.screenshot({path: path.join(output, 'cheat-consent-failure.png')});
    fs.writeFileSync(path.join(output, 'cheat-consent-failure.json'), JSON.stringify({
      error: error.stack, checks, errors, requests, commands, welcomes, leaveRequests, leaveReceipts,
      ui: await page.evaluate(() => ({status: document.querySelector('#status')?.textContent,
        menu: document.querySelector('.game-menus')?.dataset.menuScreen,
        text: document.body.innerText.slice(-2500)})),
    }, null, 2));
    throw error;
  } finally {
    let timeout;
    try {
      await Promise.race([server.close(), new Promise((resolve, reject) => {
        timeout = setTimeout(() => reject(new Error('Chrome close timed out')), 15000);
      })]);
    } catch {
      // This server owns only the browser started by this harness.
      await server.kill();
    } finally {clearTimeout(timeout);}
  }
})().catch(error => {console.error(error); process.exitCode = 1;});
