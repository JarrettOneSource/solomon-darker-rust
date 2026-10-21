// Real hosted players, stock Console commands and combat death. Socket/GPU hooks
// only observe; no protocol message, saved document or gameplay field is injected.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(process.env.PW);
const {createNativeMessageReader} = require('./native-wire.cjs');
const {walkSolomonEntrance} = require('./solomon-entrance.cjs');

const url = process.env.URL || 'https://127.0.0.1:9449/game-stg/';
const output = path.resolve(process.env.OUT);
const leaderAccount = JSON.parse(fs.readFileSync(process.env.ACCOUNT));
const peerFile = process.env.PEER_ACCOUNT || path.join(output, 'private-peer-account.json');
const build = process.env.BUILD_MANIFEST
  ? JSON.parse(fs.readFileSync(process.env.BUILD_MANIFEST)) : null;
const clients = [];
const checks = [];
let step = 'registration';

async function until(predicate, label) {
  const deadline = Date.now() + 30000;
  while (Date.now() < deadline) {
    for (const client of clients) {
      if (client.errors.length) throw new Error(`${client.label}: ${client.errors.join('; ')}`);
    }
    if (await predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 40));
  }
  throw new Error(`Timed out: ${label}`);
}

function checked(label) {
  checks.push(label);
  console.log(label);
}

async function peerAccount() {
  if (fs.existsSync(peerFile)) {
    fs.chmodSync(peerFile, 0o600);
    return JSON.parse(fs.readFileSync(peerFile));
  }
  const configuration = JSON.parse(fs.readFileSync(process.env.PRIVATE_CONFIG));
  const username = `Spectator${Date.now().toString(36)}`;
  const email = `${username.toLowerCase()}@example.invalid`;
  const account = {username, email, password: configuration.testPassword};
  fs.writeFileSync(peerFile, JSON.stringify(account, null, 2), {mode: 0o600});
  fs.chmodSync(peerFile, 0o600);
  return account;
}

function observeGpu() {
  const states = new WeakMap();
  const textures = new WeakMap();
  const diagnostic = window.spectatorGpu = {active: false, ground: [], textures: []};
  const prototype = WebGL2RenderingContext.prototype;
  const bind = prototype.bindTexture;
  const image = prototype.texImage2D;
  const upload = prototype.bufferData;
  function state(gl) {
    if (!states.has(gl)) states.set(gl, {texture: null});
    return states.get(gl);
  }
  prototype.bindTexture = function(target, texture) {
    if (target === this.TEXTURE_2D) state(this).texture = texture;
    return Reflect.apply(bind, this, arguments);
  };
  prototype.texImage2D = function() {
    const source = arguments[arguments.length - 1];
    if (source instanceof HTMLImageElement) textures.set(state(this).texture, source.src);
    return Reflect.apply(image, this, arguments);
  };
  prototype.bufferData = function(target, data) {
    if (diagnostic.active && target === this.ARRAY_BUFFER && data instanceof Float32Array) {
      const texture = textures.get(state(this).texture);
      if (texture) {
        const pathname = new URL(texture).pathname;
        if (!diagnostic.textures.includes(pathname)) diagnostic.textures.push(pathname);
        if (pathname.endsWith('/boneyard/ground.png') && data.length >= 48) {
          diagnostic.ground.push({time: performance.now(), x: data[0], y: data[1]});
          if (diagnostic.ground.length > 300) diagnostic.ground.shift();
        }
      }
    }
    return Reflect.apply(upload, this, arguments);
  };
}

