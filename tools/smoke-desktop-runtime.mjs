// Real packaged Electron -> native authority -> WASM acceptance. Public UI drives
// gameplay; sockets only observe accepted state. The failed-close case advances
// only the real IndexedDB record revision to represent a concurrent writer.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { once } from 'node:events';
import { createReadStream } from 'node:fs';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { _electron as electron } from 'playwright-core';
const require = createRequire(import.meta.url);
const {
  startNewGame
} = require('./browser-smoke/menu-journey.cjs');
const {createNativeMessageReader} = require('./browser-smoke/native-wire.cjs');
const {observeNativeChildren} = require('./browser-smoke/native-child.cjs');
const {redact, sanitizeError} = require('./browser-smoke/desktop-failure.cjs');
const ui = require('./browser-smoke/gpu-ui.cjs');
const applicationPath = resolve(process.env.SDR_DESKTOP_APP || `dist-desktop/Solomon Darker-${process.platform}-${process.arch}`);
const manifest = JSON.parse(await readFile(join(applicationPath, 'desktop-package-manifest.json'), 'utf8'));
const executable = resolve(applicationPath, manifest.executable);
const applicationResources = join(applicationPath, manifest.platform === 'darwin'
  ? 'Solomon Darker.app/Contents/Resources/app' : 'resources/app');
const authorityHash = createHash('sha256');
for await (const bytes of createReadStream(join(applicationResources, manifest.nativeAuthority.executable))) authorityHash.update(bytes);
assert.equal(authorityHash.digest('hex'), manifest.nativeAuthority.sha256,
  'the package manifest identifies the actual signed native authority bytes');
