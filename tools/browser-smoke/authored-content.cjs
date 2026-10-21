// Actual local Website package -> Cloud -> Rust hosted authority -> save continuation.
// No socket sends, state setters, synthetic save or mocked content are used. The
// hooks below observe accepted messages and real WebAudio submissions only.
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const {reachCollegeLoadout} = require('./menu-journey.cjs');
const {createNativeMessageReader} = require('./native-wire.cjs');
const output = path.resolve(process.env.OUT);
const contentDirectory = path.resolve(process.env.CONTENT_DIR);
const account = JSON.parse(fs.readFileSync(path.join(contentDirectory, 'private-account.json')));
const acceptedPackage = JSON.parse(fs.readFileSync(path.join(contentDirectory, 'package.json')));
const url = new URL(process.env.URL || 'https://127.0.0.1:9449/game-stg/');
assert.equal(url.hostname, '127.0.0.1', 'this journey uses only the isolated staging backend');
const includeConsole = process.env.CONSOLE === '1';
const witnessConsole = process.env.CONSOLE === 'witness';
const observeConsole = process.env.CONSOLE === 'observe' || witnessConsole;
const continueAuthored = process.env.CONTINUE_AUTHORED === '1';
const authoredPhase = process.env.AUTHORED_PHASE || 'complete';
assert.ok(['complete', 'reforge', 'finalize', 'console'].includes(authoredPhase), 'known authored journey phase');
const state = {id: null, frame: null, runtime: null, content: null, welcomes: [],
  lua: [], luaRequests: [], actions: [], checkpoints: 0, leaveReceipts: 0, lastSave: null,
  audioEvents: [], assets: [], loaded: null, iconProof: [], wireTypes: {}, beltBindings: [],
  cheatCommands: [], cheatModes: [], helloCheats: []};
const errors = [], requests = [], expectedNotFound = [], cancelledMedia = [], checks = [];
let step = 'bootstrap';
let ownedBrowserClosed = false;
const sha256 = value => crypto.createHash('sha256').update(value).digest('hex');
let priorPhase = null;
let priorSackPhase = null;
let priorOnwardPhase = null;
let priorSavePhase = null;
if (authoredPhase === 'reforge') {
  assert.ok(continueAuthored, 'later authored phases continue the real retained profile');
  assert.ok(process.env.PRIOR_AUTHORED_RECEIPT, 'later phases name their accepted earlier receipt');
  const file = path.resolve(process.env.PRIOR_AUTHORED_RECEIPT);
  const bytes = fs.readFileSync(file);
  const receipt = JSON.parse(bytes);
  const check = 'Source local drag admission restores the custom potion without a bind command or inventory change';
  assert.deepEqual(receipt.errors, [], 'the earlier matched-bundle phase has no runtime errors');
  assert.ok(receipt.checks.includes(check), 'the earlier phase proved the actual rejected drag');
  assert.equal(receipt.accepted.tonic, 1, 'the earlier phase kept the real purchased Tonic');
  priorPhase = {file, sha256: sha256(bytes), checks: receipt.checks, accepted: receipt.accepted};
}
if (process.env.PRIOR_SACK_RECEIPT) {
  const file = path.resolve(process.env.PRIOR_SACK_RECEIPT);
  const bytes = fs.readFileSync(file);
  const receipt = JSON.parse(bytes);
  assert.deepEqual(receipt.errors, [], 'the earlier Sack phase has no runtime errors');
  assert.equal(receipt.robeDragProof.ready.page, 'current');
  assert.equal(receipt.robeDragProof.ready.disabled, false);
  assert.ok(receipt.robeDragProof.pointers.some(pointer => pointer.type === 'pointerup'
    && pointer.hit === 'back' && pointer.draggerChildren > 0));
  priorSackPhase = {file, sha256: sha256(bytes), robeDragProof: receipt.robeDragProof};
}
if (authoredPhase === 'finalize' || authoredPhase === 'console') {
  assert.ok(continueAuthored, 'final acceptance continues the real retained run');
  assert.ok(process.env.PRIOR_ONWARD_RECEIPT, 'final acceptance names its earlier actual phases');
  const file = path.resolve(process.env.PRIOR_ONWARD_RECEIPT);
  const bytes = fs.readFileSync(file);
  const receipt = JSON.parse(bytes);
  assert.deepEqual(receipt.errors, [], 'the earlier authored phases have no runtime errors');
  for (const check of [
    'real retained purchase outputs, custom icons, semantic state and new robe affix',
    'actual minimap ping crosses the host rule boundary and submits authored audio',
    'actual custom potion Inventory use commits inventory and source on-use effects',
  ]) assert.ok(receipt.checks.includes(check), `the earlier phase proved ${check}`);
  assert.ok(receipt.accepted.runId, 'the earlier phase entered a real Boneyard');
  assert.equal(receipt.accepted.tonic, 0, 'the actual potion was consumed');
  priorOnwardPhase = {file, sha256: sha256(bytes), checks: receipt.checks,
    accepted: receipt.accepted, priorPhase: receipt.priorPhase, priorSackPhase: receipt.priorSackPhase};
}
if (authoredPhase === 'console') {
  assert.ok(includeConsole, 'the Console-only phase executes the actual Console');
  assert.ok(process.env.PRIOR_SAVE_RECEIPT, 'the Console-only phase names its accepted earlier save');
  const file = path.resolve(process.env.PRIOR_SAVE_RECEIPT);
  const bytes = fs.readFileSync(file);
  const receipt = JSON.parse(bytes);
  assert.equal(receipt.passed, true);
  assert.deepEqual(receipt.errors, []);
  assert.equal(receipt.continued.runId, priorOnwardPhase.accepted.runId);
  priorSavePhase = {file, sha256: sha256(bytes), checks: receipt.checks, continued: receipt.continued};
}

