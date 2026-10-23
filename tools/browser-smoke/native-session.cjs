// Two real browsers drive the packaged native authority through its ordinary UI.
// Socket observation reads accepted state; the driver never sends gameplay messages.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const {spawn} = require('node:child_process');
const {createHash} = require('node:crypto');
const {chromium} = require(path.resolve(process.env.PW));
const {startNewGame} = require('./menu-journey.cjs');
const {assertFullscreenToggle} = require('./fullscreen.cjs');
const {createNativeMessageReader} = require('./native-wire.cjs');
const ui = require('./gpu-ui.cjs');
const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');

const output = path.resolve(process.env.OUT || os.tmpdir());
const artifact = process.env.PACKAGE && path.resolve(process.env.PACKAGE);
const binary = process.env.BINARY || path.join(artifact, process.platform === 'win32' ? 'solomon-darker.exe' : 'solomon-darker');
const port = Number(process.env.PORT || 4190);
const origin = `http://127.0.0.1:${port}`;
const working = fs.mkdtempSync(path.join(os.tmpdir(), 'solomon-native-'));
const savePath = path.join(working, 'host.json');
const checks = [];
const errors = [];
const shutdownResourceErrors = [];
let intentionalHostShutdown = false;
const requests = new Set();
const hostLog = [];
let movementEvidence;
let host;

