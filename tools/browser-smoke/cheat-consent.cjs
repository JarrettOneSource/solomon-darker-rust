// MainMenuScene settings and private-College consent through ordinary browser UI.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const gpu = require('./gpu-ui.cjs');
const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
const {startNewGame} = require('./menu-journey.cjs');
const {revealSettingsControl} = require('./settings-journey.cjs');
const {createNativeMessageReader} = require('./native-wire.cjs');
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
  const owned = await launchOwnedBrowser(chromium, {headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
  const {browser} = owned;
  const page = await browser.newPage({ignoreHTTPSErrors: process.env.LOCAL_TLS === '1',
    viewport: {width: 1280, height: 720}});
  page.setDefaultTimeout(30000);
  await gpu.observeGpu(page);
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => {if (message.type() === 'error') errors.push(message.text());});
  page.on('response', response => {
    const pathname = new URL(response.url()).pathname;
    if (pathname.includes('/api/')) requests.push({path: pathname, status: response.status()});
    if (response.status() >= 400) errors.push(`${response.status()} ${pathname}`);
  });
  const observed = gpu.watchRequestFailures(page, {errors});
  page.on('websocket', socket => {
    const read = createNativeMessageReader();
    socket.on('framesent', ({payload}) => {
      const message = JSON.parse(payload.toString());
      if (message.type === 'hosted' && message.command.kind === 'set_cheats')
        commands.push({enabled: message.command.enabled});
      if (message.type === 'save_before_leave') leaveRequests.push(message.request_id);
    });
    socket.on('framereceived', ({payload}) => {
      const message = read(JSON.parse(payload.toString()));
      if (!message) return;
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
  let settingsKey;
  const settings = () => page.evaluate(key => JSON.parse(localStorage.getItem(key)), settingsKey);
  const cloudSettings = async () => {
    const state = await gpu.waitForUi(page, state => ['title','pause'].includes(state.surface),
      'Title or Pause settings entry');
    await gpu.click(page, state.surface === 'pause' ? 'pause-settings' : 'settings');
    await gpu.click(page, 'settings-page:cloud');
  };
  const cheats = () => gpu.widget(page, 'settings-toggle:cheats');
  try {
    await observed.ready;
    await page.goto(process.env.URL, {waitUntil:'domcontentloaded'});
    await gpu.ready(page);
    await gpu.assertGpuOnly(page);
    settingsKey = await page.evaluate(() => {
      const mount = document.querySelector('meta[name="solomon-website-mount"]')?.content || '';
      return `solomon-dark-game-settings-v1${mount ? `:${mount}` : ''}`;
    });
    if (gpu.findWidget(await gpu.readUi(page), 'prompt-secondary')) {
      await gpu.click(page, 'prompt-secondary');
      await gpu.noWidget(page, 'prompt-secondary');
    }
    await cloudSettings();
    const activity = () => gpu.widget(page, 'settings-toggle:activity');
    const initiallyEnabled = (await activity()).selected;
    await gpu.click(page, 'settings-toggle:activity');
    await page.waitForFunction(({key, expected}) =>
      JSON.parse(localStorage.getItem(key))?.enableActivityMessages === expected,
    {key: settingsKey, expected: !initiallyEnabled});
    await gpu.observeDocumentReplacement(page, () => page.reload({waitUntil: 'load'}));
    await gpu.ready(page);
    await cloudSettings();
    assert.equal((await activity()).selected, !initiallyEnabled);
    assert.equal((await settings()).enableCheats, false);
    checks.push('a changed title preference commits and survives a real reload');
    await gpu.click(page, 'settings-back');
    await gpu.click(page, 'settings-back');
    await gpu.surface(page, 'title');
    await startNewGame(page, {name:'Consent', element:'fire'});
    await until(() => welcomes.length === 1 && frame?.snapshot.participants[playerId], 'actual Global Hub admission');
    assert.equal(welcomes[0].kind, 'GlobalHub');
    await gpu.surface(page, 'hidden');
    await page.keyboard.press('Escape');
    await cloudSettings();
    await revealSettingsControl(page, 'settings-toggle:cheats');
    const beforePrompt = commands.length;
    await gpu.click(page, 'settings-toggle:cheats');
    await gpu.widget(page, 'routing/dialog');
    assert.equal((await gpu.widget(page, 'routing/dialog')).label, 'CHEATS USE PRIVATE COLLEGES');
    assert.equal((await settings()).enableCheats, false);
    assert.equal(commands.length, beforePrompt);
    await gpu.click(page, 'routing-back');
    await gpu.noWidget(page, 'routing/dialog');
    assert.equal((await cheats()).selected, false);
    assert.equal((await settings()).enableCheats, false);
    checks.push('Global cheat request waits for consent and Cancel preserves accepted preferences');
    await gpu.click(page, 'settings-toggle:cheats');
    await gpu.widget(page, 'routing/dialog');
    await page.screenshot({path: path.join(output, 'global-cheat-consent.png')});
    await gpu.click(page, 'routing-continue');
    await gpu.surface(page, 'title');
    assert.equal((await settings()).enableCheats, true);
    await gpu.settleErrors(page);
    assert.deepEqual(errors, []);
    assert.equal(commands.length, beforePrompt, 'Global College must never receive enabled cheats');
    assert.equal(leaveRequests.length, 1);
    assert.deepEqual(leaveReceipts, leaveRequests);
    checks.push('consent commits the preference and leaves through the real save receipt');
    await gpu.click(page, 'play');
    await gpu.click(page, 'continue');
    await until(() => welcomes.length === 2 && welcomes[1].kind === 'PrivateCollege', 'private College continuation');
    assert.equal(welcomes[1].cheats, true);
    assert.equal(frame.snapshot.participants[playerId].player.selection.name, 'Consent');
    checks.push('Last Game continues the same wizard in an actual private College with cheats enabled');
    await page.screenshot({path: path.join(output, 'private-cheat-college.png')});
    await gpu.settleErrors(page);
    assert.deepEqual(errors, []);
    const receipt = {passed:true, checks, errors, requests, commands, welcomes, leaveRequests, leaveReceipts, gpu:await gpu.assertGpuOnly(page), submissions:await gpu.gpuDraws(page)};
    fs.writeFileSync(path.join(output, 'cheat-consent.json'), JSON.stringify(receipt, null, 2));
    console.log(JSON.stringify(receipt));
  } catch (error) {
    await gpu.settleErrors(page);
    await page.screenshot({path: path.join(output, 'cheat-consent-failure.png')});
    fs.writeFileSync(path.join(output, 'cheat-consent-failure.json'), JSON.stringify({
      error: error.stack, checks, errors, requests, commands, welcomes, leaveRequests, leaveReceipts,
      ui: await gpu.readUi(page),
    }, null, 2));
    throw error;
  } finally {
    try {
      fs.writeFileSync(path.join(output, 'browser-cleanup.json'),
        JSON.stringify(await owned.close(), null, 2));
    } catch (error) {
      fs.writeFileSync(path.join(output, 'browser-cleanup-failure.json'),
        JSON.stringify(error.cleanup || {error: error.stack}, null, 2));
      throw error;
    }
  }
})().catch(error => {console.error(error); process.exitCode = 1;});