const pause = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
async function bounded(promise, label, milliseconds) {
  let timeout;
  try {
    return await Promise.race([promise, new Promise((resolve, reject) => {
      timeout = setTimeout(() => reject(new Error(`${label} timed out`)), milliseconds);
    })]);
  } finally {
    clearTimeout(timeout);
  }
}
async function until(predicate, label, timeout = 30000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (await predicate()) return;
    await pause(40);
  }
  throw new Error(`Timed out waiting for ${label}`);
}
function mark(label) {step = label; console.log(label);}
function check(label) {checks.push(label); console.log(label);}
function player() {return state.frame?.snapshot.participants[state.id]?.player;}
function position() {return state.frame?.snapshot.participants[state.id]?.wizard.position;}
function items(name) {return player()?.economy.backpack.filter(item => item.name === name) || [];}
function itemQuantity(name) {return items(name).reduce((total, item) => total + item.quantity, 0);}
function firstPurchase() {return state.runtime?.state_values?.some(row =>
  row.key === 'tutorial.first_purchase' && row.value === true && row.scope_id === state.id);}
const snapshot = () => ({tick: state.frame?.snapshot.ticks, runId: state.frame?.snapshot.boneyard?.run_id,
  gold: player()?.economy.gold, moondust: itemQuantity('Moondust'), tonic: itemQuantity('Ward Tonic'),
  firstPurchase: Boolean(firstPurchase()), position: position(), runtimeRevision: state.runtimeRevision,
  host: state.frame?.host});

async function gameplayReady() {
  await until(() => state.frame && !state.frame.host.pause && !state.frame.host.resume_grace
    && !state.frame.host.loading_run_id, 'actual resumed authority and completed resume grace');
}