async function open(account, label) {
  const server = await chromium.launchServer({headless: true, host: '127.0.0.1',
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
  const browser = await chromium.connect(server.wsEndpoint());
  const context = await browser.newContext({ignoreHTTPSErrors: true,
    viewport: {width: 1600, height: 900}});
  if (account.token) {
    await context.addInitScript(token => localStorage.setItem('sdr.token:/game-stg', token), account.token);
  }
  await context.addInitScript(observeGpu);
  const page = await context.newPage();
  const client = {label, browser, server, page, id: null, frame: null, loaded: null,
    errors: [], cancelledMedia: [], cancelledNavigation: [], accountNavigating: false,
    inputs: [], inputBoundaries: [], movementAttempts: [], lua: [], luaRequests: [],
    flame: {active: false, births: new Map(), frames: [], actors: []},
    checkpoints: 0, leaves: 0, wasm: new Set()};
  clients.push(client);
  page.setDefaultTimeout(30000);
  page.setDefaultNavigationTimeout(30000);
  page.on('pageerror', error => client.errors.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error') return;
    if (message.text().includes('404') && /\/api\/game\/(rejoin|saves\/0)$/.test(message.location().url)) return;
    client.errors.push(message.text());
  });
  page.on('requestfailed', item => {
    const failure = `${new URL(item.url()).pathname}: ${item.failure()?.errorText}`;
    if (item.resourceType() === 'media' && item.failure()?.errorText === 'net::ERR_ABORTED') {
      client.cancelledMedia.push(failure);
    } else if (client.accountNavigating && item.failure()?.errorText === 'net::ERR_ABORTED'
      && !new URL(item.url()).pathname.includes('/api/') && item.resourceType() !== 'document') {
      client.cancelledNavigation.push(failure);
    } else client.errors.push(failure);
  });
  page.on('response', response => {
    const pathname = new URL(response.url()).pathname;
    if (pathname.endsWith('.wasm')) client.wasm.add(path.basename(pathname));
    if (response.status() >= 400
      && !(response.status() === 404 && /\/api\/game\/(rejoin|saves\/0)$/.test(pathname))) {
      client.errors.push(`${response.status()} ${pathname}`);
    }
  });
  page.on('websocket', socket => {
    const read = createNativeMessageReader();
    socket.on('framesent', ({payload}) => {
      const row = JSON.parse(payload.toString());
      if (row.type === 'input') {
        client.inputs.push({sequence: row.sequence, primary: row.input.primary,
          quickbar: row.input.quickbar, movement: row.input.movement, aim: row.input.aim});
        if (client.inputs.length > 300) client.inputs.shift();
      } else if (row.type === 'hosted' && row.command.kind === 'lua_execute') {
        client.luaRequests.push(row.command);
      }
    });
    socket.on('framereceived', ({payload}) => {
      const row = read(JSON.parse(payload.toString()));
      if (!row) return;
      if (row.type === 'welcome') {
        client.id = row.credentials.player_id;
        client.frame = row.frame;
      } else if (row.type === 'snapshot') client.frame = row.frame;
      else if (row.type === 'boneyard_loaded') client.loaded = row.boneyard;
      else if (row.type === 'error') client.errors.push(`host ${row.code}: ${row.reason}`);
      else if (row.type === 'hosted') {
        if (row.message.kind === 'lua_result') client.lua.push(row.message);
        if (row.message.kind === 'save_checkpoint') client.checkpoints++;
        if (row.message.kind === 'save_before_leave') client.leaves++;
      }
      if ((row.type === 'welcome' || row.type === 'snapshot') && client.flame.active) {
        observeFlame(client);
      }
    });
  });
  await page.goto(url, {waitUntil: 'load'});
  await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0
    && document.querySelector('#status')?.textContent === '');
  client.renderer = await page.evaluate(() => {
    const gl = document.querySelector('canvas').getContext('webgl2');
    const extension = gl.getExtension('WEBGL_debug_renderer_info');
    return extension ? gl.getParameter(extension.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER);
  });
  if (await page.locator('.title-prompt:not([hidden])').isVisible()) {
    await page.locator('.title-prompt:not([hidden]) [data-menu-action="prompt-secondary"]').click();
  }
  if (!account.token) await registerPeer(client, account);
  return client;
}