const evidence = resolve(process.env.SDR_DESKTOP_EVIDENCE || 'reports/desktop-smoke');
await mkdir(evidence, {
  recursive: true
});
const profiles = await mkdtemp(join(tmpdir(), 'solomon-darker-smoke-'));
await writeFile(join(evidence, 'owned-profile.json'), JSON.stringify({profiles}));
const running = new Set();
const errors = [];
const resourceChecks = [];
const receipts = { platform: process.platform, revision: manifest.revision, clientStartups: [], cancelledMusic: [], cancelledMenuImages: [], storageWarnings: [], serviceStops: [], resourceFailures: [], mediaObservations: [] };
const storageConflict = 'The saved game changed in another tab. Reload to continue that save.';
const position = instance => instance.state.frame?.snapshot.participants[instance.state.id]?.wizard.position;
const playing = instance => instance.state.frame?.snapshot.boneyard && !instance.state.frame.host.loading_run_id && !instance.state.frame.host.resume_grace && !instance.state.frame.host.pause;
const rendererReadinessSent = instance => {
  const {frame, readyBoneyards, resumeReadies} = instance.state;
  return frame?.snapshot.boneyard && readyBoneyards.has(frame.snapshot.boneyard.run_id)
    && (!frame.host.resume_grace || resumeReadies.has(frame.host.resume_grace.sequence));
};
try {
  const solo = await launch('solo', join(profiles, 'solo'));
  await solo.page.screenshot({
    path: join(evidence, 'launcher.png')
  });
  receipts.launcher = await proveLauncher(solo);
  await ui.click(solo.page, 'launcher/solo');
  await title(solo.page);
  await ui.click(solo.page,'settings');
  await ui.click(solo.page,'settings-range:music');await ui.focused(solo.page,'settings-range:music');
  await solo.page.keyboard.press('Home');await solo.page.keyboard.press('ArrowRight');await ui.value(solo.page,'settings-range:music','1');
  await ui.click(solo.page,'settings-page:controls');
  await ui.click(solo.page,'settings-toggle:reverse-hotbar');
  await ui.click(solo.page,'settings-back');
  await ui.click(solo.page,'settings-back');
  await enterHub(solo, 'Offline', 'fire');
  const runtime = await solo.page.evaluate(() => ({
    origin: location.origin,
    endpoint: window.solomonDarkRuntime.gameEndpoint
  }));
  assert.equal(runtime.origin, 'solomon-darker://app');
  assert.equal(runtime.endpoint.kind, 'localhost');
  assert.equal(runtime.endpoint.sessionKind, 'standalone');
  const child = await hostProcess(solo.app);
  assert.ok(child.command.includes('--desktop'), 'the app owns the canonical native desktop scope');
  assert.equal(child.command.includes(runtime.endpoint.credential), false);
  const movement = await move(solo);
  await solo.page.screenshot({
    path: join(evidence, 'solo.png')
  });
  const updateStatus = await solo.page.evaluate(() => window.solomonDarkRuntime.desktop.checkUpdates());
  assert.equal(updateStatus.update, null);
  assert.ok(updateStatus.updateError, 'offline update failure does not block play');
  receipts.repeatedClose = await proveRepeatedClose(solo);
  assert.ok(solo.state.leaveReceipts > 0, 'ordinary app close awaited the actual host save receipt');
  await waitForExit(child.pid);
  const resumed = await launch('resume', join(profiles, 'solo'));
  await ui.click(resumed.page, 'launcher/solo');
  await title(resumed.page);
  const preferences = await resumed.page.evaluate(() =>
    JSON.parse(localStorage.getItem('solomon-dark-game-settings-v1')));
  assert.equal(preferences.musicVolumePercent, 1);
  assert.equal(preferences.reverseHotbarCycle, true);
  // Source buttons still activate when the pointer is held before release.
  await ui.click(resumed.page,'settings',{delay:1100});
  await ui.value(resumed.page,'settings-range:music','1');
  await ui.click(resumed.page,'settings-page:controls');
  assert.equal((await ui.widget(resumed.page,'settings-toggle:reverse-hotbar')).selected,true);
  await ui.click(resumed.page,'settings-back');
  await ui.click(resumed.page,'settings-back');
  const saved = await readSave(resumed.page);
  assert.equal(saved.document.schemaVersion, 51);
  const id = saved.document.continuation.summary.playerId;
  const entities = saved.document.continuation.simulation.playerEntities;
  const playerIndex = entities.identities.findIndex(row => row.playerId === id);
  assert.ok(playerIndex >= 0, 'the Source entity columns retain the saved player identity');
  const locomotion = entities.locomotions[playerIndex];
  assert.ok(locomotion.position.x > movement.before + 10);
  await ui.click(resumed.page,'play');
  await ui.click(resumed.page,'continue');
  await readyHub(resumed);
  assert.ok(position(resumed).x > movement.before + 10, 'ordinary Continue restored the committed source document');
  await resumed.page.screenshot({
    path: join(evidence, 'resumed.png')
  });
  receipts.offline = {
    movement,
    savedPosition: locomotion.position,
    resumedX: position(resumed).x,
    origin: runtime.origin,
    nativeHostExited: true,
    sourceSaveSchema: saved.document.schemaVersion
  };
  receipts.updatePrompt = await proveUpdatePrompt(resumed);
  receipts.failedClose = await proveFailedClose(resumed);
  receipts.returnToLauncher = await proveReturnToLauncher();
  const host = await launch('host', join(profiles, 'host'));
  const port = await freePort();
  await ui.fill(host.page, 'launcher/host', '127.0.0.1');
  await ui.fill(host.page, 'launcher/port', String(port));
  await ui.click(host.page, 'launcher/start-host');
  await enterHub(host, 'PeerHost', 'fire');
  const hostChild = await hostProcess(host.app);
  const invitation = await host.app.evaluate(({
    Menu,
    clipboard
  }) => {
    Menu.getApplicationMenu().items.find(row => row.label === 'Game').submenu.items.find(row => row.label === 'Copy multiplayer invite').click();
    return clipboard.readText();
  });
  if (process.platform === 'win32') await setWindowMinimized(host, true);
  const guest = await launch('guest', join(profiles, 'guest'));
  await ui.fill(guest.page, 'launcher/invite', invitation);
  await ui.click(guest.page, 'launcher/join');
  await enterHub(guest, 'PeerGuest', 'air');
  await waitFor(() => [host, guest].every(instance => instance.state.frame?.snapshot.participant_order.length === 2), 'both admitted peers');
  assert.notEqual(host.state.id, guest.state.id);
  assert.equal(host.state.frame.host.authority_player_id, host.state.id);
  assert.equal(guest.state.frame.host.authority_player_id, host.state.id);
  const guestMovement = await move(guest);
  if (process.platform === 'win32') {
    await setWindowMinimized(guest, true);
    await setWindowMinimized(host, false);
  }
  await ui.click(host.page, 'hud:map');
  const readyDeadline = Date.now() + 120000;
  const remainingReadiness = () => {
    const remaining = readyDeadline - Date.now();
    if (remaining <= 0) throw new Error('Timed out waiting for both actual Boneyard renderers');
    return remaining;
  };
  if (process.platform === 'win32') {
    // Keep one inactive view minimized until both genuine readiness messages and
    // accepted states arrive. Restoring both must produce fresh accepted frames
    // before the original readiness assertion, within its same overall limit.
    await waitFor(() => rendererReadinessSent(host),
      'the actual host renderer readiness acknowledgements', remainingReadiness());
    await setWindowMinimized(host, true);
    await setWindowMinimized(guest, false);
    await waitFor(() => rendererReadinessSent(guest),
      'the actual guest renderer readiness acknowledgements', remainingReadiness());
    await waitFor(() => playing(host) && playing(guest),
      'both accepted Boneyard states', remainingReadiness());
    const previousSequences = [host.state.frame.sequence, guest.state.frame.sequence];
    await setWindowMinimized(host, false);
    await waitFor(() => host.state.frame.sequence > previousSequences[0]
      && guest.state.frame.sequence > previousSequences[1],
    'fresh accepted peer frames after restoring both windows', remainingReadiness());
  }
  await waitFor(() => playing(host) && playing(guest), 'both actual Boneyard renderers', remainingReadiness());
  const readiness = {elapsedMs: 120000 - remainingReadiness(),
    hostSequence: host.state.frame.sequence, guestSequence: guest.state.frame.sequence};
  assert.equal(host.state.frame.snapshot.boneyard.run_id, guest.state.frame.snapshot.boneyard.run_id);
  assert.equal(host.state.frame.snapshot.participant_order.length, 2);
  if (process.platform === 'win32') await setWindowMinimized(guest, true);
  await host.page.screenshot({
    path: join(evidence, 'peer-host-boneyard.png')
  });
  if (process.platform === 'win32') {
    await setWindowMinimized(guest, false);
    await setWindowMinimized(host, true);
  }
  await guest.page.screenshot({
    path: join(evidence, 'peer-guest-boneyard.png')
  });
  receipts.peer = {
    hostPlayer: host.state.id,
    guestPlayer: guest.state.id,
    guestMovement,
    readiness,
    sharedBoneyardPlayers: 2
  };
  await close(guest);
  await close(host);
  await waitForExit(hostChild.pid);
  await Promise.all(resourceChecks);
  assert.deepEqual(errors, []);
  await writeFile(join(evidence, 'receipt.json'), `${JSON.stringify({
    status: 'ok',
    ...receipts
  }, null, 2)}\n`);
  console.log(JSON.stringify({
    status: 'ok',
    ...receipts,
    evidence
  }));
} catch (error) {
  const sanitized = sanitizeError(error);
  const failure = {status: 'failed', ...receipts, error: sanitized.stack, errors,
    instances: [...running].map(({label, state}) => ({label, playerId: state.id,
      sequence: state.frame?.sequence, tick: state.frame?.snapshot.ticks,
      loadingRunId: state.frame?.host.loading_run_id, resumeGrace: state.frame?.host.resume_grace,
      boneyardRunId: state.frame?.snapshot.boneyard?.run_id,
      wizard: state.frame?.snapshot.participants[state.id] && {
        position: state.frame.snapshot.participants[state.id].wizard.position,
        region: state.frame.snapshot.participants[state.id].wizard.region,
        transition: state.frame.snapshot.participants[state.id].wizard.transition,
      },
      readyBoneyards: [...state.readyBoneyards], resumeReadies: [...state.resumeReadies],
      closedSockets: state.closedSockets, sentDisconnects: state.disconnects})),
  };
  console.error(JSON.stringify(failure));
  await writeFile(join(evidence, 'failure.json'), `${JSON.stringify(failure, null, 2)}\n`);
  for (const instance of running) {
    await instance.page.screenshot({
      path: join(evidence, `${instance.label}-failure.png`), timeout: 8000
    }).catch(() => {});
    console.error(JSON.stringify({
      label: instance.label,
      url: redact(instance.page.url()),
      ui: await ui.readUi(instance.page).catch(()=>null),
      errors,
      cancelledMenuImages: receipts.cancelledMenuImages
    }));
  }
  throw sanitized;
} finally {
  for (const instance of running) {
    await bounded(instance.app.evaluate(() => {
      const dialog = globalThis.__desktopCloseSmoke;
      if (dialog) {
        dialog.cleanup = true;
        dialog.answer?.({response: 1, checkboxChecked: false});
      }
    }), 5000, `${instance.label} dialog cleanup timed out`).catch(() => {});
    await cleanupApp(instance.app, instance.appPid);
  }
  await bounded(rm(profiles, {
    recursive: true,
    force: true
  }), 5000, 'Desktop profile cleanup timed out');
}
async function launch(label, userData) {
  const stopJournal = join(evidence, `${label}-service-stops.jsonl`);
  await writeFile(stopJournal, '');
  const app = await electron.launch({
    executablePath: executable,
    // Windows' native ANGLE backend avoids the measured SwiftShader CPU stall.
    args: [...(process.platform === 'win32' ? [] : [
      '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--use-angle=swiftshader', '--use-gl=angle'
    ]), '--proxy-server=http://127.0.0.1:9'],
    env: {
      ...process.env,
      SDR_DESKTOP_USER_DATA: userData,
      ELECTRON_DISABLE_SECURITY_WARNINGS: 'true'
    },
    timeout: 30000
  });
  let nativeChildren, appPid, page;
  try {
    nativeChildren = await app.evaluateHandle(observeNativeChildren, stopJournal);
    appPid = await nativeChildren.evaluate(observer => observer.parentPid);
    page = await app.firstWindow({timeout: 30000});
  } catch (error) {
    console.error(redact(error.stack));
    await cleanupApp(app, appPid);
    throw error;
  }
  app.context().setDefaultTimeout(30000);
  const instance = {
    app,
    appPid,
    nativeChildren,
    stopJournal,
    page,
    label,
    state: {
      id: null,
      frame: null,
      leaveReceipts: 0,
      disconnects: 0,
      closedSockets: 0,
      readyBoneyards: new Set(),
      resumeReadies: new Set(),
      expectedStorageConflict: false,
      clientResources: []
    }
  };
  running.add(instance);
  // The shared observer retains every raw failure and only retires one when
  // the actual resident media channel proves the browser suspended preloading.
  await ui.watchRequestFailures(page, {errors}).ready;
  page.on('pageerror', error => errors.push(redact(`${label}: ${error.message}`)));
  page.on('console', message => {
    if (message.type() === 'warning' && message.text() === storageConflict
      && instance.state.expectedStorageConflict) {
      receipts.storageWarnings.push({label, message: message.text()});
      return;
    }
    if (message.type() === 'error' && !message.text().includes('ERR_PROXY_CONNECTION_FAILED')) errors.push(redact(`${label}: ${message.text()}`));
  });
  page.on('response', response => {
    const url = new URL(response.url());
    if (url.protocol !== 'solomon-darker:' || url.host !== 'app'
      || !/\.(?:m?js|wasm)$/.test(url.pathname)) return;
    instance.state.clientResources.push({path: url.pathname, status: response.status()});
  });
  page.on('requestfailed', request => {
    if (request.url().startsWith('solomon-darker://app/')) {
      const failure = {label, at: new Date().toISOString(), url: redact(request.url()),
        document: redact(page.url()), resourceType: request.resourceType(),
        method: request.method(), failure: request.failure()};
      receipts.resourceFailures.push(failure);
      resourceChecks.push(page.evaluate(() => window.__solomonResidentMedia?.() ?? null)
        .then(snapshot => { failure.media = snapshot; },
          error => { failure.observationError = redact(error.message); }));
    }
  });
  page.on('websocket', socket => {
    const readMessage = createNativeMessageReader();
    socket.on('framesent', ({payload}) => {
      const message = JSON.parse(payload.toString());
      if (message.type === 'disconnect') instance.state.disconnects++;
      if (message.type === 'ready_boneyard') instance.state.readyBoneyards.add(message.run_id);
      if (message.type === 'resume_ready') instance.state.resumeReadies.add(message.sequence);
    });
    socket.on('close', () => instance.state.closedSockets++);
    socket.on('framereceived', ({payload}) => {
      const message = readMessage(JSON.parse(payload.toString()));
      if (message === null) return;
      if (message.type === 'welcome') {
        instance.state.id = message.credentials.player_id;
        instance.state.frame = message.frame;
      } else if (message.type === 'snapshot') instance.state.frame = message.frame;else if (message.type === 'hosted' && message.message.kind === 'save_before_leave') instance.state.leaveReceipts++;else if (message.type === 'error') errors.push(redact(`${label} host: ${message.code}: ${message.reason}`));
    });
  });
  if (process.platform === 'win32') await setWindowMinimized(instance, false);
  await ui.ready(page);
  await ui.surface(page, 'desktop-launcher');
  await ui.assertGpuOnly(page);
  return instance;
}
async function proveLauncher(instance) {
  const {page} = instance;
  const initialHost = (await ui.widget(page, 'launcher/host')).value;
  await ui.click(page, 'launcher/join');
  await ui.value(page, 'launcher/status', "Paste your friend's multiplayer invite.");
  await ui.fill(page, 'launcher/invite', 'not-an-invite');
  await ui.click(page, 'launcher/join');
  await ui.value(page, 'launcher/status', 'Invalid multiplayer invite.');
  await ui.widget(page, 'launcher/solo', {enabled: true});
  await ui.value(page, 'launcher/invite', 'not-an-invite');
  assert.equal(await page.evaluate(() => window.solomonDarkRuntime.gameEndpoint), undefined);
  await ui.fill(page, 'launcher/host', '');
  await ui.click(page, 'launcher/start-host');
  await ui.value(page, 'launcher/status', 'Enter your reachable IP or hostname.');
  await ui.fill(page, 'launcher/host', '127.0.0.1');
  await ui.fill(page, 'launcher/port', '0');
  await ui.click(page, 'launcher/start-host');
  await ui.value(page, 'launcher/status', 'Enter a TCP port from 1 to 65535.');
  await ui.fill(page, 'launcher/port', '27843');
  await page.keyboard.press('ArrowUp');
  await ui.value(page, 'launcher/port', '27844');
  await page.keyboard.press('ArrowDown');
  await ui.value(page, 'launcher/port', '27843');
  await ui.fill(page, 'launcher/host', initialHost);
  await ui.fill(page, 'launcher/invite', '');
  await page.mouse.move(100, 100);
  await page.mouse.wheel(0, 700);
  await ui.click(page, 'launcher/help');
  await ui.value(page, 'launcher/help', 'open');
  for (let step = 0; step < 12
    && !ui.findWidget(await ui.readUi(page), 'launcher/help')?.focused; step++) {
    await page.keyboard.press('Tab');
    await ui.nextFrame(page);
  }
  await ui.focused(page, 'launcher/help');
  await page.keyboard.press('Space');
  await ui.value(page, 'launcher/help', 'closed');
  await ui.click(page, 'launcher/check');
  await ui.waitForUi(page, state => {
    const status = ui.findWidget(state, 'launcher/status')?.value;
    return status && status !== 'Checking for updates…';
  }, 'the real offline update result');
  const state = await page.evaluate(() => window.solomonDarkRuntime.desktop.getState());
  assert.ok(state.updateError);
  await ui.value(page, 'launcher/status', state.updateError);
  await ui.widget(page, 'launcher/website', {enabled: true});
  await ui.assertGpuOnly(page);
  await page.screenshot({path: join(evidence, 'launcher-validation.png')});
  await page.mouse.move(100, 100);
  await page.mouse.wheel(0, -1000);
  return {gpuOnly: true, requiredInputs: true, nativeJoinError: true,
    draftPreserved: true, portKeyboard: true, keyboardHelp: true, offlineUpdates: true};
}