async function moveTo(page, x, y, admitted) {
  // Courtyard waypoints keep the public input journey outside the desk/fountain.
  const held = new Set();
  const started = Date.now();
  try {
    while (Math.hypot(position().x - x, position().y - y) > 9) {
      if (admitted && await admitted()) return;
      if (Date.now() - started >= 25000) {
        // Preserve admission and velocity while the failing real keys are still
        // held. The outer failure handler runs after finally releases them.
        state.movementFailure = {
          target: {x, y},
          heldKeys: [...held],
          accepted: snapshot(),
          wizard: state.frame.snapshot.participants[state.id].wizard,
          host: {
            pause: state.frame.host.pause,
            loadingRunId: state.frame.host.loading_run_id,
            resumeGrace: state.frame.host.resume_grace,
          },
          browser: await page.evaluate(() => ({
            wizard: Array.from(window.wasmBindings.wizard_state()),
            input: Array.from(window.wasmBindings.gameplay_input_state()),
            focusedElement: {
              tag: document.activeElement?.tagName,
              id: document.activeElement?.id,
              type: document.activeElement?.getAttribute('type'),
            },
            menu: document.querySelector('.game-menus')?.dataset.menuScreen,
            panelsVisible: Boolean(document.querySelector('.player-panels')?.getClientRects().length),
            status: document.querySelector('#status')?.textContent,
          })),
        };
        assert.fail(`movement blocked approaching ${x},${y}`);
      }
      const point = position();
      const desired = new Set();
      if (Math.abs(x - point.x) > 6) desired.add(x > point.x ? 'KeyD' : 'KeyA');
      if (Math.abs(y - point.y) > 6) desired.add(y > point.y ? 'KeyS' : 'KeyW');
      for (const key of held) if (!desired.has(key)) {await page.keyboard.up(key); held.delete(key);}
      for (const key of desired) if (!held.has(key)) {await page.keyboard.down(key); held.add(key);}
      await pause(40);
    }
  } finally {for (const key of held) await page.keyboard.up(key);}
}
async function hiddenMenu(page) {
  await page.locator('.game-menus[data-menu-screen="hidden"]').waitFor({state: 'attached'});
}
async function resumeFromPause(page) {
  const resume = page.locator('[data-menu-action="resume"]:visible');
  await resume.waitFor({state: 'visible'});
  await resume.click();
  await hiddenMenu(page);
}
async function approachApothecary(page) {
  const buy = page.getByRole('button', {name: /Apprentice Apothecary · 8 gold/});
  const admitted = () => buy.isVisible();
  if (await admitted()) return;
  // Source exposes this actual shop button only inside its mounted radius.
  // The east detour is clear for the real retained storeroom approach.
  await moveTo(page, 1510, 740, admitted);
  if (await admitted()) return;
  await moveTo(page, 1450, 740, admitted);
  await buy.waitFor({state: 'visible'});
}
async function openInventory(page) {
  const surface = page.locator('.player-panels:not([hidden]) [data-player-panel="inventory"]');
  if (!await surface.isVisible()) {
    await page.getByRole('button', {name: /Inventory, \d+ gold/}).click();
    await surface.waitFor({state: 'visible'});
  }
  // Source holds input during its 400 ms book reveal, after cells are visible.
  await page.waitForFunction(() => {
    const content = document.querySelector('.player-panel-content');
    return content && getComputedStyle(content).opacity === '1';
  });
  await page.locator('[data-native-inventory-flyby-phase]').waitFor({state: 'detached'});
}
async function assertAuthoredIcon(button) {
  const crop = button.locator('.mod-item-icon .mod-frame-crop').first();
  await crop.waitFor({state: 'visible'});
  const proof = await crop.evaluate(node => {
    const bounds = node.getBoundingClientRect();
    const style = getComputedStyle(node);
    return {background: style.backgroundImage, width: bounds.width, height: bounds.height,
      backgroundPosition: style.backgroundPosition};
  });
  assert.match(proof.background, /^url\(["']?blob:/);
  assert.ok(proof.width > 0 && proof.height > 0, 'the actual authored frame occupies visible pixels');
  state.iconProof.push(proof);
}
async function assertRejectedCustomPotionBinding(page, tonicButton) {
  // Current HubInventoryActions.dropInventorySource restores an unbindable
  // custom item locally. A belt tap activates it; only a drag attempts binding.
  const previousBelt = JSON.stringify(player().belt.entries);
  const previousTonic = JSON.stringify(items('Ward Tonic'));
  const beforeRejectedBinding = state.frame.snapshot.ticks;
  const beforeBindingRequests = state.beltBindings.length;
  const tonicSlot = await tonicButton.boundingBox();
  const beltSlot = await page.locator('[data-panel-action="belt:7"]').boundingBox();
  assert.ok(tonicSlot && beltSlot, 'the actual Inventory item and bank-zero belt target are visible');
  state.rejectedBeltProof = {source: tonicSlot, destination: beltSlot};
  await page.evaluate(() => {
    window.authoredFlybyFrames = [];
    window.authoredDragEvents = [];
    const pointerTypes = ['pointerdown', 'pointermove', 'pointerup',
      'gotpointercapture', 'lostpointercapture', 'pointercancel'];
    const observePointer = event => {
      const bounds = document.querySelector('.player-panel-stage').getBoundingClientRect();
      window.authoredDragEvents.push({type: event.type, pointerId: event.pointerId,
        client: {x: event.clientX, y: event.clientY},
        logical: {x: (event.clientX - bounds.left) * 1600 / bounds.width,
          y: (event.clientY - bounds.top) * 900 / bounds.height},
        target: event.target.closest?.('[data-panel-action]')?.dataset.panelAction || event.target.className,
        hit: document.elementFromPoint(event.clientX, event.clientY)
          ?.closest('[data-panel-action]')?.dataset.panelAction,
        captured: document.querySelector('.player-panels').hasPointerCapture(event.pointerId)});
    };
    for (const type of pointerTypes) document.addEventListener(type, observePointer, true);
    const observer = new MutationObserver(mutations => {
      for (const mutation of mutations) {
        const target = mutation.target;
        const phase = target.getAttribute('data-native-inventory-flyby-phase');
        if (phase) window.authoredFlybyFrames.push({phase,
          tick: target.getAttribute('data-native-inventory-flyby-ticks'),
          mainItems: target.getAttribute('data-native-inventory-flyby-main-items')});
      }
    });
    observer.observe(document.body, {subtree: true, attributes: true,
      attributeFilter: ['data-native-inventory-flyby-phase', 'data-native-inventory-flyby-ticks']});
    window.stopAuthoredFlybyObservation = () => {
      observer.disconnect();
      for (const type of pointerTypes) document.removeEventListener(type, observePointer, true);
      return {flybys: window.authoredFlybyFrames, pointers: window.authoredDragEvents};
    };
  });
  await page.mouse.move(tonicSlot.x + tonicSlot.width / 2, tonicSlot.y + tonicSlot.height / 2);
  await page.mouse.down();
  await page.mouse.move(beltSlot.x + beltSlot.width / 2, beltSlot.y + beltSlot.height / 2, {steps: 15});
  await page.waitForFunction(() => document.querySelector('.player-panel-dragger')?.childElementCount > 0);
  await page.mouse.up();
  await until(() => state.frame.snapshot.ticks > beforeRejectedBinding + 30,
    'later authoritative ticks after the local binding rejection');
  const {flybys, pointers} = await page.evaluate(() => window.stopAuthoredFlybyObservation());
  state.rejectedBeltProof = {...state.rejectedBeltProof, flybys, pointers, beforeTick: beforeRejectedBinding,
    afterTick: state.frame.snapshot.ticks, bindingCommands: state.beltBindings.length - beforeBindingRequests};
  assert.ok(flybys.some(frame => frame.phase === 'flying'), 'the actual item restoration flew back');
  await tonicButton.waitFor({state: 'visible'});
  assert.equal(state.beltBindings.length, beforeBindingRequests,
    'Source local admission rejects the custom item before any BindBelt command');
  assert.equal(JSON.stringify(player().belt.entries), previousBelt);
  assert.equal(JSON.stringify(items('Ward Tonic')), previousTonic);
  assert.equal(itemQuantity('Ward Tonic'), 1);
  check('Source local drag admission restores the custom potion without a bind command or inventory change');
}
async function runLua(page, code) {
  const before = state.luaRequests.length;
  await page.locator('#cheat-menu-lua-editor').fill(code);
  await page.locator('[data-cheat-action="run"]').click();
  await until(() => state.luaRequests.slice(before).some(request => request.code === code),
    'actual Console request');
  const request = state.luaRequests.slice(before).find(request => request.code === code);
  await until(() => state.lua.some(response => response.request_id === request.requestId),
    'matching authoritative Lua response');
  const response = state.lua.find(response => response.request_id === request.requestId);
  await page.locator(`[data-console-result="${response.ok ? 'ok' : 'error'}"]`).first().waitFor();
  return response;
}

(async () => {
  fs.mkdirSync(output, {recursive: true});
  const server = await chromium.launchServer({headless: true, host: '127.0.0.1',
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
  const browser = await chromium.connect(server.wsEndpoint());
  const context = await browser.newContext({ignoreHTTPSErrors: true, viewport: {width: 1280, height: 720}});
  await context.addInitScript(token => {
    localStorage.setItem('sdr.token:/game-stg', token);
    window.authoredAudioStarts = [];
    const decodedAssets = new WeakMap();
    const decode = AudioContext.prototype.decodeAudioData;
    AudioContext.prototype.decodeAudioData = function (...args) {
      // Observe encoded identity without changing the returned decode promise or
      // its callbacks. Native decode may detach the input, so copy it first.
      const header = new Uint8Array(args[0], 0, Math.min(4, args[0].byteLength));
      const ogg = header[0] === 79 && header[1] === 103 && header[2] === 103 && header[3] === 83;
      const identity = {sha256: null};
      if (ogg) {
        const encoded = args[0].slice(0);
        crypto.subtle.digest('SHA-256', encoded).then(bytes => {
          identity.sha256 = [...new Uint8Array(bytes)].map(value => value.toString(16).padStart(2, '0')).join('');
        });
      }
      const result = decode.apply(this, args);
      result.then(buffer => decodedAssets.set(buffer, identity), () => {});
      return result;
    };
    const start = AudioBufferSourceNode.prototype.start;
    AudioBufferSourceNode.prototype.start = function (...args) {
      window.authoredAudioStarts.push({duration: this.buffer?.duration, loop: this.loop,
        identity: decodedAssets.get(this.buffer)});
      return start.apply(this, args);
    };
  }, account.token);
  const page = await context.newPage();
  page.setDefaultTimeout(30000);
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error') return;
    const location = message.location().url;
    if (message.text().includes('404') && (/\/api\/game\/saves\//.test(location) || location.endsWith('/api/game/rejoin'))) return;
    errors.push(message.text());
  });
  page.on('requestfailed', request => {
    const receipt = `${new URL(request.url()).pathname}: ${request.failure()?.errorText}`;
    if (request.resourceType() === 'media' && request.failure()?.errorText === 'net::ERR_ABORTED') cancelledMedia.push(receipt);
    else errors.push(receipt);
  });
  page.on('response', response => {
    const pathname = new URL(response.url()).pathname;
    if (pathname.includes('/api/')) requests.push({path: pathname, status: response.status(), method: response.request().method()});
    if (response.status() === 404 && (/\/api\/game\/saves\//.test(pathname) || pathname.endsWith('/api/game/rejoin'))) expectedNotFound.push(pathname);
    else if (response.status() >= 400) errors.push(`${response.status()} ${pathname}`);
    if (/\/api\/game\/content\//.test(pathname) && response.ok()) state.assets.push(pathname.split('/').at(-1));
  });
  page.on('websocket', socket => {
    const readMessage = createNativeMessageReader();
    socket.on('framesent', ({payload}) => {
      const message = JSON.parse(payload.toString());
      if (message.type === 'hosted' && message.command.kind === 'lua_execute') {
        state.luaRequests.push({requestId: message.command.request_id, code: message.command.code});
      }
      if (message.type === 'hosted' && message.command.kind === 'mod_action') {
        const {action, target, arguments: args} = message.command;
        state.actions.push({action, target, arguments: args});
      }
      if (message.type === 'hosted' && message.command.kind === 'hello') {
        state.helloCheats.push({enabled: message.command.hello.cheats_enabled});
      }
      if (message.type === 'hosted' && message.command.kind === 'set_cheats') {
        state.cheatCommands.push({enabled: message.command.enabled, tick: state.frame?.snapshot.ticks});
      }
      if (message.type === 'command' && message.command.kind === 'bind_belt') {
        state.beltBindings.push(message.command);
      }
    });
    socket.on('framereceived', ({payload}) => {
      const raw = JSON.parse(payload.toString());
      state.wireTypes[raw.type] = (state.wireTypes[raw.type] || 0) + 1;
      const message = readMessage(raw);
      if (message === null) return;
      if (message.type === 'welcome') {
        state.id = message.credentials.player_id;
        state.frame = message.frame;
        state.welcomes.push(message.hosted);
      } else if (message.type === 'snapshot') state.frame = message.frame;
      else if (message.type === 'social') state.social = message.snapshot;
      else if (message.type === 'boneyard_loaded') state.loaded = message.boneyard;
      else if (message.type === 'error') errors.push(`host ${message.code}: ${message.reason}`);
      else if (message.type === 'hosted') {
        const event = message.message;
        if (event.kind === 'mod_runtime') {
          state.runtime = event.projection;
          state.runtimeRevision = event.revision;
          for (const row of event.projection.presentation_events || []) {
            if (!state.audioEvents.some(event => event.owner === row.owner)) state.audioEvents.push(row);
          }
        } else if (event.kind === 'mod_content') state.content = event.projection;
        else if (event.kind === 'cheat_mode') state.cheatModes.push({enabled: event.enabled,
          tick: state.frame?.snapshot.ticks});
        else if (event.kind === 'lua_result') state.lua.push(event);
        else if (event.kind === 'save_checkpoint') {state.checkpoints++; state.lastSave = event.save;}
        else if (event.kind === 'save_before_leave') state.leaveReceipts++;
      }
    });
  });
  try {
    await page.goto(url.href, {waitUntil: 'load'});
    await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0 && document.querySelector('#status')?.textContent === '', null, {timeout: 60000});
    const prompt = page.locator('.title-prompt:not([hidden])');
    const consent = page.getByRole('dialog', {name: 'Local play is active', exact: true});
    if (await prompt.isVisible()) await prompt.locator('[data-menu-action="prompt-secondary"]').click();
    if (!priorOnwardPhase) {
      mark('enable the real uploaded package through Cloud');
      await page.getByRole('button', {name: 'Explore the Dark Cloud', exact: true}).click();
      await page.locator('.dark-cloud-rows[aria-busy="false"]').waitFor();
      const subscribe = page.locator(`[data-menu-action="cloud-subscription:subscribe:${acceptedPackage.slug}"]`);
      let activatedHere = false;
      if (await subscribe.isVisible()) {await subscribe.click(); activatedHere = true;}
      else {
        const enable = page.locator(`[data-menu-action="cloud-subscription:enable:${acceptedPackage.slug}"]`);
        if (await enable.isVisible()) {await enable.click(); activatedHere = true;}
      }
      await page.locator(`[data-mod-slug="${acceptedPackage.slug}"] .dark-cloud-row-state.enabled`).waitFor();
      await page.screenshot({path: path.join(output, 'authored-enabled.png')});
      check(activatedHere ? 'actual account Cloud subscription enables the unchanged uploaded Source package'
        : 'retained Source package remains enabled after its earlier actual Cloud activation');
      await page.keyboard.press('Escape');
      await page.getByRole('button', {name: 'MAIN MENU', exact: true}).click();
    }
    await page.locator('[data-menu-action="play"]').click();
    if (continueAuthored) {
      await page.locator('[data-menu-action="continue"]').click();
      await until(async () => player() || await consent.isVisible(), 'actual retained authored profile');
      if (await consent.isVisible()) {
        await consent.getByRole('button', {name: 'CONTINUE LOCAL', exact: true}).click();
      }
      await hiddenMenu(page);
      await until(() => player() && !state.frame.host.loading_run_id && !state.frame.host.resume_grace,
        'actual retained authority readiness');
      state.continuedProfile = snapshot();
    } else {
      await page.locator('[data-menu-action="new-game"]').click();
      await until(async () => await consent.isVisible() || await prompt.isVisible(),
        'local content consent or the retained QA character replacement prompt');
      if (await prompt.isVisible()) {
        assert.match(await prompt.innerText(), /Kill character\?/i);
        await prompt.locator('[data-menu-action="prompt-primary"]').click();
      }
      await consent.waitFor();
      assert.ok((await consent.innerText()).includes('Apprentice Apothecary'));
      await consent.getByRole('button', {name: 'CONTINUE LOCAL', exact: true}).click();
      await until(async () => await page.locator('.game-menus[data-menu-screen="create"]').count()
        || await page.locator('.player-panels:not([hidden]) [data-player-panel="dialogue"]').count(), 'real Create or College admission');
      if (await page.locator('.player-panels:not([hidden]) [data-player-panel="dialogue"]').count()) await reachCollegeLoadout(page);
      await page.getByRole('textbox', {name: 'Wizard name', exact: true}).fill('PipStudent');
      await page.locator('[data-menu-action="element:ether"]').click();
      await page.locator('[data-menu-action="discipline:arcane"]').click();
      await hiddenMenu(page);
    }
    await until(() => state.runtime?.shops?.length && player(), 'real accepted content and runtime');
    assert.equal(state.welcomes.at(-1).session_kind, 'PrivateCollege');
    const identity = state.welcomes.at(-1).content.mods.find(mod => mod.id === acceptedPackage.packageId);
    assert.ok(identity, 'the authority accepted the enabled package identity');
    assert.equal(identity.contentSha256, acceptedPackage.contentSha256);
    check('actual package identity admits a private authority and nonempty prepared runtime');
    let runId;
    if (priorOnwardPhase) {
      const accepted = priorOnwardPhase.accepted;
      assert.equal(state.frame.snapshot.boneyard?.run_id, accepted.runId);
      assert.equal(player().economy.gold, accepted.gold);
      assert.equal(itemQuantity('Moondust'), accepted.moondust);
      assert.equal(itemQuantity('Ward Tonic'), accepted.tonic);
      assert.ok(firstPurchase());
      runId = accepted.runId;
      check('the real retained authored run continues after the independently proven earlier phases');
    } else {
      mark(continueAuthored ? 'continue with the real purchased authored items' : 'walk to Pip and purchase through Source ModPanels');
      if (continueAuthored) {
        assert.equal(itemQuantity('Ward Tonic'), 1, 'the saved profile retains the purchased Tonic');
        assert.equal(itemQuantity('Moondust'), 3, 'the saved profile retains actual purchase outputs');
        assert.ok(firstPurchase(), 'the real prepared profile state survived Continue');
      } else {
        // The native north arch blocks a direct southward walk (hub.rs's source
        // regression covers it). Use the western stair corridor; each segment was
        // checked with Source isPathTraversableAgainstHubSegments at radius25.
        for (const [x, y] of [[840, 195], [810, 195], [720, 345], [735, 465],
          [1450, 740]]) await moveTo(page, x, y);
        const beforeGold = player().economy.gold;
        await page.getByRole('button', {name: /Apprentice Apothecary · 8 gold/}).click();
        await until(() => itemQuantity('Moondust') === 2 && firstPurchase(), 'actual purchase plus Lua grant and semantic state');
        assert.equal(player().economy.gold, beforeGold - 8);
        assert.ok(state.audioEvents.some(event => event.path === 'audio/handleCoins.ogg'));
        const buyTonic = page.getByRole('button', {name: /Apprentice Apothecary · 30 gold/});
        if (await buyTonic.isDisabled()) {
          mark('wait for the real retained shop stock to restock');
          await until(() => buyTonic.isEnabled(), 'the actual two-minute shop restock', 180000);
        }
        await buyTonic.click();
        await until(() => itemQuantity('Ward Tonic') === 1 && itemQuantity('Moondust') === 3, 'custom potion inventory grant');
      }
      if (priorPhase) {
        check('matched-bundle prior receipt retains the proven custom potion rejection/restoration');
      } else {
        await openInventory(page);
        const tonicButton = page.getByRole('button', {name: 'Ward Tonic, quantity 1', exact: true});
        await assertAuthoredIcon(tonicButton);
        await assertAuthoredIcon(page.getByRole('button', {name: 'Moondust, quantity 3', exact: true}));
        await assertRejectedCustomPotionBinding(page, tonicButton);
        await page.screenshot({path: path.join(output, 'authored-inventory.png')});
        await page.locator('[data-panel-action="close"]').first().click();
        await page.locator('.player-panels').waitFor({state: 'hidden'});
      }
      // Source forbids removing worn robes. The earlier real character replacement
      // left its gear in Luthacus's scavenged bag; retrieve that actual spare robe.
      // ModPanels reforges the first root-backpack equipment, so move only the robe
      // out of the bag through the visible Back target before returning to Pip.
      const isRobe = item => String(item.equipment_type).toLowerCase() === 'robe';
      const rootRobe = player().economy.backpack.find(isRobe);
      const hasSpareRobe = item => item.contents?.some(isRobe);
      const bag = player().economy.backpack.find(hasSpareRobe)
        || player().economy.storage.find(hasSpareRobe);
      assert.ok(rootRobe || bag, 'the retained profile supplies its real scavenged robe');
      const robeId = rootRobe?.id ?? bag.contents.find(isRobe).id;
      state.spareRobeProof = {robeId, location: rootRobe ? 'root backpack' : 'scavenged bag'};
      if (rootRobe) {
        // A prior ordinary child→Back drag already moved this item. Continue from
        // the actual accepted inventory instead of repeating that passed phase.
        check('the real spare robe remains in the root backpack for the remaining reforge phase');
      } else {
        if (!player().economy.backpack.some(item => item.id === bag.id)) {
          await moveTo(page, 1640, 520);
          await page.getByRole('button', {name: 'Luthacus', exact: true}).click();
          await page.locator('.player-panels:not([hidden]) [data-player-panel="luthacus"]').waitFor();
          await page.locator(`[data-panel-action="storage:${bag.id}"]`).dblclick();
          await until(() => player().economy.backpack.some(item => item.id === bag.id),
            'actual scavenged bag transfer');
          await page.locator('[data-panel-action="close"]').first().click();
          await page.locator('.player-panels').waitFor({state: 'hidden'});
        }
        await openInventory(page);
        await page.locator(`[data-panel-action="item:${bag.id}"]`).dblclick();
        // Child cells appear during the Source sack slide, while input is locked.
        await page.locator('[data-native-sack-page-state]').waitFor({state: 'detached'});
        const robeButton = page.locator(`[data-panel-action="item:${robeId}"]`);
        await robeButton.waitFor({state: 'visible'});
        const robeAdmission = () => robeButton.evaluate(button => ({
          disabled: button.disabled,
          page: button.closest('[data-native-sack-page]')?.getAttribute('data-native-sack-page'),
          pageState: document.querySelector('.player-panel-stage')?.dataset.nativeSackPageState,
          bookOpacity: getComputedStyle(document.querySelector('.player-panel-content')).opacity,
          flyby: Boolean(document.querySelector('[data-native-inventory-flyby-phase]')),
        }));
        state.robeDragProof = {before: await robeAdmission()};
        // Visible incoming pages are disabled. Wait for the real current/enabled
        // page, including a slide that has not started in the frame after dblclick.
        await until(async () => {
          const admission = await robeAdmission();
          return !admission.disabled && admission.page === 'current'
            && admission.bookOpacity === '1' && !admission.flyby;
        }, 'enabled robe on the settled current Sack page');
        state.robeDragProof.ready = await robeAdmission();
        const robeSlot = await robeButton.boundingBox();
        const backTarget = page.locator('[data-panel-action="back"]');
        const backSlot = await backTarget.boundingBox();
        assert.ok(robeSlot && backSlot, 'the actual open bag exposes its robe and Back target');
        state.robeDragProof.source = robeSlot;
        state.robeDragProof.destination = backSlot;
        await page.evaluate(() => {
          window.authoredRobePointers = [];
          const observe = event => {
            const hit = document.elementFromPoint(event.clientX, event.clientY);
            window.authoredRobePointers.push({type: event.type,
              client: {x: event.clientX, y: event.clientY},
              target: event.target.closest?.('[data-panel-action]')?.dataset.panelAction,
              hit: hit?.closest('[data-panel-action]')?.dataset.panelAction,
              captured: document.querySelector('.player-panels').hasPointerCapture(event.pointerId),
              draggerChildren: document.querySelector('.player-panel-dragger')?.childElementCount});
          };
          for (const type of ['pointerdown', 'pointermove', 'pointerup']) {
            document.addEventListener(type, observe, true);
          }
          window.stopAuthoredRobeObservation = () => {
            for (const type of ['pointerdown', 'pointermove', 'pointerup']) {
              document.removeEventListener(type, observe, true);
            }
            return window.authoredRobePointers;
          };
        });
        await page.mouse.move(robeSlot.x + robeSlot.width / 2, robeSlot.y + robeSlot.height / 2);
        await page.mouse.down();
        await page.mouse.move(backSlot.x + backSlot.width / 2, backSlot.y + backSlot.height / 2, {steps: 12});
        await page.waitForFunction(() => document.querySelector('.player-panel-dragger')?.childElementCount > 0);
        await page.mouse.up();
        state.robeDragProof.pointers = await page.evaluate(() => window.stopAuthoredRobeObservation());
        await until(() => player().economy.backpack.some(item => item.id === robeId),
          'actual spare robe movement into the root backpack');
        state.spareRobeProof.movedToRoot = true;
        await page.locator('[data-native-inventory-flyby-phase]').waitFor({state: 'detached'});
        // The footer closes the book directly; Back starts another Sack slide and
        // an immediate Escape can be consumed by that still-active transition.
        await page.locator('[data-panel-action="close"]').first().click();
        await page.locator('.player-panels').waitFor({state: 'hidden'});
      }
      await approachApothecary(page);
      const goldBeforeReforge = player().economy.gold;
      await page.getByRole('button', {name: /Apprentice Apothecary · Reforge · 25 gold/}).click();
      await until(() => player().economy.backpack.some(item => item.id === robeId
        && item.mod_affixes?.some(affix => affix.name === 'Clear Minded')), 'actual Clear Minded reforge');
      assert.equal(player().economy.gold, goldBeforeReforge - 25);
      await openInventory(page);
      await page.locator(`[data-panel-action="item:${robeId}"]`).dblclick();
      await until(() => !player().economy.backpack.some(item => item.id === robeId), 'actual robe re-equip');
      await page.screenshot({path: path.join(output, 'authored-affix.png')});
      await page.keyboard.press('Escape');
      check(continueAuthored
        ? 'real retained purchase outputs, custom icons, semantic state and new robe affix'
        : 'real stock, gold, custom icons, purchase rule, bonus item, semantic state, robe affix and authored audio');
      mark('enter an ordinary Boneyard and activate the authored minimap');
      await page.getByRole('button', {name: 'Enter the Boneyard', exact: true}).click();
      await until(() => state.frame?.snapshot.boneyard && !state.frame.host.loading_run_id && !state.frame.host.resume_grace && !state.frame.host.pause, 'actual Boneyard readiness', 120000);
      runId = state.frame.snapshot.boneyard.run_id;
      const minimap = page.getByRole('region', {name: 'Apprentice survey minimap', exact: true});
      await minimap.waitFor();
      const beforeAudio = await page.evaluate(() => window.authoredAudioStarts.length);
      const beforePresentation = new Set(state.audioEvents.map(event => event.owner));
      await minimap.locator('[data-mod-minimap-action="ping"]').click();
      await until(() => state.actions.some(action => action.action === 'ui-action') && state.audioEvents.some(event => event.path === 'audio/bookOpen.ogg' && !beforePresentation.has(event.owner)), 'actual authored UI rule and sound event');
      const pageSoundHash = state.welcomes.at(-1).mod_assets.find(asset => asset.path === 'audio/bookOpen.ogg').sha256;
      await until(() => page.evaluate(({count, hash}) => window.authoredAudioStarts.slice(count)
        .some(source => source.identity?.sha256 === hash), {count: beforeAudio, hash: pageSoundHash}),
        'the actual authored audio buffer submission');
      state.authoredAudioProof = await page.evaluate(({count, hash}) => window.authoredAudioStarts.slice(count)
        .filter(source => source.identity?.sha256 === hash), {count: beforeAudio, hash: pageSoundHash});
      await page.screenshot({path: path.join(output, 'authored-minimap.png')});
      check('actual minimap ping crosses the host rule boundary and submits authored audio');
      mark('consume the actual custom potion');
      await openInventory(page);
      await page.getByRole('button', {name: 'Ward Tonic, quantity 1', exact: true}).dblclick();
      await until(() => itemQuantity('Ward Tonic') === 0 && state.content?.statuses?.some(status =>
        status.targetId === state.id), 'real custom potion consume and on-use status');
      const appliedStatus = state.content.statuses.find(status => status.targetId === state.id);
      assert.equal(appliedStatus.expiresTick - appliedStatus.startedTick, 2000,
        'the accepted 20-second Source status is applied by the real rule');
      await page.keyboard.press('Escape');
      await page.locator('.player-panels').waitFor({state: 'hidden'});
      check('actual custom potion Inventory use commits inventory and source on-use effects');
    }
    if (includeConsole || observeConsole) {
      mark('execute real authoritative Console work');
      await page.keyboard.press('Escape');
      await page.locator('[data-menu-action="pause-settings"]').click();
      await page.locator('[data-menu-action="settings-page:cloud"]').click();
      const cheats = page.locator('[data-menu-action="settings-toggle:cheats"]');
      state.cheatProof = {before: await cheats.getAttribute('aria-pressed')};
      if (await cheats.getAttribute('aria-pressed') === 'false') await cheats.click();
      await pause(1000);
      if (witnessConsole) {
        let replyCount = state.cheatModes.length;
        await cheats.click();
        await until(() => state.cheatModes.slice(replyCount).some(reply => !reply.enabled),
          'actual disabled cheat-mode reply');
        replyCount = state.cheatModes.length;
        await cheats.click();
        await until(() => state.cheatModes.slice(replyCount).some(reply => reply.enabled),
          'actual re-enabled cheat-mode reply');
      }
      state.cheatProof.after = await cheats.getAttribute('aria-pressed');
      state.cheatProof.disabled = await cheats.isDisabled();
      const welcome = state.welcomes.at(-1);
      const party = state.social?.party_state?.party;
      state.cheatProof.admission = {
        localPlayerId: state.id,
        authorityPlayerId: state.frame.host.authority_player_id,
        connectedPlayerIds: state.frame.host.connected_player_ids,
        sessionKind: welcome.session_kind,
        developerAccess: welcome.developer_access,
        welcomeCheatsEnabled: welcome.cheats_enabled,
        helloCheats: state.helloCheats,
        commands: state.cheatCommands,
        replies: state.cheatModes,
        party: party && {id: party.id, leaderPlayerId: party.leader_player_id,
          memberPlayerIds: party.member_player_ids, visibility: party.visibility},
      };
      fs.writeFileSync(path.join(output, 'console-admission.json'), JSON.stringify(state.cheatProof, null, 2));
      await page.locator('[data-menu-action="settings-back"]').click();
      await page.locator('[data-settings-page="root"]').waitFor({state: 'visible'});
      await page.locator('[data-menu-action="settings-back"]').click();
      await page.locator('.game-menus[data-menu-screen="pause"]').waitFor({state: 'visible'});
      state.cheatProof.pauseRowVisible = await page.locator('[data-menu-action="pause-cheats"]').isVisible();
      state.cheatProof.pauseRows = await page.locator('.game-menus [data-menu-action]:visible').evaluateAll(
        buttons => buttons.map(button => ({action: button.dataset.menuAction, label: button.getAttribute('aria-label')})));
      fs.writeFileSync(path.join(output, 'console-admission.json'), JSON.stringify(state.cheatProof, null, 2));
      if (witnessConsole) {
        assert.equal(state.cheatProof.admission.authorityPlayerId, state.id);
        assert.equal(state.cheatModes.at(-1).enabled, true);
        assert.equal(state.cheatProof.after, 'true');
        const expectedRow = process.env.EXPECT_CHEAT_ROW === 'present';
        assert.equal(state.cheatProof.pauseRowVisible, expectedRow);
        await page.screenshot({path: path.join(output, 'console-row-witness.png')});
        fs.writeFileSync(path.join(output, 'console-row-witness.json'), JSON.stringify({
          matched: true, expectedRow, errors, admission: state.cheatProof, accepted: snapshot()}, null, 2));
        assert.deepEqual(errors, []);
        return;
      }
      if (includeConsole) await page.locator('[data-menu-action="pause-cheats"]').waitFor({state: 'visible'});
      await resumeFromPause(page);
      await gameplayReady();
      if (includeConsole) {
        await page.keyboard.press('Backquote');
        await page.locator('.cheat-menu-backdrop').waitFor({state: 'visible'});
        await page.locator('[data-cheat-action="console"]').click();
        await page.locator('#cheat-menu-lua-editor').waitFor({state: 'visible'});
        let result = await runLua(page, "return _VERSION, sd.runtime.api_version, sd.player.get_state().gold, type(sd.dev)");
        assert.equal(result.ok, true, result.error);
        assert.deepEqual(result.values, ['Lua 5.4', '1.0.0', player().economy.gold, 'nil']);
        result = await runLua(page, "sd.state.set('parity.counter',41); sd.timer.after(10,function() sd.state.set('parity.counter',42) end); return true");
        assert.equal(result.ok, true, result.error);
        await pause(40);
        result = await runLua(page, "return sd.state.get('parity.counter'), sd.state.is_authority(), type(io), type(sd.events.filter)");
        assert.equal(result.ok, true, result.error);
        assert.deepEqual(result.values, [42, true, 'nil', 'nil']);
        const gold = player().economy.gold - 1;
        result = await runLua(page, `return sd.player.set_gold(${gold})`);
        assert.equal(result.ok, true, result.error);
        await until(() => player().economy.gold === gold, 'actual Console mutation');
        result = await runLua(page, 'while true do end');
        assert.equal(result.ok, false);
        assert.match(result.error, /thread timeout exceeded/);
        result = await runLua(page, 'return 42');
        assert.equal(result.ok, true, result.error);
        assert.deepEqual(result.values, [42]);
        await page.screenshot({path: path.join(output, 'authored-console.png')});
        await page.keyboard.press('Backquote');
        await page.locator('.cheat-menu-backdrop').waitFor({state: 'hidden'});
        check('real Lua values, state, timer, sandbox, queued mutation and timeout recovery keep the client alive');
      }
    }
    if (authoredPhase === 'console') {
      assert.deepEqual(errors, []);
      fs.writeFileSync(path.join(output, 'authored-console.json'), JSON.stringify({
        passed: true, source: acceptedPackage.source, package: acceptedPackage,
        checks, errors, priorOnwardPhase, priorSavePhase, consoleExecuted: true,
        cheatProof: state.cheatProof, accepted: snapshot(),
        lua: state.lua.map(({request_id, ok, output, values, error}) =>
          ({request_id, ok, output, values, error})),
      }, null, 2));
      return;
    }
    mark('save and resume the same authored run');
    const beforeLeave = state.leaveReceipts;
    await gameplayReady();
    await page.locator('[data-hud-action="pause"]').click();
    await page.locator('.game-menus[data-menu-screen="pause"]').waitFor({state: 'visible'});
    await page.locator('[data-menu-action="leave"]').click();
    await page.locator('.game-menus[data-menu-screen="title"]').waitFor();
    assert.ok(state.leaveReceipts > beforeLeave);
    assert.ok(state.lastSave, 'the actual host supplied its signed source continuation');
    const saved = JSON.parse(state.lastSave);
    const modState = saved.modState;
    assert.ok(modState && Object.keys(modState).length, 'accepted authored runtime reaches the real save');
    const beforeReload = snapshot();
    state.frame = null; state.runtime = null;
    await page.reload({waitUntil: 'load'});
    await page.waitForFunction(() => document.querySelector('#status')?.textContent === '', null, {timeout: 60000});
    await page.locator('[data-menu-action="play"]').click();
    await page.locator('[data-menu-action="continue"]').click();
    await until(async () => state.frame?.snapshot.boneyard?.run_id === runId || await consent.isVisible(),
      'actual resume admission or local-content consent', 120000);
    if (await consent.isVisible()) await consent.getByRole('button', {name: 'CONTINUE LOCAL', exact: true}).click();
    await until(() => state.frame?.snapshot.boneyard?.run_id === runId && firstPurchase() && itemQuantity('Moondust') === 3 && !state.frame.host.loading_run_id && !state.frame.host.resume_grace, 'real saved run and prepared state continuation', 120000);
    assert.equal(player().selection.name, 'PipStudent');
    assert.equal(player().economy.gold, beforeReload.gold);
    const resumedTick = state.frame.snapshot.ticks;
    await until(() => state.frame.snapshot.ticks > resumedTick + 30, 'continued authored authority');
    await page.screenshot({path: path.join(output, 'authored-resumed.png')});
    check('real checkpoint persistence and same-run reload retain inventory, semantic state and later authority ticks');
    assert.deepEqual(errors, []);
    const receipt = {passed: true, package: acceptedPackage, checks, requests, expectedNotFound,
      continuedProfile: state.continuedProfile, priorPhase, priorSackPhase, priorOnwardPhase,
      cheatProof: state.cheatProof, consoleExecuted: includeConsole,
      cancelledMedia, errors, actions: state.actions, assets: [...new Set(state.assets)],
      lua: state.lua.map(({request_id, ok, output, values, error}) => ({request_id, ok, output, values, error})),
      savedModStateSha256: sha256(JSON.stringify(modState)), continued: snapshot(),
      audioEvents: state.audioEvents, authoredAudioProof: state.authoredAudioProof, iconProof: state.iconProof,
      rejectedBeltProof: state.rejectedBeltProof,
      robeDragProof: state.robeDragProof,
      spareRobeProof: state.spareRobeProof,
      webAudioStarts: await page.evaluate(() => window.authoredAudioStarts)};
    fs.writeFileSync(path.join(output, 'authored-content.json'), JSON.stringify(receipt, null, 2));
    console.log(JSON.stringify({passed: true, checks, errors}));
  } catch (error) {
    await page.screenshot({path: path.join(output, 'authored-content-failure.png')});
    fs.writeFileSync(path.join(output, 'authored-content-failure.json'), JSON.stringify({step, error: error.stack, checks, errors, requests, expectedNotFound, continuedProfile: state.continuedProfile, priorPhase, priorSackPhase, priorOnwardPhase, accepted: snapshot(), wireTypes: state.wireTypes, movementFailure: state.movementFailure, rejectedBeltProof: state.rejectedBeltProof,
      cheatProof: state.cheatProof,
      spareRobeProof: state.spareRobeProof,
      robeDragProof: state.robeDragProof ? {...state.robeDragProof,
        pointers: await page.evaluate(() => window.authoredRobePointers || [])} : undefined,
      ui: await page.evaluate(() => ({menu: document.querySelector('.game-menus')?.dataset.menuScreen, status: document.querySelector('#status')?.textContent, text: document.body.innerText.slice(-3000)}))}, null, 2));
    throw error;
  } finally {
    const cleanup = {};
    await bounded(browser.close(), 'browser connection close', 5000).catch(error => {
      cleanup.connectionError = error.message;
    });
    try {
      await bounded(server.close(), 'owned Chrome close', 15000);
      cleanup.mode = 'graceful';
    } catch (error) {
      cleanup.error = error.message;
      await bounded(server.kill(), 'owned Chrome termination', 5000).catch(error => {
        cleanup.terminationError = error.message;
      });
      cleanup.mode = 'forced';
    }
    ownedBrowserClosed = server.process().exitCode !== null || server.process().signalCode !== null;
    cleanup.browserClosed = ownedBrowserClosed;
    fs.writeFileSync(path.join(output, 'browser-cleanup.json'), JSON.stringify(cleanup, null, 2));
    assert.ok(ownedBrowserClosed, 'the owned Chrome process exited');
  }
})().then(() => {
  // Playwright's server socket can outlive its already-exited Chrome child.
  // Finish only after the cleanup receipt confirms that child is closed.
  if (ownedBrowserClosed) process.exit(0);
}).catch(error => {
  console.error(error.message);
  if (ownedBrowserClosed) process.exit(1);
  process.exitCode = 1;
});