async function registerPeer(client, account) {
  const page = client.page;
  client.accountNavigating = true;
  const catalog = page.waitForResponse(response => new URL(response.url()).pathname
    === '/game-stg/api/mods' && response.request().method() === 'GET');
  await page.locator('[data-menu-action="explore"]').click();
  const response = await catalog;
  assert.equal(response.status(), 200);
  await response.finished();
  await page.locator('[data-menu-action="cloud-sign-in"]').click();
  await page.waitForURL(current => current.pathname === '/game-stg/login');
  await page.getByRole('link', {name: 'Create an account', exact: true}).click();
  await page.waitForURL(current => current.pathname === '/game-stg/register');
  await page.waitForFunction(() => {
    const reveal = document.querySelector('form')?.closest('.reveal-in');
    return reveal && getComputedStyle(reveal).opacity === '1';
  });
  for (const [autocomplete, value] of [['username', account.username], ['email', account.email],
    ['new-password', account.password]]) {
    const input = page.locator(`input[autocomplete="${autocomplete}"]`);
    await input.pressSequentially(value, {delay: 10});
    await input.press('Tab');
    assert.equal(await input.inputValue(), value);
  }
  const registered = page.waitForResponse(response => new URL(response.url()).pathname
    === '/game-stg/api/auth/register' && response.request().method() === 'POST');
  await page.getByRole('button', {name: 'Create account', exact: true}).click();
  assert.equal((await registered).status(), 201);
  // Current Source Register.tsx navigates to Account, then Shell's real Play
  // link returns to the Rust mount. Registration itself does not open the game.
  await page.waitForURL(current => current.pathname === '/game-stg/account');
  await page.getByRole('link', {name: 'Play', exact: true}).first().click();
  await page.waitForURL(current => current.pathname === '/game-stg/');
  await page.waitForFunction(expected => window.wasmBindings?.wizard_state().length > 0
    && document.querySelector('#status')?.textContent === ''
    && document.querySelector('.game-account-name-title')?.getAttribute('data-account-username') === expected,
  account.username);
  const prompt = page.locator('.title-prompt:not([hidden])');
  if (await prompt.isVisible()) await prompt.locator('[data-menu-action="prompt-secondary"]').click();
  const storage = await page.evaluate(() => ({unscopedAbsent: localStorage.getItem('sdr.token') === null,
    token: localStorage.getItem('sdr.token:/game-stg')}));
  assert.equal(storage.unscopedAbsent, true);
  assert.ok(storage.token);
  account.token = storage.token;
  fs.writeFileSync(peerFile, JSON.stringify(account, null, 2), {mode: 0o600});
  fs.chmodSync(peerFile, 0o600);
  client.registration = {sourceFormStatus: 201, accountThenPlay: true, scopedCredential: true,
    unscopedCredentialAbsent: true};
  client.accountNavigating = false;
}