async function waitUntil(predicate, label, timeout = 30000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (await predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for ${label}`);
}

async function startHost() {
  const args = ['--no-open', '--port', String(port), '--save', savePath];
  if (process.env.ASSETS) args.push('--assets', path.resolve(process.env.ASSETS));
  host = spawn(binary, args, {cwd: working, env: {...process.env, PATH: ''}, stdio: ['ignore', 'pipe', 'pipe']});
  let ready = false;
  let startError;
  host.on('error', error => {startError = error;});
  host.stderr.on('data', data => hostLog.push(data.toString()));
  host.stdout.on('data', data => {
    const line = data.toString();
    hostLog.push(line);
    if (line.includes('is running at')) ready = true;
  });
  await waitUntil(() => {
    if (startError) throw startError;
    if (host.exitCode !== null) throw new Error(`Host exited ${host.exitCode}: ${hostLog.join('')}`);
    return ready;
  }, 'native host startup');
}

async function stopHost() {
  if (!host || host.exitCode !== null) return;
  host.kill('SIGINT');
  await waitUntil(() => host.exitCode !== null, 'native save and shutdown');
  assert.equal(host.exitCode, 0, 'native shutdown commits the save successfully');
  host = null;
}

function observe(page) {
  const state = {frame: null, id: null, token: null, status: null, social: null, messages: [], sent: [], commands: [], results: [], saves: 0};
  state.failureObserverReady = ui.watchRequestFailures(page, {errors}).ready;
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error') return;
    const text = message.text();
    if (intentionalHostShutdown && text === 'Failed to load resource: net::ERR_CONNECTION_REFUSED') {
      // The final regression deliberately removes the process that serves game assets.
      shutdownResourceErrors.push({text, url: message.location().url});
    } else errors.push(text);
  });
  page.on('response', response => {if (response.status() >= 400) errors.push(`${response.status()} ${response.url()}`);});
  page.on('websocket', socket => {
    const read = createNativeMessageReader();
    socket.on('framereceived', ({payload}) => {
      const message = read(JSON.parse(payload.toString()));
      if (!message) return;
      if (message.type === 'welcome') {
        state.id = message.credentials.player_id;
        state.token = message.credentials.token;
        state.frame = message.frame;
      } else if (message.type === 'snapshot') state.frame = message.frame;
      else if (message.type === 'session_status') state.status = message;
      else if (message.type === 'social') state.social = message.snapshot;
      else if (message.type === 'chat') state.messages.push(message.message);
      else if (message.type === 'command_result') state.results.push(message);
      else if (message.type === 'save_committed') state.saves++;
      else if (message.type === 'error') errors.push(`host ${message.code}: ${message.reason}`);
    });
    socket.on('framesent', ({payload}) => {
      const message = JSON.parse(payload.toString());
      // Keep command kinds and input sequence evidence, never resume credentials.
      state.sent.push({type: message.type, kind: message.command?.kind, sequence: message.sequence});
      if (message.type === 'command') state.commands.push({kind: message.command.kind, sequence: message.sequence});
      if (state.sent.length > 2000) state.sent.splice(0, 1000);
    });
  });
  return state;
}

const participant = (state, id = state.id) => state.frame?.snapshot.participants[id];
const position = (state, id) => participant(state, id)?.wizard.position;
const playing = state => state.frame?.snapshot.boneyard && !state.frame.host.loading_run_id
  && !state.frame.host.resume_grace && !state.frame.host.pause;

async function loaded(page, state) {
  await state.failureObserverReady;
  state.status = null;
  if (page.url() === 'about:blank') await page.goto(`${origin}/?host=1`);
  else await ui.observeDocumentReplacement(page, () => page.goto(`${origin}/?host=1`));
  await ui.ready(page,{timeout:120000});
  await waitUntil(() => state.status, 'native title inspection');
  // Socket delivery precedes the next application frame that installs title prompts.
  await ui.surface(page,'title');
  await ui.assertGpuOnly(page);
}

async function continueGame(page) {
  await ui.click(page,'play');
  await ui.click(page,'continue');
  await ui.surface(page,'hidden');
}

async function gameplayInputReady(page) {
  // Socket delivery can precede the frame that removes the client's loading/grace hold.
  await page.bringToFront();
  await ui.waitForUi(page,snapshot => snapshot.surface === 'hidden' && !snapshot.inputScope
    && !snapshot.capture.text && ui.findWidget(snapshot,'hud:health')
    && !ui.findWidget(snapshot,'shared/pause') && !ui.findWidget(snapshot,'shared/resume'),
    'actual HUD and world input handoff');
}

async function pause(page) {
  await page.bringToFront();
  await page.keyboard.press('Escape');
  await ui.surface(page,'pause');
}

async function leaveGame(page, state) {
  await pause(page);
  const previous = state.saves;
  await ui.click(page,'leave');
  await waitUntil(() => state.saves > previous, 'durable save acknowledgement');
  await ui.surface(page,'title');
}

async function approachSolomon(page, state) {
  const held = new Set();
  const route = [];
  const world = state.frame.snapshot.boneyard;
  const start = position(state);
  const gates = new Map();
  for (const leaf of world.gate_leaves) {
    const pair = gates.get(leaf.fence_eid) || [];
    pair.push(leaf.hinge);
    gates.set(leaf.fence_eid, pair);
  }
  // Follow the actual entrance opening before aiming at Solomon across the arena.
  // Walking straight toward him from the spawn can run into the solid fence.
  const gate = [...gates.values()].filter(pair => pair.length === 2)
    .map(pair => ({x: (pair[0].x + pair[1].x) / 2, y: (pair[0].y + pair[1].y) / 2}))
    .sort((a, b) => Math.hypot(a.x - start.x, a.y - start.y) - Math.hypot(b.x - start.x, b.y - start.y))[0];
  assert.ok(gate, 'the generated opening has its authored gate');
  const entryDirection = world.arena_transition.entry_side === 'North' ? 1 : -1;
  const waypoints = [{x: gate.x, y: gate.y - entryDirection * 75},
    {x: gate.x, y: gate.y + entryDirection * 150}, world.encounter.position];
  let waypoint = 0;
  const deadline = Date.now() + 90000;
  try {
    while (state.frame.snapshot.boneyard.encounter.phase === 'Digging' && Date.now() < deadline) {
      const at = position(state);
      const target = waypoints[waypoint];
      const dx = target.x - at.x;
      const dy = target.y - at.y;
      const close = Math.hypot(dx, dy) < (waypoint === waypoints.length - 1 ? 90 : 12);
      const desired = new Set();
      if (close && waypoint < waypoints.length - 1) waypoint++;
      if (!close && Math.abs(dx) > 8) desired.add(dx < 0 ? 'KeyA' : 'KeyD');
      if (!close && Math.abs(dy) > 8) desired.add(dy < 0 ? 'KeyW' : 'KeyS');
      for (const key of held) {
        if (!desired.has(key)) {await page.keyboard.up(key); held.delete(key);}
      }
      for (const key of desired) {
        if (!held.has(key)) {await page.keyboard.down(key); held.add(key);}
      }
      route.push({tick: state.frame.snapshot.ticks, position: at, target, waypoint});
      await page.waitForTimeout(100);
    }
    assert.notEqual(state.frame.snapshot.boneyard.encounter.phase, 'Digging', 'real proximity wakes Solomon');
  } finally {
    for (const key of held) await page.keyboard.up(key);
    fs.writeFileSync(path.join(output, 'native-solomon-approach.json'), JSON.stringify(route, null, 2));
  }
}

(async () => {
  let browser;
  let browserOwner;
  let first;
  let second;
  try {
    fs.mkdirSync(output, {recursive: true});
    await startHost();
    browserOwner = await launchOwnedBrowser(chromium, {headless: true,
      executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
      args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
    browser = browserOwner.browser;
    async function client() {
      const context = await browser.newContext({viewport: {width: 1280, height: 720}});
      await context.route('**/*', route => {
        const url = route.request().url();
        requests.add(url);
        if (new URL(url).origin !== origin) {errors.push(`Unbundled request: ${url}`); return route.abort();}
        return route.continue();
      });
      const page = await context.newPage();
      const state = observe(page);
      await loaded(page, state);
      return {page, state};
    }
    first = await client();
    await startNewGame(first.page, {name: 'Felix', element: 'fire', discipline: 'body'});
    checks.push('first real College admission');
    console.log(checks.at(-1));
    await ui.click(first.page,'hud:inventory');
    await ui.widget(first.page,'panel:surface:inventory');
    await first.page.screenshot({path:path.join(output,'native-scaled-inventory.png')});
    await ui.click(first.page,'panel:close');
    await ui.noWidget(first.page,'panel:surface:inventory');
    checks.push('scaled desktop HUD Inventory opens its actual book through real pointer input');
    second = await client();
    await startNewGame(second.page, {name: 'Caius', element: 'ether', discipline: 'arcane'});
    await waitUntil(() => first.state.frame.snapshot.participant_order.length === 2
      && participant(first.state, second.state.id)?.player.selection.name === 'Caius', 'shared named participants');
    assert.notEqual(first.state.id, second.state.id);
    checks.push('two distinct browsers in one College');
    console.log(checks.at(-1));

    const before = {...position(first.state)};
    await first.page.bringToFront();
    await first.page.keyboard.down('KeyA');
    const samples = await first.page.evaluate(() => new Promise(resolve => {
      const samples = [];
      const start = performance.now();
      function sample(now) {
        const wizard = window.wasmBindings.wizard_state();
        samples.push({ms: now - start, x: wizard[0], y: wizard[1], tick: wizard[5]});
        if (now - start < 700) requestAnimationFrame(sample); else resolve(samples);
      }
      requestAnimationFrame(sample);
    }));
    await first.page.keyboard.up('KeyA');
    movementEvidence = {before, samples};
    fs.writeFileSync(path.join(output, 'native-movement-samples.json'), JSON.stringify(movementEvidence, null, 2));
    await waitUntil(() => Math.hypot(position(second.state, first.state.id).x - before.x,
      position(second.state, first.state.id).y - before.y) > 5
      && first.state.frame.input_acknowledgements[first.state.id] > 0, 'accepted movement visible to peer');
    checks.push('accepted movement, input acknowledgements and peer state');

    await ui.click(first.page,(await ui.byLabel(first.page,'Fomentius',{role:'button'})).id);
    const firstHealthPotion = snapshot => snapshot.widgets
      .filter(widget => widget.role === 'button' && widget.id.startsWith('panel:shop:')
        && widget.label === 'Buy Health Potion for 150 gold')
      .sort((a,b) => a.z-b.z)[0];
    const stock = await ui.waitForUi(first.page, firstHealthPotion, 'the first visible Health Potion stock row');
    const buy = firstHealthPotion(stock);
    await ui.click(first.page,buy.id); await ui.click(first.page,buy.id);
    await ui.byLabel(first.page,'Health Potion, quantity 2',{role:'button',prefix:'panel:item:'});
    await waitUntil(() => participant(second.state, first.state.id).player.economy.gold === 350, 'accepted purchase on both clients');
    await ui.click(first.page,'panel:close');
    await ui.noWidget(first.page,'panel:navigation');
    checks.push('real inventory transaction and authoritative feedback');
    await first.page.screenshot({path: path.join(output, 'native-college.png')});

    if (process.env.SOCIAL !== '0') {
      await first.page.keyboard.press('KeyI');
      await ui.widget(first.page,'panel:surface:inventory');
      const chatPosition = {...position(first.state)};
      await first.page.keyboard.press('KeyT');
      // Actual key events exercise shortcuts in the underlying inventory as well as chat.
      await ui.focused(first.page,'social/chat-message');
      await first.page.keyboard.type('Shared College ready wasd',{delay:10});
      await first.page.keyboard.press('Enter');
      await waitUntil(() => second.state.messages.some(message => message.text === 'Shared College ready wasd'), 'accepted peer chat');
      await ui.noWidget(first.page,'social/chat-message');
      await first.page.keyboard.press('KeyT');
      await ui.widget(first.page,'social/chat-message');
      await first.page.keyboard.press('Escape');
      await ui.noWidget(first.page,'social/chat-message');
      await ui.widget(first.page,'panel:surface:inventory');
      assert.deepEqual(position(first.state), chatPosition, 'typing chat cannot move the wizard');
      assert.equal((await ui.readUi(first.page)).surface, 'hidden');
      await first.page.keyboard.press('Escape');
      await ui.noWidget(first.page,'panel:navigation');
      checks.push('real chat delivery and keyboard focus above Inventory');

      await ui.click(first.page,'social/party');
      await ui.widget(first.page,'social/party-menu');
      await ui.widget(first.page,`social/member:${first.state.id}`);
      await ui.widget(first.page,`social/member:${second.state.id}`);
      await first.page.screenshot({path:path.join(output,'native-party.png')});
      await ui.click(first.page,`social/member:${second.state.id}`);
      assert.equal((await ui.widget(first.page,'social/player-card')).label,'Caius');
      await ui.click(first.page,'social/card-message');
      await ui.fill(first.page,'social/chat-message','Private College greeting');
      await first.page.keyboard.press('Enter');
      await waitUntil(() => second.state.messages.some(message => message.text === 'Private College greeting'
        && message.channel === 'Whisper'), 'host-delivered profile-card whisper');
      await ui.noWidget(first.page,'social/chat-message');
      checks.push('actual Party roster, player card and whisper');

      await pause(first.page);
      await first.page.keyboard.press('KeyT');
      await ui.fill(first.page,'social/chat-message','Pause menu greeting');
      await first.page.keyboard.press('Enter');
      await waitUntil(() => second.state.messages.some(message => message.text === 'Pause menu greeting'), 'chat above Pause');
      await ui.noWidget(first.page,'social/chat-message');
      await first.page.keyboard.press('KeyT');
      await ui.widget(first.page,'social/chat-message');
      await first.page.keyboard.press('Escape');
      await ui.noWidget(first.page,'social/chat-message');
      assert.equal((await ui.readUi(first.page)).surface, 'pause', 'closing chat preserves Pause');
      await ui.click(first.page,'resume');
      checks.push('chat focus and dismissal above Pause');
    }
    assert.equal(first.state.commands.some(command => command.kind === 'tutorial_action'), false,
      'ordinary College panels never send Tutorial surface acknowledgements');

    await ui.click(second.page,(await ui.byLabel(second.page,'Fomentius',{role:'button'})).id);
    await ui.widget(second.page,'panel:surface:inventory');
    await ui.click(first.page,(await ui.byLabel(first.page,'Enter the Boneyard',{role:'button'})).id);
    await waitUntil(() => playing(first.state) && playing(second.state), 'both Boneyard renderers and resume readiness', 120000);
    await ui.noWidget(second.page,'panel:navigation');
    assert.equal(first.state.frame.snapshot.boneyard.run_id, second.state.frame.snapshot.boneyard.run_id);
    checks.push('shared Boneyard loading, old shop cleanup and renderer readiness');
    console.log(checks.at(-1));
    await gameplayInputReady(first.page);
    const battlePosition = {...position(first.state)};
    await first.page.keyboard.down('KeyA');
    await waitUntil(() => Math.hypot(position(second.state, first.state.id).x - battlePosition.x,
      position(second.state, first.state.id).y - battlePosition.y) > 70, 'distinct visible Boneyard wizard positions');
    await first.page.keyboard.up('KeyA');
    await pause(first.page);
    await waitUntil(() => second.state.frame.host.pause?.owner_player_id === first.state.id, 'peer pause ownership');
    assert.match((await ui.widget(second.page,'shared/pause')).label,/Felix has paused the game/);
    await ui.noWidget(first.page,'shared/pause');
    await second.page.screenshot({path: path.join(output, 'native-shared-pause.png')});
    const pausedTick = second.state.frame.snapshot.ticks;
    const commands = second.state.sent.filter(message => message.kind === 'interact_goodie').length;
    await second.page.keyboard.press('KeyE');
    await second.page.waitForTimeout(300);
    assert.equal(second.state.frame.snapshot.ticks, pausedTick, 'shared Boneyard stops while paused');
    assert.equal(second.state.sent.filter(message => message.kind === 'interact_goodie').length, commands, 'remote pause blocks reliable interactions');
    await ui.click(first.page,'resume');
    await ui.value(second.page,'shared/resume','pause-menu-closed');
    await second.page.screenshot({path: path.join(output, 'native-shared-resume.png')});
    await waitUntil(() => playing(first.state) && playing(second.state), 'shared resume grace');
    await ui.noWidget(second.page,'shared/resume');
    checks.push('shared pause, named waiting view, interaction hold and resume countdown');
    await assertFullscreenToggle(first.page);
    checks.push('native gameplay fullscreen enters and exits through trusted clicks');
    await second.page.screenshot({path: path.join(output, 'native-boneyard-peer.png')});

    const originalId = first.state.id;
    const originalToken = first.state.token;
    await leaveGame(first.page, first.state);
    await loaded(first.page, first.state);
    assert.equal(first.state.status.can_resume, true);
    await continueGame(first.page);
    await waitUntil(() => playing(first.state) && playing(second.state), 'rejoined Boneyard readiness');
    assert.equal(first.state.id, originalId);
    assert.notEqual(first.state.token, originalToken, 'rejoin rotates the resume credential');
    assert.equal(participant(first.state).player.economy.gold, 350);
    checks.push('save-before-leave and same-wizard reconnect');

    await leaveGame(first.page, first.state);
    await waitUntil(() => playing(second.state), 'remaining player ready after departure');
    await leaveGame(second.page, second.state);
    await stopHost();
    assert.ok(fs.statSync(savePath).size > 0, 'native save exists outside the browser');
    await startHost();
    await loaded(first.page, first.state);
    assert.equal(first.state.status.can_resume, true, 'restarted native host recognizes the durable credential');
    await continueGame(first.page);
    await waitUntil(() => playing(first.state), 'restarted game renderer', 120000);
    assert.equal(first.state.id, originalId);
    assert.equal(participant(first.state).player.economy.gold, 350);
    checks.push('native file save and launcher restart');
    if (process.env.GAMEOVER === '1') {
      await gameplayInputReady(first.page);
      await approachSolomon(first.page, first.state);
      checks.push('real movement wakes Solomon and begins the opening encounter');
      await waitUntil(() => first.state.frame.snapshot.run.phase === 'GameOver', 'actual combat death', 180000);
      await ui.waitForUi(first.page,snapshot => ui.findWidget(snapshot,'game-over:continue')?.label.includes('Continue'),
        'actual native Game Over input admission');
      await first.page.screenshot({path: path.join(output, 'native-game-over.png')});
      for (const client of [first, second]) await ui.settleErrors(client.page);
      assert.deepEqual(errors, [], 'no errors before the intentional host shutdown');
      intentionalHostShutdown = true;
      await stopHost();
      await ui.surface(first.page,'title');
      await ui.noWidget(first.page,'game-over:continue');
      await first.page.screenshot({path: path.join(output, 'native-title-after-disconnect.png')});
      checks.push('Game Over reaches Continue readiness and leaves with its disconnected session');
    }
    assert.ok([...requests].some(url => url.endsWith('.wasm')));
    for (const client of [first, second]) await ui.settleErrors(client.page);
    assert.deepEqual(errors, []);
    const commandEvidence = [first, second].map(client => ({commands: client.state.commands, results: client.state.results}));
    fs.writeFileSync(path.join(output, 'native-command-results.json'), JSON.stringify(commandEvidence, null, 2));
    const digest = file => createHash('sha256').update(fs.readFileSync(file)).digest('hex');
    const assets = process.env.ASSETS ? path.resolve(process.env.ASSETS) : path.join(artifact, 'game');
    const renderer = await first.page.evaluate(() => {
      const gl = document.querySelector('#game').getContext('webgl2');
      const debug = gl.getExtension('WEBGL_debug_renderer_info');
      return debug ? gl.getParameter(debug.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER);
    });
    const receipt = {passed: true, checks, renderer, socialChecked: process.env.SOCIAL !== '0', gameOverChecked: process.env.GAMEOVER === '1',
      clientRevision: process.env.CLIENT_REVISION || null, binarySha256: digest(binary),
      wasm: fs.readdirSync(assets).filter(file => file.endsWith('.wasm')).map(file => ({file, sha256: digest(path.join(assets, file))})),
      origin, requests: requests.size, errors, shutdownResourceErrors};
    fs.writeFileSync(path.join(output, 'native-session-receipt.json'), JSON.stringify(receipt, null, 2));
    console.log(JSON.stringify(receipt));
  } catch (error) {
    const state = {};
    for (const [name, client] of [['first', first], ['second', second]]) {
      if (!client) continue;
      await ui.settleErrors(client.page);
      state[name] = {id: client.state.id, tick: client.state.frame?.snapshot.ticks, host: client.state.frame?.host,
        commands: client.state.commands, results: client.state.results,
        inputAcknowledgements: client.state.frame?.input_acknowledgements, recentSent: client.state.sent.slice(-12),
        wizard: participant(client.state)?.wizard, run: client.state.frame?.snapshot.run,
        ui: await ui.readUi(client.page).catch(() => null)};
      await client.page.screenshot({path: path.join(output, `native-failure-${name}.png`)}).catch(() => {});
    }
    fs.writeFileSync(path.join(output, 'native-session-failure.json'), JSON.stringify({error: error.stack, checks, errors, shutdownResourceErrors, movementEvidence, state, hostLog}, null, 2));
    throw error;
  } finally {
    try {
      if (browserOwner) {
        try {
          const cleanup = await browserOwner.close();
          fs.writeFileSync(path.join(output,'browser-cleanup.json'),JSON.stringify(cleanup,null,2));
        } catch (error) {
          fs.writeFileSync(path.join(output,'browser-cleanup.json'),JSON.stringify(
            error.cleanup || {error:error.message},null,2));
          throw error;
        }
      }
    } finally {
      await stopHost();
      fs.rmSync(working, {recursive: true, force: true});
    }
  }
})().catch(error => {console.error(error); process.exitCode = 1;});