async function captureMedia(instance, phase) {
  const report = await ui.settleErrors(instance.page);
  receipts.mediaObservations.push({label: instance.label, phase,
    at: new Date().toISOString(), report});
  for (const suspension of report.expectedSuspensions) {
    if (!receipts.cancelledMusic.some(previous => previous.label === instance.label
      && previous.failure.requestId === suspension.failure.requestId)) {
      receipts.cancelledMusic.push({label: instance.label, ...suspension});
    }
  }
}
async function title(page) {
  await page.waitForURL('solomon-darker://app/game', {waitUntil: 'domcontentloaded'});
  await ui.ready(page);await ui.surface(page,'title');
  if(ui.findWidget(await ui.readUi(page),'prompt-secondary')){await ui.click(page,'prompt-secondary');await ui.noWidget(page,'prompt-secondary');}
  await ui.assertGpuOnly(page);
  const instance = [...running].find(candidate => candidate.page === page);
  const bootstrap = await page.evaluate(() => ({
    bases: document.querySelectorAll('base').length,
    scripts: [...document.querySelectorAll('script[type="module"][src]')]
      .map(script => script.getAttribute('src')),
    inlineModules: [...document.querySelectorAll('script[type="module"]')]
      .filter(script => !script.hasAttribute('src') && script.textContent.trim()).length,
  }));
  assert.equal(bootstrap.inlineModules, 0, 'the real client needs no inline-script CSP exception');
  assert.equal(bootstrap.bases, 0, 'the packaged document honors its base-uri none policy');
  assert.equal(bootstrap.scripts.length, 1);
  assert.match(bootstrap.scripts[0], /^\/solomon-boot-[a-f0-9]{16}\.mjs$/);
  const resources = instance.state.clientResources;
  for (const extension of ['js', 'wasm']) {
    assert.ok(resources.some(resource => resource.status === 200
      && new RegExp(`^/solomon-web-[a-f0-9]+(?:_bg)?\\.${extension}$`).test(resource.path)),
    `the packaged root serves the actual generated ${extension} resource`);
  }
  assert.ok(resources.every(resource => !resource.path.startsWith('/game-stg/')
    && !resource.path.startsWith('/editor-test/')));
  receipts.clientStartups.push({label: instance.label, bootstrap: bootstrap.scripts[0], resources});
}
async function enterHub(instance, name, element) {
  await title(instance.page);
  await startNewGame(instance.page, {
    name,
    element
  });
  await readyHub(instance);
  assert.equal(instance.state.frame.snapshot.participants[instance.state.id].player.selection.name, name);
}
async function readyHub(instance) {
  await waitFor(() => instance.state.id && !instance.state.frame?.snapshot.boneyard, 'accepted College');
  await ui.surface(instance.page,'hidden');
  await instance.page.waitForFunction(()=>window.wasmBindings.wizard_state()[6]===0&&window.wasmBindings.wizard_state()[7]===-1);
}
async function move(instance) {
  await instance.page.bringToFront();
  const before = position(instance).x;
  await instance.page.keyboard.down('KeyD');
  try {
    await waitFor(() => position(instance).x > before + 10, 'accepted movement');
  } finally {
    await instance.page.keyboard.up('KeyD');
  }
  return {
    before,
    after: position(instance).x
  };
}
async function readSave(page) {
  const stored = await readStoredSave(page);
  return {...stored, document: JSON.parse(stored.document)};
}
async function readStoredSave(page) {
  return page.evaluate(() => new Promise((resolveSave, reject) => {
    const open = indexedDB.open('solomon-dark-game-saves', 1);
    open.onerror = () => reject(open.error);
    open.onsuccess = () => {
      const database = open.result,
        request = database.transaction('slots').objectStore('slots').get(0);
      request.onerror = () => {
        database.close();
        reject(request.error);
      };
      request.onsuccess = () => {
        database.close();
        resolveSave(request.result);
      };
    };
  }));
}
async function proveFailedClose(instance) {
  const child = await hostProcess(instance.app);
  const identity = instance.state.id;
  const disconnects = instance.state.disconnects;
  const closedSockets = instance.state.closedSockets;
  const saveReceipts = instance.state.leaveReceipts;
  const original = await readStoredSave(instance.page);
  instance.state.expectedStorageConflict = true;
  const concurrent = await instance.page.evaluate(() => new Promise((resolveWrite, reject) => {
    const open = indexedDB.open('solomon-dark-game-saves', 1);
    open.onerror = () => reject(open.error);
    open.onsuccess = () => {
      const database = open.result;
      const transaction = database.transaction('slots', 'readwrite');
      const store = transaction.objectStore('slots');
      const request = store.get(0);
      let committed;
      transaction.onabort = transaction.onerror = () => {
        database.close();
        reject(transaction.error ?? new Error('Concurrent save transaction failed.'));
      };
      transaction.oncomplete = () => {
        database.close();
        resolveWrite(committed);
      };
      request.onsuccess = () => {
        committed = {...request.result, revision: request.result.revision + 1};
        store.put(committed);
      };
    };
  }));
  assert.equal(concurrent.revision, original.revision + 1);
  assert.equal(concurrent.document, original.document, 'the concurrent writer changes no saved gameplay');
  // Only automate the response to the real Electron warning. The production
  // preload callback, host save receipt and IndexedDB rejection remain intact.
  await instance.app.evaluate(({dialog}) => {
    const original = dialog.showMessageBox.bind(dialog);
    const state = globalThis.__desktopCloseSmoke = {calls: [], answer: null, cleanup: false};
    dialog.showMessageBox = (window, options) => {
      if (options.message !== 'The latest progress could not be saved') return original(window, options);
      state.calls.push({...options});
      if (state.cleanup) return Promise.resolve({response: 1, checkboxChecked: false});
      return new Promise(resolveDialog => { state.answer = resolveDialog; });
    };
  });
  const requestClose = () => instance.app.evaluate(({BrowserWindow}) => BrowserWindow.getAllWindows()[0].close());
  const warning = async index => {
    await waitFor(() => instance.app.evaluate((_, index) =>
      globalThis.__desktopCloseSmoke.calls.length === index, index), 'the actual failed-save close warning');
    const options = await instance.app.evaluate((_, index) => globalThis.__desktopCloseSmoke.calls[index - 1], index);
    assert.equal(options.type, 'warning');
    assert.equal(options.message, 'The latest progress could not be saved');
    assert.equal(options.detail, storageConflict, 'the durable revision guard rejected, rather than the close timer');
    assert.deepEqual(options.buttons, ['Keep playing', 'Close without saving']);
    assert.equal(options.defaultId, 0);
    assert.equal(options.cancelId, 0);
    return options;
  };
  const answer = response => instance.app.evaluate((_, response) => {
    const state = globalThis.__desktopCloseSmoke;
    const resolveDialog = state.answer;
    state.answer = null;
    resolveDialog({response, checkboxChecked: false});
  }, response);
  await requestClose();
  const keepPlaying = await warning(1);
  await answer(0);
  assert.equal(instance.page.isClosed(), false);
  assert.equal(instance.app.process().exitCode, null);
  assert.equal((await hostProcess(instance.app)).pid, child.pid);
  await waitFor(async () => {
    const paused = await ui.readUi(instance.page);
    if (paused?.error) assert.equal(paused.error, storageConflict);
    return paused?.error === storageConflict && paused.surface === 'pause';
  }, 'the GPU frame reports the deliberately rejected write and paused game');
  // This case deliberately owns that exact nonfatal storage notice. Use ordinary
  // Escape and observe recovery directly; generic GPU helpers reject all errors.
  await instance.page.keyboard.press('Escape');
  await waitFor(() => !instance.state.frame.host.pause && !instance.state.frame.host.resume_grace,
    'the accepted post-warning resume');
  await waitFor(async () => {
    const current = await ui.readUi(instance.page);
    assert.equal(current.error, storageConflict);
    return current.surface === 'hidden';
  }, 'the visible pause menu closes while the exact storage conflict remains reported');
  const movement = await move(instance);
  assert.equal(instance.state.id, identity, 'Keep playing retains the admitted native participant');
  assert.equal(instance.state.disconnects, disconnects, 'a rejected durable write never acknowledges Disconnect');
  assert.equal(instance.state.closedSockets, closedSockets, 'Keep playing retains the actual game connection');
  assert.ok(instance.state.leaveReceipts > saveReceipts, 'the close requested a real host checkpoint');
  assert.deepEqual(await readStoredSave(instance.page), concurrent, 'the committed record remains byte-for-byte intact');
  assert.ok(receipts.storageWarnings.some(warning => warning.label === instance.label));
  await instance.page.screenshot({path: join(evidence, 'failed-close-kept-playing.png')});
  const keptLeaveReceipts = instance.state.leaveReceipts - saveReceipts;
  const keptDisconnects = instance.state.disconnects - disconnects;
  await requestClose();
  const closeWithoutSaving = await warning(2);
  assert.deepEqual(await readStoredSave(instance.page), concurrent);
  const exited = once(instance.app, 'close');
  await captureMedia(instance, 'before-close-without-saving');
  await answer(1);
  let timer;
  try {
    await Promise.race([exited, new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error('Close without saving did not exit the app')), 30000);
    })]);
  } finally { clearTimeout(timer); }
  running.delete(instance);
  await waitForExit(child.pid);
  await verifyServiceStops(instance);
  return {
    fault: 'A native IndexedDB concurrent writer advances only the committed Source slot revision.',
    dialogAutomation: 'Only Electron showMessageBox responses are automated; no save callback is replaced.',
    priorRevision: original.revision,
    concurrentRevision: concurrent.revision,
    dialogs: [{...keepPlaying, chosen: 'Keep playing'}, {...closeWithoutSaving, chosen: 'Close without saving'}],
    movementAfterKeepPlaying: movement,
    hostCheckpointReceipts: keptLeaveReceipts,
    disconnectsAfterRejectedWrite: keptDisconnects,
    committedRecordUnchanged: true,
    sameNativeHostAndParticipant: true,
    closedWithoutSaveAndReapedHost: true,
  };
}
async function hostProcess(application) {
  const instance = [...running].find(instance => instance.app === application);
  const children = await instance.nativeChildren.evaluate(observer => observer.nativeChildren());
  assert.equal(children.length, 1, 'the app owns one live separate native authority');
  const [child] = children;
  return {pid: child.pid, command: child.arguments.join(' ')};
}
async function proveRepeatedClose(instance) {
  await captureMedia(instance, 'before-repeated-close');
  // Hold only the final navigation, after the real save, to send real repeated
  // close/quit requests while the existing document and services remain alive.
  await instance.app.evaluate(({BrowserWindow}) => {
    const window = BrowserWindow.getAllWindows()[0];
    const loadURL = window.loadURL;
    const state = globalThis.__desktopRetirementSmoke = {entered: false, release: null};
    window.loadURL = function(url, ...options) {
      if (url !== 'about:blank') return Reflect.apply(loadURL, this, [url, ...options]);
      state.entered = true;
      return new Promise((resolveNavigation, reject) => {
        state.release = () => {
          window.loadURL = loadURL;
          Reflect.apply(loadURL, window, [url, ...options]).then(resolveNavigation, reject);
        };
      });
    };
  });
  const exited = once(instance.app, 'close');
  let released = false;
  try {
    await bounded((async () => {
      await instance.app.evaluate(({BrowserWindow}) => BrowserWindow.getAllWindows()[0].close());
      await waitFor(() => instance.app.evaluate(() => globalThis.__desktopRetirementSmoke.entered),
        'the saved document retirement');
      assert.ok(await instance.app.evaluate(({app, BrowserWindow}) => {
        const window = BrowserWindow.getAllWindows()[0];
        window.close();
        app.quit();
        return !window.isDestroyed();
      }), 'repeated close and quit leave the saved document owned until retirement');
      await instance.app.evaluate(() => { globalThis.__desktopRetirementSmoke.release(); });
      released = true;
      await exited;
    })(), 30000, 'Repeated close did not finish owned shutdown');
  } finally {
    if (!released) await bounded(instance.app.evaluate(() => {
      globalThis.__desktopRetirementSmoke.release?.();
    }), 5000, 'Retirement test cleanup timed out').catch(() => {});
  }
  running.delete(instance);
  await verifyServiceStops(instance);
  return {repeatedWindowClose: true, repeatedAppQuit: true, savedRetirementCompleted: true};
}
async function proveReturnToLauncher() {
  const instance = await launch('return', join(profiles, 'return'));
  await ui.click(instance.page, 'launcher/solo');
  await enterHub(instance, 'Return', 'fire');
  const child = await hostProcess(instance.app);
  await captureMedia(instance, 'before-return-to-launcher');
  await instance.app.evaluate(({Menu}) => {
    Menu.getApplicationMenu().items.find(row => row.label === 'Game').submenu.items
      .find(row => row.label === 'Save and return to launcher').click();
  });
  await instance.page.waitForURL('solomon-darker://app/launcher', {waitUntil: 'domcontentloaded'});
  await ui.ready(instance.page);
  await ui.surface(instance.page, 'desktop-launcher');
  await ui.assertGpuOnly(instance.page);
  await waitForExit(child.pid);
  assert.ok(instance.state.leaveReceipts > 0, 'returning to the launcher awaited the actual host save receipt');
  assert.equal(await instance.page.evaluate(() => window.solomonDarkRuntime.gameEndpoint), undefined,
    'the new launcher has no retired native endpoint');
  const saved = await readSave(instance.page);
  assert.equal(saved.document.schemaVersion, 51);
  assert.equal(saved.document.continuation.summary.playerId, instance.state.id);
  await close(instance);
  return {hostSaveReceipt: true, nativeHostExited: true, endpointCleared: true,
    savedPlayer: saved.document.continuation.summary.playerId};
}
async function proveUpdatePrompt(instance) {
  const fixture = {
    revision: 'f'.repeat(40),
    sourceTimestamp: manifest.sourceTimestamp + 1,
    repository: manifest.releaseRepository
  };
  await instance.app.evaluate(({
    net
  }, fixture) => {
    const original = net.fetch.bind(net),
      base = `https://github.com/${fixture.repository}/releases/download/desktop-${fixture.revision}`;
    net.fetch = (url, options) => {
      if (url === `https://api.github.com/repos/${fixture.repository}/releases/latest`) return Promise.resolve(Response.json({
        tag_name: `desktop-${fixture.revision}`,
        draft: false,
        prerelease: false,
        assets: ['desktop-release.json', `Solomon-Darker-${process.platform}-${process.arch}.zip`].map(name => ({
          name,
          browser_download_url: `${base}/${name}`
        }))
      }));
      if (url === `${base}/desktop-release.json`) {
        net.fetch = original;
        return Promise.resolve(Response.json({
          schemaVersion: 1,
          revision: fixture.revision,
          sourceTimestamp: fixture.sourceTimestamp
        }));
      }
      return original(url, options);
    };
  }, fixture);
  const state = await instance.page.evaluate(() => window.solomonDarkRuntime.desktop.checkUpdates());
  assert.equal(state.update.revision, fixture.revision);
  await ui.widget(instance.page,'desktop/download');
  const notice=await ui.widget(instance.page,'desktop/update');
  const {x:left,y:top,width,height}=ui.bounds(notice);const viewport=await instance.page.evaluate(()=>({width:innerWidth,height:innerHeight}));
  const bounds={left,top,right:left+width,bottom:top+height,width,height,viewportWidth:viewport.width,viewportHeight:viewport.height};
  assert.ok(width>0&&height>0&&left>=0&&top>=0&&bounds.right<=viewport.width&&bounds.bottom<=viewport.height,
    `the GPU desktop update notice is inside the actual game window: ${JSON.stringify(bounds)}`);
  await ui.assertGpuOnly(instance.page);await instance.page.screenshot({path:join(evidence,'update-prompt.png')});
  await ui.click(instance.page,'desktop/later');await ui.noWidget(instance.page,'desktop/update');
  assert.ok(instance.state.frame, 'update dismissal leaves the accepted game alive');
  return {
    shown: true,
    bounds,
    dismissedWithoutRestart: true
  };
}
async function cleanupApp(app, appPid) {
  const child = app.process();
  const timeout = setTimeout(() => {
    if (child.exitCode !== null || child.signalCode !== null) return;
    if (process.platform === 'win32' && appPid === undefined) {
      // Before inspector initialization, Playwright owns only a Windows cmd
      // wrapper. The native tree operation includes its actual Electron child.
      execFile('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'],
        {timeout: 5000, windowsHide: true}, error => { if (error) console.error(redact(error.message)); });
      return;
    }
    try { process.kill(appPid ?? child.pid, 'SIGKILL'); }
    catch (error) { if (error.code !== 'ESRCH') console.error(redact(error.message)); }
  }, 25000);
  try {
    await bounded(app.close(), 30000, 'Desktop failure cleanup timed out');
  } catch (error) { console.error(redact(error.message)); }
  finally { clearTimeout(timeout); }
}
async function close(instance) {
  await captureMedia(instance, 'before-close');
  await bounded(instance.app.close(), 30000, `${instance.label} did not close after saving`);
  running.delete(instance);
  await verifyServiceStops(instance);
}
async function verifyServiceStops(instance) {
  const stops = (await readFile(instance.stopJournal, 'utf8')).trim().split('\n').filter(Boolean).map(row => JSON.parse(row));
  assert.ok(stops.length > 0, 'the real app requested its owned services to stop');
  assert.ok(stops.every(stop => !stop.gameDocumentLive),
    'the game document retires before any owned service receives its stop request');
  receipts.serviceStops.push({label: instance.label, stops});
}
async function bounded(operation, timeout, message) {
  let timer;
  try {
    await Promise.race([operation, new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error(message)), timeout);
    })]);
  } finally {
    clearTimeout(timer);
  }
}
async function setWindowMinimized(instance, minimized) {
  await instance.app.evaluate(({BrowserWindow}, minimized) => {
    const window = BrowserWindow.getAllWindows()[0];
    if (minimized) window.minimize();
    else { window.restore(); window.show(); window.focus(); }
  }, minimized);
  // Native window methods return before focus/visibility and rendering settle.
  await waitFor(() => instance.app.evaluate(({BrowserWindow}, minimized) => {
    const window = BrowserWindow.getAllWindows()[0];
    return minimized ? window.isMinimized()
      : !window.isMinimized() && window.isVisible() && window.isFocused();
  }, minimized), `the actual ${instance.label} window ${minimized ? 'minimized' : 'visible and focused'}`);
  if (!minimized) {
    const previousFrame = (await ui.readUi(instance.page))?.frame;
    await instance.page.waitForFunction(previousFrame => {
      if (!document.hasFocus() || document.visibilityState !== 'visible') return false;
      const read = window.wasmBindings?.gpu_ui_state;
      return !read || previousFrame === undefined || JSON.parse(read()).frame !== previousFrame;
    }, previousFrame);
  }
}
async function waitForExit(pid) {
  await waitFor(() => {
    try {
      process.kill(pid, 0);
      return false;
    } catch (error) {
      if (error.code === 'ESRCH') return true;
      throw error;
    }
  }, 'reaped native authority');
}
async function waitFor(predicate, label, timeout = 30000) {
  const deadline = Date.now() + timeout;
  while (!(await predicate())) {
    if (Date.now() > deadline) throw new Error(`Timed out waiting for ${label}`);
    await delay(40);
  }
}
async function freePort() {
  const server = createServer();
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const port = server.address().port;
  await new Promise(resolveClose => server.close(resolveClose));
  return port;
}