function participant(client) {
  return client.frame?.snapshot.participants[client.id];
}
function life(client) {
  return participant(client)?.player.combat.life_state.toLowerCase();
}
function castAuthority(client) {
  const row = participant(client);
  return {tick: client.frame?.snapshot.ticks, sequence: client.frame?.sequence,
    acknowledgement: client.frame?.input_acknowledgements[client.id],
    host: client.frame?.host, barrier: client.frame?.snapshot.level_up_barrier,
    encounter: client.frame?.snapshot.boneyard?.encounter && {
      phase: client.frame.snapshot.boneyard.encounter.phase,
      runEventId: client.frame.snapshot.boneyard.encounter.run_event_id,
      position: client.frame.snapshot.boneyard.encounter.position},
    pendingOffer: row?.player.progression.pending_offer,
    life: row?.player.combat.life_state, mana: row?.player.combat.current_mana,
    primary: row?.player.book.primary_skill_id, weld: row?.player.book.weld_build_id,
    cast: row?.wizard.primary_cast,
    projectiles: client.frame?.snapshot.spells.projectiles.map(row => Object.keys(row)[0]),
    transients: client.frame?.snapshot.spells.transients.map(row => Object.keys(row)[0])};
}
async function inputBoundary(client, label) {
  const browser = await client.page.evaluate(() => {
    const node = value => value && {tag: value.tagName, id: value.id,
      className: value.className, hidden: value.hidden};
    return {input: Array.from(window.wasmBindings.gameplay_input_state()),
      active: node(document.activeElement), hit: node(document.elementFromPoint(1000, 350)),
      menu: document.querySelector('.game-menus')?.dataset.menuScreen,
      joysticks: [...document.querySelectorAll('[data-joystick]')].map(value => ({
        lane: value.dataset.joystick, blocked: value.hasAttribute('data-blocked'),
        castDisabled: value.hasAttribute('data-cast-disabled')}))};
  });
  client.inputBoundaries.push({label, authority: castAuthority(client), browser});
}
async function castReady(client) {
  await client.page.waitForFunction(() => {
    const primary = document.querySelector('[data-joystick="primary"]');
    return primary && !primary.hasAttribute('data-blocked')
      && !primary.hasAttribute('data-cast-disabled')
      && document.querySelector('.game-menus')?.dataset.menuScreen === 'hidden'
      && document.elementFromPoint(1000, 350)?.id === 'game';
  });
}
function observeFlame(client) {
  const frame = client.frame.flame_presentation;
  if (frame) {
    client.flame.frames.push({tick: client.frame.snapshot.ticks, kind: frame.kind,
      epoch: frame.epoch, throughTick: frame.throughTick, afterTick: frame.afterTick,
      count: frame.births.length, authority: castAuthority(client)});
    if (client.flame.frames.length > 150) client.flame.frames.shift();
    for (const birth of frame.births) {
      if (birth.ownerId !== client.id) continue;
      const key = `${birth.worldKey}:${birth.kind}:${birth.id}:${birth.birthTick}`;
      client.flame.births.set(key, birth);
    }
  }
  for (const transient of client.frame.snapshot.spells.transients) {
    const actor = transient.Weld?.Channel || transient.Weld?.FlameLashFade;
    if (actor?.common.build_id !== 1003 || actor.common.owner.owner_id !== client.id) continue;
    client.flame.actors.push({tick: client.frame.snapshot.ticks,
      kind: transient.Weld.Channel ? 'channel' : 'fade', actor});
    if (client.flame.actors.length > 20) client.flame.actors.shift();
  }
}
async function selection(client, name) {
  await client.page.locator('.game-menus[data-menu-screen="create"]').waitFor();
  await client.page.getByRole('textbox', {name: 'Wizard name', exact: true}).fill(name);
  await client.page.locator('[data-menu-action="element:fire"]').click();
  await client.page.locator('[data-menu-action="discipline:arcane"]').click();
  await until(() => client.id && participant(client), `${client.label} actual admission`);
  await client.page.locator('.game-menus[data-menu-screen="hidden"]').waitFor({state: 'attached'});
}
async function move(client, key, distance) {
  const before = {...participant(client).wizard.position};
  const previousInput = client.inputs.at(-1)?.sequence ?? 0;
  await client.page.bringToFront();
  await client.page.locator('[data-joystick="movement"]:not([data-blocked])')
    .waitFor({state: 'attached'});
  await client.page.keyboard.down(key);
  try {
    await until(() => Math.hypot(participant(client).wizard.position.x - before.x,
      participant(client).wizard.position.y - before.y) >= distance, 'accepted living peer movement');
  } finally {
    await client.page.keyboard.up(key);
    const input = client.inputs.findLast(input => input.sequence > previousInput
      && Math.hypot(input.movement.x, input.movement.y) > 0);
    client.movementAttempts.push({key, distance, before,
      after: {...participant(client).wizard.position}, input});
  }
}
async function consoleCommand(client, code) {
  await client.page.bringToFront();
  await client.page.keyboard.press('Backquote');
  await client.page.locator('[data-cheat-action="console"]').click();
  await client.page.locator('#cheat-menu-lua-editor').fill(code);
  const firstRequest = client.luaRequests.length;
  await client.page.getByRole('button', {name: 'RUN LUA', exact: true}).click();
  await until(() => client.luaRequests.slice(firstRequest).some(request => request.code === code),
    'the actual Console execution request');
  const request = client.luaRequests.slice(firstRequest).find(request => request.code === code);
  await until(() => client.lua.some(result => result.request_id === request.request_id),
    'the matching Console execution result');
  const result = client.lua.find(result => result.request_id === request.request_id);
  assert.equal(result.ok, true, result.error || 'ordinary Console command accepted');
  await client.page.keyboard.press('Backquote');
  await client.page.locator('[data-joystick="movement"]:not([data-blocked])')
    .waitFor({state: 'attached'});
  return result;
}
async function flamePresentation(client) {
  await consoleCommand(client,
    'sd.dev.grant_weld(1003); sd.player.restore_health(10000000); sd.player.restore_mana(); return true');
  await until(() => participant(client).player.book.weld_build_id === 1003,
    'the actual selected Flame Lash build');
  await castReady(client);
  await inputBoundary(client, 'before aim press');

  // Hover only stores the cursor in input.rs; an ordinary cast press owns aim.
  // Observe that mapped world point before using the stock enemy-spawn API.
  const previousInput = client.inputs.at(-1)?.sequence ?? 0;
  await client.page.mouse.move(1000, 350);
  await client.page.mouse.down({button: 'left'});
  try {
    await until(() => client.inputs.some(input => input.sequence > previousInput
      && input.primary && input.aim), 'ordinary cast press sends its mapped world aim');
    await inputBoundary(client, 'actual aim press');
  } finally {
    await client.page.mouse.up({button: 'left'});
  }
  const target = client.inputs.findLast(input => input.sequence > previousInput
    && input.primary && input.aim).aim;
  await consoleCommand(client,
    `for i=1,3 do sd.enemies.spawn("coffin", {x=${target.x}+i*8,y=${target.y}}) end return true`);
  await castReady(client);
  await inputBoundary(client, 'before held Flame');
  await client.page.mouse.move(1000, 350);
  const before = {...participant(client).wizard.position};
  client.flame.active = true;
  await client.page.evaluate(() => {
    window.spectatorGpu.ground.length = 0;
    window.spectatorGpu.textures.length = 0;
    window.spectatorGpu.active = true;
  });
  await client.page.mouse.down({button: 'left'});
  try {
    await until(() => [...client.flame.births.values()].filter(birth => birth.kind === 'ribbon').length >= 10
      && [...client.flame.births.values()].some(birth => birth.kind === 'fade'),
    'accepted per-tick Flame ribbons and fades arrive through the real history');
    await client.page.screenshot({path: path.join(output, 'flame-held.png')});
  } finally {
    await client.page.mouse.up({button: 'left'});
  }
  const releasedSequence = client.inputs.at(-1)?.sequence ?? 0;
  await until(() => client.inputs.some(input => input.sequence >= releasedSequence && !input.primary),
    'ordinary primary release reaches the input stream');
  await client.page.screenshot({path: path.join(output, 'flame-released.png')});
  const gpu = await client.page.evaluate(() => {
    window.spectatorGpu.active = false;
    return structuredClone(window.spectatorGpu);
  });
  client.flame.active = false;
  const births = [...client.flame.births.values()];
  const ribbon = births.find(birth => birth.kind === 'ribbon');
  const fade = births.find(birth => birth.kind === 'fade');
  assert.equal(ribbon.recipe.birth.kind, 'native-birth-v1');
  assert.equal(fade.recipe.birth.kind, 'native-birth-v1');
  assert.equal(typeof ribbon.recipe.birth.cameraDisplacement.x, 'number');
  assert.equal(typeof ribbon.recipe.birth.cameraDisplacement.y, 'number');
  assert.ok(gpu.ground.length > 0, 'Flame and camera samples reached actual ground submissions');
  const receipt = {before, after: participant(client).wizard.position, target,
    frames: client.flame.frames, ribbonCount: births.filter(birth => birth.kind === 'ribbon').length,
    fadeCount: births.filter(birth => birth.kind === 'fade').length,
    ribbon, fade, sampledActors: client.flame.actors, gpu};
  fs.writeFileSync(path.join(output, 'flame-presentation.json'), JSON.stringify(receipt, null, 2));
}
async function leave(client) {
  const previous = client.leaves;
  await client.page.bringToFront();
  await client.page.keyboard.press('Escape');
  await client.page.locator('.game-menus[data-menu-screen="pause"]').waitFor({state: 'attached'});
  await client.page.locator('[data-menu-action="leave"]:visible').click();
  await client.page.locator('.game-menus[data-menu-screen="title"]').waitFor({state: 'attached'});
  await until(() => client.leaves > previous, 'actual saved Leave acknowledgement');
}
async function shadowSettings(client, lighting, shadows, name) {
  await client.page.bringToFront();
  await client.page.keyboard.press('Escape');
  await client.page.locator('.game-menus[data-menu-screen="pause"]').waitFor({state: 'attached'});
  await client.page.locator('[data-menu-action="pause-settings"]').click();
  await client.page.locator('[data-menu-action="settings-page:performance"]').click();
  for (const [key, enabled] of [['lighting', lighting], ['shadows', shadows]]) {
    const toggle = client.page.locator(`[data-menu-action="settings-toggle:${key}"]`);
    if (await toggle.getAttribute('aria-pressed') !== String(enabled)) await toggle.click();
    await until(async () => await toggle.getAttribute('aria-pressed') === String(enabled),
      `the actual ${key} setting reaches its rendered state`);
  }
  await client.page.locator('[data-menu-action="settings-back"]').click();
  await client.page.locator('[data-menu-action="settings-back"]').click();
  await client.page.locator('[data-menu-action="resume"]').click();
  await until(() => !client.frame.host.pause && !client.frame.host.resume_grace,
    'real settings resume admission');
  await client.page.screenshot({path: path.join(output, `${name}.png`)});
}
async function combatAdmission(client) {
  if (client.frame.snapshot.boneyard.encounter?.run_event_id !== 0) return;
  await client.page.bringToFront();
  // Pause requests a real accepted checkpoint. Wait for its actual cloud write
  // before giving the unchanged Source navigator that run's saved geometry.
  const persisted = client.page.waitForResponse(response => {
    return new URL(response.url()).pathname === '/game-stg/api/game/saves/0'
      && response.request().method() === 'PUT';
  });
  await client.page.keyboard.press('Escape');
  await client.page.locator('.game-menus[data-menu-screen="pause"]').waitFor({state: 'attached'});
  const response = await persisted;
  assert.equal(response.status(), 200);
  await response.finished();
  await client.page.locator('[data-menu-action="resume"]').click();
  await until(() => !client.frame.host.pause && !client.frame.host.resume_grace,
    'real resume before the Solomon entrance');
  await castReady(client);
  const saved = await client.page.context().request.get(new URL('api/game/saves/0', url).href,
    {headers: {Authorization: `Bearer ${leaderAccount.token}`}});
  assert.equal(saved.status(), 200, 'the actual saved entrance document is available');
  const source = await saved.json();
  await walkSolomonEntrance(client.page, {document: source.save.document,
    snapshot: () => client.frame.snapshot, playerId: () => client.id, output});
  await castReady(client);
  assert.ok(client.frame.snapshot.boneyard.encounter.run_event_id > 0,
    'the accepted Solomon entrance enables actual primary combat');
  await inputBoundary(client, 'actual Solomon combat admission');
}
function summary(client) {
  const actor = participant(client);
  return {label: client.label, id: client.id, tick: client.frame?.snapshot.ticks,
    runId: client.frame?.snapshot.run.run_id, lifeState: actor?.player.combat.life_state,
    position: actor?.wizard.position, health: actor?.player.combat.current_health,
    movementAttempts: client.movementAttempts,
    inputBoundaries: client.inputBoundaries, authority: castAuthority(client),
    checkpoints: client.checkpoints, leaves: client.leaves, lua: client.lua,
    recentInputs: client.inputs.slice(-30),
    wasm: [...client.wasm], renderer: client.renderer, errors: client.errors,
    cancelledMedia: client.cancelledMedia, cancelledNavigation: client.cancelledNavigation,
    registration: client.registration};
}

