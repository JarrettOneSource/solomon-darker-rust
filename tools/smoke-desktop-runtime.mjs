// Real packaged Electron -> native authority -> WASM acceptance. Public UI drives
// gameplay; sockets only observe accepted state. The failed-close case advances
// only the real IndexedDB record revision to represent a concurrent writer.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { once } from 'node:events';
import { appendFileSync, createReadStream } from 'node:fs';
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
const obsoleteMenuImage = require('./browser-smoke/obsolete-menu-image.cjs');
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
const running = new Set();
const errors = [];
const resourceChecks = [];
const imageAbortQueries = [];
const receipts = { platform: process.platform, revision: manifest.revision, diagnosticNativeExitDialogIntercept: true, clientStartups: [], cancelledMusic: [], cancelledMenuImages: [], storageWarnings: [], failedImageObservations: [] };
let phase = 'launch-solo';
let phaseBytes = 0;
const redact = value => String(value).replace(/solomondarker:\/\/[^\s'"<>]+/g, '[redacted-invite]');
function recordPhase(value) {
  const line = `${JSON.stringify(value)}\n`;
  console.log(line.trimEnd());
  const bytes = Buffer.byteLength(line);
  if (phaseBytes + bytes <= 128 * 1024) {
    appendFileSync(join(evidence, 'phases.jsonl'), line);
    phaseBytes += bytes;
  }
}
function progress(nextPhase) {
  phase = nextPhase;
  recordPhase({diagnosticPhase: phase, at: new Date().toISOString()});
}
const heartbeat = setInterval(() => {
  recordPhase({diagnosticPhase: phase, at: new Date().toISOString(), instances: [...running].map(instance => ({
    label: instance.label, sequence: instance.state.frame?.sequence, tick: instance.state.frame?.snapshot.ticks,
    position: position(instance), closedSockets: instance.state.closedSockets, sentDisconnects: instance.state.disconnects,
  }))});
}, 15000);
const storageConflict = 'The saved game changed in another tab. Reload to continue that save.';
const position = instance => instance.state.frame?.snapshot.participants[instance.state.id]?.wizard.position;
const playing = instance => instance.state.frame?.snapshot.boneyard && !instance.state.frame.host.loading_run_id && !instance.state.frame.host.resume_grace && !instance.state.frame.host.pause;
try {
  const solo = await launch('solo', join(profiles, 'solo'));
  await solo.page.screenshot({
    path: join(evidence, 'launcher.png')
  });
  await solo.page.getByRole('button', {
    name: 'Play offline'
  }).click();
  progress('solo-title-settings');
  await title(solo.page);
  await solo.page.locator('[data-menu-action="settings"]').click();
  const music = solo.page.locator('input[data-setting="music"]');
  await music.focus();
  await music.press('Home');
  await music.press('ArrowRight');
  await solo.page.locator('[data-menu-action="settings-page:controls"]').click();
  await solo.page.locator('[data-setting="reverse-hotbar"]').click();
  await solo.page.locator('[data-menu-action="settings-back"]').click();
  await solo.page.locator('[data-menu-action="settings-back"]').click();
  progress('solo-college-admission');
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
  progress('solo-movement');
  const movement = await move(solo);
  await solo.page.screenshot({
    path: join(evidence, 'solo.png')
  });
  const updateStatus = await solo.page.evaluate(() => window.solomonDarkRuntime.desktop.checkUpdates());
  assert.equal(updateStatus.update, null);
  assert.ok(updateStatus.updateError, 'offline update failure does not block play');
  progress('solo-save-close');
  await close(solo);
  assert.ok(solo.state.leaveReceipts > 0, 'ordinary app close awaited the actual host save receipt');
  await waitForExit(child.pid);
  progress('launch-resume');
  const resumed = await launch('resume', join(profiles, 'solo'));
  await resumed.page.getByRole('button', {
    name: 'Play offline'
  }).click();
  progress('resume-title-settings');
  await title(resumed.page);
  const preferences = await resumed.page.evaluate(() =>
    JSON.parse(localStorage.getItem('solomon-dark-game-settings-v1')));
  assert.equal(preferences.musicVolumePercent, 1);
  assert.equal(preferences.reverseHotbarCycle, true);
  await resumed.page.locator('[data-menu-action="settings"]').click();
  assert.equal(await resumed.page.locator('input[data-setting="music"]').inputValue(), '1');
  await resumed.page.locator('[data-menu-action="settings-page:controls"]').click();
  assert.equal(await resumed.page.locator('[data-setting="reverse-hotbar"]').getAttribute('aria-pressed'), 'true');
  await resumed.page.locator('[data-menu-action="settings-back"]').click();
  await resumed.page.locator('[data-menu-action="settings-back"]').click();
  const saved = await readSave(resumed.page);
  assert.equal(saved.document.schemaVersion, 51);
  const id = saved.document.continuation.summary.playerId;
  const entities = saved.document.continuation.simulation.playerEntities;
  const playerIndex = entities.identities.findIndex(row => row.playerId === id);
  assert.ok(playerIndex >= 0, 'the Source entity columns retain the saved player identity');
  const locomotion = entities.locomotions[playerIndex];
  assert.ok(locomotion.position.x > movement.before + 10);
  await resumed.page.locator('[data-menu-action="play"]').click();
  await resumed.page.locator('[data-menu-action="continue"]').click();
  progress('resume-college');
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
  progress('resume-update-prompt');
  receipts.updatePrompt = await proveUpdatePrompt(resumed);
  progress('resume-failed-close');
  receipts.failedClose = await proveFailedClose(resumed);
  progress('launch-host');
  const host = await launch('host', join(profiles, 'host'));
  const port = await freePort();
  await host.page.getByLabel('Your reachable IP or hostname').fill('127.0.0.1');
  await host.page.getByLabel('TCP port').fill(String(port));
  await host.page.getByRole('button', {
    name: 'Host game',
    exact: true
  }).click();
  progress('host-college-admission');
  await enterHub(host, 'PeerHost', 'fire');
  progress('host-process-and-invite');
  const hostChild = await hostProcess(host.app);
  const invitation = await host.app.evaluate(({
    Menu,
    clipboard
  }) => {
    Menu.getApplicationMenu().items.find(row => row.label === 'Game').submenu.items.find(row => row.label === 'Copy multiplayer invite').click();
    return clipboard.readText();
  });
  progress('launch-guest');
  const guest = await launch('guest', join(profiles, 'guest'));
  progress('guest-join');
  await guest.page.getByLabel('Multiplayer invite').fill(invitation);
  await guest.page.getByRole('button', {
    name: 'Join game',
    exact: true
  }).click();
  progress('guest-college-admission');
  await enterHub(guest, 'PeerGuest', 'air');
  await waitFor(() => [host, guest].every(instance => instance.state.frame?.snapshot.participant_order.length === 2), 'both admitted peers');
  assert.notEqual(host.state.id, guest.state.id);
  assert.equal(host.state.frame.host.authority_player_id, host.state.id);
  assert.equal(guest.state.frame.host.authority_player_id, host.state.id);
  progress('guest-movement');
  const guestMovement = await move(guest);
  progress('peer-boneyard');
  await host.page.getByRole('button', {
    name: 'Enter the Boneyard',
    exact: true
  }).click();
  await waitFor(() => playing(host) && playing(guest), 'both actual Boneyard renderers', 120000);
  assert.equal(host.state.frame.snapshot.boneyard.run_id, guest.state.frame.snapshot.boneyard.run_id);
  assert.equal(host.state.frame.snapshot.participant_order.length, 2);
  await host.page.screenshot({
    path: join(evidence, 'peer-host-boneyard.png')
  });
  await guest.page.screenshot({
    path: join(evidence, 'peer-guest-boneyard.png')
  });
  receipts.peer = {
    hostPlayer: host.state.id,
    guestPlayer: guest.state.id,
    guestMovement,
    sharedBoneyardPlayers: 2
  };
  await close(guest);
  await close(host);
  await waitForExit(hostChild.pid);
  await Promise.all(resourceChecks);
  await Promise.allSettled(imageAbortQueries);
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
  clearInterval(heartbeat);
  const originalError = redact(error.stack);
  console.error(JSON.stringify({diagnosticPhase: phase, originalError}));
  await writeFile(join(evidence, 'original-error.json'), `${JSON.stringify({phase, originalError}, null, 2)}\n`);
  const failedInstances = [];
  for (const instance of running) {
    await instance.page.screenshot({
      path: join(evidence, `${instance.label}-failure.png`), timeout: 8000,
    }).catch(error => console.error(JSON.stringify({label: instance.label, diagnosticScreenshotError: redact(error.message)})));
    const failed = {
      label: instance.label,
      url: instance.page.url(),
      body: (await instance.page.locator('body').innerText({timeout: 5000}).catch(() => '')).slice(0, 2500),
      accepted: {
        playerId: instance.state.id,
        sequence: instance.state.frame?.sequence,
        tick: instance.state.frame?.snapshot.ticks,
        position: position(instance),
        closedSockets: instance.state.closedSockets,
        sentDisconnects: instance.state.disconnects,
      },
      nativeExitDialogs: await bounded(instance.app.evaluate(() => globalThis.__nativeExitDialogs || []), 5000, 'native exit dialogs').catch(error => ({diagnosticError: redact(error.message)})),
    };
    failedInstances.push(failed);
    console.error(JSON.stringify({ ...failed, errors, cancelledMenuImages: receipts.cancelledMenuImages }));
  }
  await bounded(Promise.allSettled([...imageAbortQueries, ...resourceChecks]), 5000, 'resource observations').catch(() => {});
  await writeFile(join(evidence, 'failure.json'), `${JSON.stringify({
    status: 'failed', phase, error: originalError, ...receipts, instances: failedInstances, errors,
  }, null, 2)}\n`);
  throw error;
} finally {
  clearInterval(heartbeat);
  for (const instance of running) {
    await bounded(instance.app.evaluate(() => {
      const dialog = globalThis.__desktopCloseSmoke;
      if (dialog) {
        dialog.cleanup = true;
        dialog.answer?.({response: 1, checkboxChecked: false});
      }
    }), 5000, 'dialog cleanup').catch(() => {});
    const timeout = setTimeout(() => instance.app.process().kill('SIGKILL'), 25000);
    await instance.app.close().catch(() => {});
    clearTimeout(timeout);
  }
  await rm(profiles, {
    recursive: true,
    force: true
  });
}
async function bounded(operation, timeout, label) {
  let timer;
  try {
    return await Promise.race([operation, new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error(`Timed out collecting ${label}`)), timeout);
    })]);
  } finally { clearTimeout(timer); }
}
async function launch(label, userData) {
  const app = await electron.launch({
    executablePath: executable,
    args: ['--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--use-angle=swiftshader', '--use-gl=angle', '--proxy-server=http://127.0.0.1:9'],
    env: {
      ...process.env,
      SDR_DESKTOP_USER_DATA: userData,
      ELECTRON_DISABLE_SECURITY_WARNINGS: 'true'
    },
    timeout: 30000
  });
  appendFileSync(join(evidence, 'owned-processes.jsonl'), `${JSON.stringify({
    label, pid: app.process().pid, executable, recordedAt: new Date().toISOString(),
  })}\n`);
  const page = await app.firstWindow({
    timeout: 30000
  });
  // Retain the real child exit notice instead of blocking this diagnostic run on
  // a native modal dialog that cannot be captured by a renderer screenshot.
  await app.evaluate(({dialog}) => {
    const original = dialog.showErrorBox.bind(dialog);
    globalThis.__nativeExitDialogs = [];
    dialog.showErrorBox = (title, content) => {
      if (title !== 'The local game stopped') return original(title, content);
      globalThis.__nativeExitDialogs.push({at: Date.now(), title, content});
    };
  });
  app.context().setDefaultTimeout(30000);
  const instance = {
    app,
    page,
    label,
    state: {
      id: null,
      frame: null,
      leaveReceipts: 0,
      disconnects: 0,
      closedSockets: 0,
      expectedStorageConflict: false,
      clientResources: []
    }
  };
  running.add(instance);
  page.on('pageerror', error => errors.push(`${label}: ${error.message}`));
  page.on('console', message => {
    if (message.type() === 'warning' && message.text() === storageConflict
      && instance.state.expectedStorageConflict) {
      receipts.storageWarnings.push({label, message: message.text()});
      return;
    }
    if (message.type() === 'error' && !message.text().includes('ERR_PROXY_CONNECTION_FAILED')) errors.push(`${label}: ${message.text()}`);
  });
  page.on('response', response => {
    const url = new URL(response.url());
    if (url.protocol !== 'solomon-darker:' || url.host !== 'app'
      || !/\.(?:m?js|wasm)$/.test(url.pathname)) return;
    instance.state.clientResources.push({path: url.pathname, status: response.status()});
  });
  page.on('requestfailed', request => {
    if (request.url().startsWith('solomon-darker://app/')) {
      if (request.failure()?.errorText === 'net::ERR_ABORTED'
        && new URL(request.url()).pathname.startsWith('/assets/stock/audio/music/')) {
        receipts.cancelledMusic.push({label, path: new URL(request.url()).pathname});
        return;
      }
      if (request.failure()?.errorText === 'net::ERR_ABORTED'
        && /\.(?:png|jpg|webp)$/.test(new URL(request.url()).pathname)) {
        const query = page.evaluate(failedUrl => ({
          menu: document.querySelector('.game-menus')?.dataset.menuScreen,
          images: [...document.images]
            .filter(image => image.src === failedUrl || image.currentSrc === failedUrl
              || /(?:create-hand|menu-solomon-cloak)/.test(image.src))
            .map(image => ({
              src: new URL(image.src, location.href).pathname,
              currentSrc: image.currentSrc ? new URL(image.currentSrc, location.href).pathname : null,
              stillReferencesFailedRequest: image.src === failedUrl || image.currentSrc === failedUrl,
              complete: image.complete, naturalWidth: image.naturalWidth, connected: image.isConnected,
            })),
        }), request.url()).then(observation => {
          receipts.failedImageObservations.push({label, failedPath: new URL(request.url()).pathname, ...observation});
        }).catch(error => {
          receipts.failedImageObservations.push({label, failedPath: new URL(request.url()).pathname, observationError: error.message});
        });
        imageAbortQueries.push(query);
      }
      resourceChecks.push(obsoleteMenuImage(page, request).then(cancelled => {
        if (cancelled) receipts.cancelledMenuImages.push({label, ...cancelled});
        else errors.push(`${label}: ${new URL(request.url()).pathname}: ${request.failure()?.errorText}`);
      }));
    }
  });
  page.on('websocket', socket => {
    const readMessage = createNativeMessageReader();
    socket.on('framesent', ({payload}) => {
      if (JSON.parse(payload.toString()).type === 'disconnect') instance.state.disconnects++;
    });
    socket.on('close', () => instance.state.closedSockets++);
    socket.on('framereceived', ({payload}) => {
      const message = readMessage(JSON.parse(payload.toString()));
      if (message === null) return;
      if (message.type === 'welcome') {
        instance.state.id = message.credentials.player_id;
        instance.state.frame = message.frame;
      } else if (message.type === 'snapshot') instance.state.frame = message.frame;else if (message.type === 'hosted' && message.message.kind === 'save_before_leave') instance.state.leaveReceipts++;else if (message.type === 'error') errors.push(`${label} host: ${message.code}: ${message.reason}`);
    });
  });
  await page.getByRole('button', {
    name: 'Play offline'
  }).waitFor({
    timeout: 30000
  });
  return instance;
}
async function title(page) {
  await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0
    && document.querySelector('#status')?.textContent === '', null, {
    timeout: 120000
  });
  await page.locator('.game-menus[data-menu-screen="title"]').waitFor();
  const prompt = page.locator('.title-prompt:not([hidden])');
  if (await prompt.isVisible()) {
    await prompt.locator('[data-menu-action="prompt-secondary"]').click();
    await prompt.waitFor({state: 'hidden'});
  }
  const instance = [...running].find(candidate => candidate.page === page);
  const bootstrap = await page.evaluate(() => ({
    scripts: [...document.querySelectorAll('script[type="module"][src]')]
      .map(script => script.getAttribute('src')),
    inlineModules: [...document.querySelectorAll('script[type="module"]')]
      .filter(script => !script.hasAttribute('src') && script.textContent.trim()).length,
  }));
  assert.equal(bootstrap.inlineModules, 0, 'the real client needs no inline-script CSP exception');
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
  progress(`${instance.label}-title`);
  await title(instance.page);
  progress(`${instance.label}-start-new-game`);
  await startNewGame(instance.page, {
    name,
    element
  });
  progress(`${instance.label}-ready-college`);
  await readyHub(instance);
  assert.equal(instance.state.frame.snapshot.participants[instance.state.id].player.selection.name, name);
}
async function readyHub(instance) {
  await waitFor(() => instance.state.id && !instance.state.frame?.snapshot.boneyard, 'accepted College');
  await instance.page.waitForFunction(() => document.querySelector('.game-menus')?.dataset.menuScreen === 'hidden' && window.wasmBindings.wizard_state()[6] === 0 && window.wasmBindings.wizard_state()[7] === -1);
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
  await instance.page.locator('[data-menu-action="resume"]').click();
  await readyHub(instance);
  await waitFor(() => !instance.state.frame.host.pause && !instance.state.frame.host.resume_grace,
    'the accepted post-warning resume');
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
  await answer(1);
  let timer;
  try {
    await Promise.race([exited, new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error('Close without saving did not exit the app')), 30000);
    })]);
  } finally { clearTimeout(timer); }
  running.delete(instance);
  await waitForExit(child.pid);
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
  const parent = await application.evaluate(() => process.pid);
  if (process.platform === 'win32') {
    const raw = execFileSync('powershell.exe', ['-NoProfile', '-Command', `Get-CimInstance Win32_Process -Filter "ParentProcessId=${parent}" | Select-Object ProcessId,ExecutablePath,CommandLine | ConvertTo-Json -Compress`], {
      encoding: 'utf8'
    });
    const value = JSON.parse(raw),
      child = (Array.isArray(value) ? value : [value]).find(row => row.CommandLine?.includes('--desktop'));
    assert.ok(child, 'the app owns a separate native authority');
    return {
      pid: child.ProcessId,
      command: child.CommandLine
    };
  }
  const child = execFileSync('ps', ['-axo', 'pid=,ppid=,command='], {
    encoding: 'utf8'
  }).split('\n').map(line => /^\s*(\d+)\s+(\d+)\s+(.+)$/.exec(line)).find(parts => parts && Number(parts[2]) === parent && parts[3].includes('--desktop'));
  assert.ok(child, 'the app owns a separate native authority');
  return {
    pid: Number(child[1]),
    command: child[3]
  };
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
  const notice = instance.page.locator('.desktop-update-notice');
  await notice.getByRole('button', {
    name: 'Download update'
  }).waitFor();
  await instance.page.screenshot({
    path: join(evidence, 'update-prompt.png')
  });
  await notice.getByRole('button', {
    name: 'Later'
  }).click();
  await notice.waitFor({
    state: 'detached'
  });
  assert.ok(instance.state.frame, 'update dismissal leaves the accepted game alive');
  return {
    shown: true,
    dismissedWithoutRestart: true
  };
}
async function close(instance) {
  let timer;
  try {
    await Promise.race([instance.app.close(), new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error(`${instance.label} did not close after saving`)), 30000);
    })]);
  } finally {
    clearTimeout(timer);
  }
  running.delete(instance);
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