async function close(client) {
  const owned = client.server.process();
  client.cleanup = {mode: 'graceful', errors: []};
  async function bounded(promise, label) {
    let timer;
    try {
      await Promise.race([promise, new Promise((resolve, reject) => {
        timer = setTimeout(() => reject(new Error(label)), 5000);
      })]);
    } finally {
      clearTimeout(timer);
    }
  }
  try {
    await bounded(client.browser.close(), 'remote browser close deadline');
    await bounded(client.server.close(), 'Chrome server close deadline');
  } catch (error) {
    client.cleanup.mode = 'forced';
    client.cleanup.errors.push(error.message);
    try {
      await bounded(client.server.kill(), 'Chrome termination deadline');
    } catch (error) {
      client.cleanup.errors.push(error.message);
      owned.kill('SIGKILL');
    }
  }
  if (owned.exitCode !== null || owned.signalCode !== null) return true;
  try {
    process.kill(owned.pid, 0);
    return false;
  } catch (error) {
    if (error.code === 'ESRCH') return true;
    throw error;
  }
}

(async () => {
  fs.mkdirSync(output, {recursive: true});
  let failure;
  try {
    const peer = await peerAccount();
    step = 'private College';
    const leader = await open(leaderAccount, 'leader');
    const guest = await open(peer, 'living-peer');
    await leader.page.locator('[data-menu-action="settings"]:visible').click();
    await leader.page.locator('[data-menu-action="settings-page:cloud"]').click();
    for (const [key, value] of [['shared-hub', 'false'], ['cheats', 'true']]) {
      const toggle = leader.page.locator(`[data-menu-action="settings-toggle:${key}"]`);
      if (await toggle.getAttribute('aria-pressed') !== value) await toggle.click();
    }
    await leader.page.locator('[data-menu-action="settings-back"]').click();
    await leader.page.locator('[data-menu-action="settings-back"]').click();
    await leader.page.locator('[data-menu-action="play"]').click();
    await leader.page.locator('[data-menu-action="new-game"]').click();
    await leader.page.locator('.game-menus[data-menu-screen="create"], [role="dialog"][aria-label="Kill character?"]')
      .first().waitFor();
    const confirmation = leader.page.getByRole('dialog', {name: 'Kill character?', exact: true});
    if (await confirmation.isVisible()) await confirmation.locator('[data-menu-action="prompt-primary"]').click();
    await selection(leader, 'Severin');
    await leader.page.locator('[data-social-action="party"]').click();
    await leader.page.locator('[data-native-ui-party-tab-action="settings"]').click();
    const code = await leader.page.locator('[data-party-code]').getAttribute('data-party-code');
    assert.ok(code);
    await leader.page.keyboard.press('Escape');
    await leader.page.locator('[data-native-ui-party-tab-action="settings"]').waitFor({state: 'detached'});
    assert.equal(await leader.page.locator('.game-menus').getAttribute('data-menu-screen'), 'hidden',
      'closing direct-chip Party with Escape does not open Pause underneath');
    await guest.page.locator('[data-menu-action="play"]').click();
    await guest.page.locator('[data-menu-action="join-party"]').click();
    await guest.page.getByRole('textbox', {name: 'Party ID', exact: true}).fill(code);
    await guest.page.locator('button[data-menu-action="join-code"]').click();
    await guest.page.getByRole('dialog', {name: 'Join party consent', exact: true})
      .getByRole('button', {name: 'JOIN PARTY', exact: true}).click();
    await selection(guest, 'Lucian');
    await until(() => leader.frame.host.connected_player_ids.length === 2
      && guest.frame.host.connected_player_ids.length === 2, 'same actual two-player College');
    checked('actual private party code and consent admit two fresh wizard selections');

    step = 'Boneyard readiness';
    await leader.page.getByRole('button', {name: 'Enter the Boneyard', exact: true}).click();
    await until(() => clients.every(client => client.frame?.snapshot.boneyard
      && !client.frame.host.loading_run_id && !client.frame.host.resume_grace
      && !client.frame.host.pause), 'both actual renderers ready');
    const runId = leader.frame.snapshot.boneyard.run_id;
    assert.equal(guest.frame.snapshot.boneyard.run_id, runId);
    assert.equal(leader.frame.snapshot.run.eligible_player_ids.length, 2);
    checked('both participants are eligible in one actual ready Boneyard');
    await leader.page.screenshot({path: path.join(output, 'two-living-players.png')});

    step = 'actual scenery shadow settings';
    await shadowSettings(leader, true, false, 'scenery-flat-shadows');
    await shadowSettings(leader, false, true, 'scenery-unlit-complex-shadows');
    await shadowSettings(leader, true, true, 'scenery-lit-complex-shadows');
    const counts = {};
    for (const object of leader.loaded.scene.objects) counts[object.type_id] = (counts[object.type_id] || 0) + 1;
    fs.writeFileSync(path.join(output, 'accepted-scene-families.json'), JSON.stringify(counts, null, 2));
    checked('real settings paint flat, unlit complex and lit complex scenery modes');

    step = 'ordinary living peer separation';
    const bounds = guest.loaded.scene.bounds;
    const peerDirection = participant(guest).wizard.position.y < bounds.y + bounds.height / 2
      ? 'KeyS' : 'KeyW';
    await move(guest, peerDirection, 150);

    step = 'ordinary Solomon entrance';
    await combatAdmission(leader);
    checked('ordinary movement completes Solomon entrance before primary combat expectations');

    step = 'actual retained Flame presentation';
    await flamePresentation(leader);
    checked('ordinary held and released Flame casts deliver native births, retained history and real GPU frames');

    step = 'accepted combat death';
    const position = participant(leader).wizard.position;
    await consoleCommand(leader,
      `for i=1,8 do sd.enemies.spawn("skeleton", {x=${position.x}+i*3,y=${position.y}}) end return true`);
    await until(() => life(leader) === 'spectating', 'real enemy damage through terminal player pose');
    assert.equal(life(guest), 'alive');
    assert.equal(leader.frame.snapshot.run.phase.toLowerCase(), 'active');
    const status = leader.page.locator('.boneyard-spectator-status:not([hidden])');
    await status.waitFor();
    assert.equal(await status.getAttribute('data-target-player-id'), guest.id);
    assert.equal(await status.getAttribute('data-run-id'), runId);
    assert.ok((await status.getAttribute('aria-label')).includes('Lucian'));
    checked('actual combat death selects the eligible living peer and renders Source spectator status');
    await leader.page.screenshot({path: path.join(output, 'spectator-follow-before.png')});

    step = 'living peer focus and cycle';
    const deadPosition = {...participant(leader).wizard.position};
    await leader.page.evaluate(() => {
      window.spectatorGpu.ground.length = 0;
      window.spectatorGpu.textures.length = 0;
      window.spectatorGpu.active = true;
    });
    await move(guest, peerDirection, 75);
    assert.deepEqual(participant(leader).wizard.position, deadPosition);
    assert.equal(await status.getAttribute('data-target-player-id'), guest.id);
    await leader.page.bringToFront();
    await leader.page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    const firstSequence = leader.inputs.at(-1)?.sequence ?? 0;
    for (const button of ['left', 'right']) {
      const previousFrame = leader.frame.sequence;
      await leader.page.mouse.click(800, 600, {button});
      await leader.page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
      // Unchanged input deliberately sends no wire message. Observe actual UI
      // frames and a later accepted snapshot without inventing a cast heartbeat.
      await until(() => leader.frame.sequence > previousFrame, 'later accepted spectator frame');
      assert.equal(await status.getAttribute('data-target-player-id'), guest.id);
    }
    assert.ok(leader.inputs.filter(input => input.sequence > firstSequence)
      .every(input => !input.primary && input.quickbar === null));
    const gpu = await leader.page.evaluate(() => {
      window.spectatorGpu.active = false;
      return window.spectatorGpu;
    });
    assert.ok(gpu.ground.length > 0, 'the actual Boneyard field reached GPU submissions');
    fs.writeFileSync(path.join(output, 'spectator-ground-gpu.json'), JSON.stringify(gpu, null, 2));
    await leader.page.screenshot({path: path.join(output, 'spectator-follow-after.png')});
    checked('stationary dead local keeps its living target; left/right wrapping sends no cast');

    step = 'actual saved Leave and reset';
    await leave(leader);
    await leader.page.locator('.boneyard-spectator-status').waitFor({state: 'hidden'});
    // The authority's Leave ends this private run. Its surviving participant
    // returns to College, where a second Boneyard Pause is no longer available.
    await until(() => !guest.frame.snapshot.boneyard
      && guest.frame.snapshot.run.phase === 'Hub', 'the surviving peer returns to College');
    checked('saved leader Leave resets spectator presentation and returns the peer to College');
    assert.deepEqual(clients.flatMap(client => client.errors), []);
    if (build) {
      const wasm = Object.keys(build.files).find(file => /^solomon-web-.*\.wasm$/.test(file));
      assert.ok(wasm, 'the accepted artifact manifest names the game WASM');
      for (const client of clients) assert.ok(client.wasm.has(wasm));
    }
  } catch (error) {
    failure = {step, error: error.stack};
    for (const client of clients) {
      await inputBoundary(client, 'failure').catch(() => {});
      await client.page.screenshot({path: path.join(output, `${client.label}-failure.png`)}).catch(() => {});
      failure[client.label] = await client.page.locator('body').innerText().catch(() => 'page closed');
    }
  } finally {
    for (const client of clients) {
      if (!client.flame.active && client.flame.frames.length === 0) continue;
      const gpu = await client.page.evaluate(() => structuredClone(window.spectatorGpu))
        .catch(() => null);
      fs.writeFileSync(path.join(output, `${client.label}-flame-observation.json`), JSON.stringify({
        authority: castAuthority(client), inputs: client.inputs, frames: client.flame.frames,
        births: [...client.flame.births.values()], actors: client.flame.actors, gpu}, null, 2));
    }
    const artifact = build && {revision: build.revision, builtAt: build.builtAt,
      wasm: Object.fromEntries(Object.entries(build.files)
        .filter(([file]) => /^solomon-web-.*\.wasm$/.test(file)))};
    const receipt = {passed: !failure, step, artifact, checks, failure};
    receipt.browserClosed = (await Promise.all(clients.map(close))).every(Boolean);
    receipt.clients = clients.map(client => ({...summary(client), cleanup: client.cleanup}));
    fs.writeFileSync(path.join(output, 'receipt.json'), JSON.stringify(receipt, null, 2));
    console.log(JSON.stringify({passed: receipt.passed, checks, step, browserClosed: receipt.browserClosed,
      failure: failure?.error}));
    // The launched browser server can retain an idle observation socket after
    // its Chrome process exits. All evidence is written synchronously above.
    process.exit(failure || !receipt.browserClosed ? 1 : 0);
  }
})().catch(error => {console.error(error.stack); process.exitCode = 1;});
