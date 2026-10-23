// Actual Website accounts, ordinary UI and read-only socket observation.
// Expected flow: MainMenuScene.tsx, PlayerCardDialog.tsx,
// CollegeInvitationDialog.tsx and PartyJoinConsentDialog.tsx in pinned Website.
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const {createNativeMessageReader} = require('./native-wire.cjs');
const ui = require('./gpu-ui.cjs');
const {startNewGame} = require('./menu-journey.cjs');
const {chromium, request} = require(path.resolve(process.env.PW));

const url = process.env.URL || 'https://127.0.0.1:9449/game-stg/';
const output = path.resolve(process.env.OUT);
const websiteMount = process.env.WEBSITE_MOUNT ?? new URL(url).pathname.replace(/\/$/, '');
const tokenKey = websiteMount ? `sdr.token:${websiteMount}` : 'sdr.token';
// Only initial asset residency uses this override; later behavior waits are unchanged.
const coldStartTimeout = Number(process.env.COLD_START_TIMEOUT_MS || 30000);
assert.ok(Number.isSafeInteger(coldStartTimeout) && coldStartTimeout > 0,
  'COLD_START_TIMEOUT_MS must be a positive integer');
const privateConfig = path.resolve(process.env.PRIVATE_CONFIG);
const accountFile = process.env.ACCOUNT_FILE || path.join(output, 'private-accounts.json');
const checks = [];
const clients = [];
let step = 'registration';

async function waitUntil(predicate, label) {
  const deadline = Date.now() + 30000;
  while (Date.now() < deadline) {
    if (await predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for ${label}`);
}

async function bounded(promise, label, milliseconds) {
  let timer;
  try {
    return await Promise.race([promise, new Promise((resolve, reject) => {
      timer = setTimeout(() => reject(new Error(`${label} timed out`)), milliseconds);
    })]);
  } finally {
    clearTimeout(timer);
  }
}

function checked(label) {
  checks.push(label);
  console.log(label);
}

async function accounts() {
  if (fs.existsSync(accountFile)) {
    fs.chmodSync(accountFile, 0o600);
    return JSON.parse(fs.readFileSync(accountFile));
  }
  const configuration = JSON.parse(fs.readFileSync(privateConfig));
  const api = await request.newContext({ignoreHTTPSErrors: process.env.LOCAL_TLS === '1', timeout: 30000});
  const registered = [];
  const suffix = `${Date.now().toString(36)}${crypto.randomBytes(2).toString('hex')}`;
  try {
    for (const letter of ['A', 'B']) {
      const username = `Social${letter}${suffix}`;
      const email = `${username.toLowerCase()}@example.invalid`;
      const response = await api.post(new URL('api/auth/register', url).href, {
        data: {username, email, password: configuration.testPassword},
      });
      assert.equal(response.status(), 201, `fresh account ${letter} registration`);
      const body = await response.json();
      assert.equal(body.user.username, username);
      registered.push({username, email, password: configuration.testPassword, token: body.token});
      // Retain partial registration too, so a failed run never loses account ownership.
      fs.writeFileSync(accountFile, JSON.stringify(registered, null, 2), {mode: 0o600});
      fs.chmodSync(accountFile, 0o600);
    }
    return registered;
  } finally {
    await api.dispose();
  }
}

function observe(page, label) {
  const state = {label, id: null, frame: null, social: null, welcomes: [], cards: [],
    invitations: [], partyActions: [], chats: [], commands: [], requests: [],
    errors: [], wasm: new Set(), sockets: [], trace: []};
  state.failureObserverReady = ui.watchRequestFailures(page, {errors: state.errors}).ready;
  page.on('pageerror', error => state.errors.push(error.message));
  page.on('console', message => {
    if (message.type() === 'error') state.errors.push(message.text());
  });
  page.on('response', response => {
    const pathname = new URL(response.url()).pathname;
    if (pathname.endsWith('.wasm')) state.wasm.add(path.basename(pathname));
    if (pathname.includes('/api/')) {
      const receipt = {path: pathname, status: response.status(), method: response.request().method()};
      state.requests.push(receipt);
      state.trace.push({direction: 'response', ...receipt});
    }
    if (response.status() >= 400) state.errors.push(`${response.status()} ${pathname}`);
  });
  page.on('websocket', socket => {
    const read = createNativeMessageReader();
    // No credential/query string or complete socket payload enters the receipt.
    const connection = {path: new URL(socket.url()).pathname, closed: false};
    state.sockets.push(connection);
    socket.on('close', () => {connection.closed = true;});
    socket.on('framesent', ({payload}) => {
      const message = JSON.parse(payload.toString());
      const kind = message.type === 'hosted' ? message.command.kind
        : message.type === 'command' ? message.command.kind : message.type;
      if (!['input', 'snapshot_ack', 'listener_view', 'ping'].includes(kind)) {
        state.commands.push(kind);
        state.trace.push({direction: 'sent', kind, connection: connection.path});
      }
    });
    socket.on('framereceived', ({payload}) => {
      const message = read(JSON.parse(payload.toString()));
      if (!message) return;
      if (message.type === 'welcome') {
        state.id = message.credentials.player_id;
        state.frame = message.frame;
        state.welcomes.push({playerId: state.id, sessionKind: message.hosted?.session_kind,
          path: connection.path, receivedAt: Date.now()});
      } else if (message.type === 'snapshot') state.frame = message.frame;
      else if (message.type === 'social') state.social = message.snapshot;
      else if (message.type === 'chat') {
        state.chats.push({sequence: message.message.sequence, channel: message.message.channel, text: message.message.text,
          sender: message.message.sender.display_name});
      } else if (message.type === 'error') state.errors.push(`host ${message.code}: ${message.reason}`);
      else if (message.type === 'chat_rejected') state.errors.push(`chat rejected: ${message.rejection.reason}`);
      else if (message.type === 'hosted') {
        const event = message.message;
        state.trace.push({direction: 'received', kind: event.kind, connection: connection.path});
        if (event.kind === 'player_card') state.cards.push(event.profile);
        else if (event.kind === 'college_invitations') {
          state.invitations.push({ids: event.invitations.map(invitation => invitation.id),
            inviters: event.invitations.map(invitation => invitation.inviter.display_name)});
        } else if (event.kind === 'party_action') {
          state.partyActions.push({action: event.action, accepted: event.accepted, reason: event.reason});
        }
      }
    });
  });
  return state;
}

async function openClient(account, label) {
  const server = await chromium.launchServer({headless: true, host: '127.0.0.1',
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
  const browser = await chromium.connect(server.wsEndpoint());
  const context = await browser.newContext({ignoreHTTPSErrors: process.env.LOCAL_TLS === '1',
    viewport: {width: 1280, height: 720}});
  // Bootstrap the real registration credential in the staging API namespace.
  // This tests authenticated social UI, not the separately owned account login form.
  await context.addInitScript(({key,token}) => localStorage.setItem(key,token), {key:tokenKey,token:account.token});
  const page = await context.newPage();
  page.setDefaultTimeout(30000);
  page.setDefaultNavigationTimeout(30000);
  const state = observe(page, label);
  await state.failureObserverReady;
  const client = {browser, server, page, state, account};
  clients.push(client);
  await page.goto(url, {waitUntil: 'load'});
  await ui.ready(page, {timeout:coldStartTimeout});
  state.authBootstrap = await page.evaluate(key => ({
    unscopedAbsent: key === 'sdr.token' || localStorage.getItem('sdr.token') === null,
    scopedPresent: Boolean(localStorage.getItem(key)),
  }), tokenKey);
  assert.deepEqual(state.authBootstrap, {unscopedAbsent:true, scopedPresent:true});
  assert.ok(state.requests.some(item => item.path.endsWith('/api/auth/me') && item.status === 200));
  state.renderer = await page.evaluate(() => {
    const gl = document.querySelector('canvas').getContext('webgl2');
    const info = gl.getExtension('WEBGL_debug_renderer_info');
    return info ? gl.getParameter(info.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER);
  });
  if (ui.findWidget(await ui.readUi(page),'prompt-secondary')) await ui.click(page,'prompt-secondary');
  await ui.click(page,'settings');
  await ui.click(page,'settings-page:cloud');
  if ((await ui.widget(page,'settings-toggle:shared-hub')).selected) await ui.click(page,'settings-toggle:shared-hub');
  await ui.value(page,'settings-toggle:shared-hub','false');
  await ui.click(page,'settings-back');
  await ui.widget(page,'settings-page:cloud');
  await ui.click(page,'settings-back');
  await ui.surface(page,'title');
  await ui.assertGpuOnly(page);
  return client;
}

async function newWizard(client, name) {
  const {page, state} = client;
  await startNewGame(page,{name,element:'fire',discipline:'arcane'});
  await waitUntil(async () => {
    if (state.errors.length > 0) await ui.settleErrors(page);
    if (state.errors.length > 0) throw new Error(state.errors.join('; '));
    return state.welcomes.length > 0 && state.social?.party_state;
  }, `${state.label} accepted private College`);
  await ui.surface(page,'hidden');
  // The paired Rust protocol retains its native enum spelling, independently of
  // the Website protocol's kebab-case session labels.
  assert.equal(state.welcomes.at(-1).sessionKind, 'PrivateCollege');
  assert.equal(state.social.world_kind, 'Hub');
  assert.equal(state.social.party_state.party.leader_player_id, state.id);
  assert.deepEqual(state.frame.host.connected_player_ids, [state.id]);
}

async function openGlobal(client) {
  const {page} = client;
  if (!ui.findWidget(await ui.readUi(page),'social/chat-message')) await ui.click(page,'social/chat-open');
  await ui.click(page,'social/channel:global');
}

async function sendGlobal(client, text) {
  await openGlobal(client);
  await ui.fill(client.page,'social/chat-message',text);
  await ui.click(client.page,'social/chat-send');
}

async function openCard(client, text, name) {
  await openGlobal(client);
  const message = client.state.chats.find(row => row.text === text && row.sender === name);
  assert.ok(message, 'the actual delivered message exists');
  const row = await ui.widget(client.page,`social/chat-message:${message.sequence}`);
  assert.ok(row.label.includes(text));
  await ui.click(client.page,`social/chat-author:${message.sequence}`);
  const card = await ui.widget(client.page,'social/player-card');
  assert.equal(card.label,name);
  return card;
}

function publicState(state) {
  return {label: state.label, playerId: state.id, renderer: state.renderer,
    authBootstrap: state.authBootstrap,
    welcomes: state.welcomes, connectedPlayerIds: state.frame?.host.connected_player_ids,
    sessionKind: state.social?.session_kind, worldKind: state.social?.world_kind,
    partyId: state.social?.party_state?.party.id,
    memberPlayerIds: state.social?.party_state?.party.member_player_ids,
    cards: state.cards.map(card => card && ({name: card.display_name,
      accountUsername: card.account_username, sessionKind: card.session_kind,
      activity: card.activity, playerId: card.player_id})),
    invitations: state.invitations, partyActions: state.partyActions,
    chats: state.chats, commands: state.commands, requests: state.requests,
    wasm: [...state.wasm], errors: state.errors,
    sockets: state.sockets, trace: state.trace, heldCheckpointSave: state.heldCheckpointSave};
}

async function closeClient(client) {
  const process = client.server.process();
  let mode = 'graceful';
  try {
    await bounded(client.browser.close(), 'Playwright disconnect', 5000);
    await bounded(client.server.close(), 'Chrome close', 15000);
  } catch {
    // The retained process belongs to this harness, never another fleet browser.
    mode = 'forced';
    await bounded(client.server.kill(), 'Chrome termination', 5000);
  }
  return {mode, exited: process.exitCode !== null || process.signalCode !== null};
}

(async () => {
  fs.mkdirSync(output, {recursive: true});
  let failure;
  let releaseCheckpointSave;
  try {
    const registered = await accounts();
    assert.equal(registered.length, 2);
    step = 'first private College';
    const leader = await openClient(registered[0], 'leader');
    await newWizard(leader, 'Severin');
    step = 'second private College';
    const guest = await openClient(registered[1], 'guest');
    await newWizard(guest, 'Octavia');
    const originalLeaderId = leader.state.id;
    const originalGuestId = guest.state.id;
    const originalGuestPath = guest.state.welcomes.at(-1).path;
    const originalGuestParty = guest.state.social.party_state.party.id;
    const assertSourceMembership = () => {
      assert.equal(guest.state.welcomes.length, 1);
      assert.equal(guest.state.id, originalGuestId);
      assert.equal(guest.state.welcomes.at(-1).path, originalGuestPath);
      assert.equal(guest.state.social.party_state.party.id, originalGuestParty);
      assert.deepEqual(guest.state.social.party_state.party.member_player_ids, [originalGuestId]);
      assert.deepEqual(guest.state.frame.host.connected_player_ids, [originalGuestId]);
      assert.equal(guest.state.sockets.find(socket => socket.path === originalGuestPath).closed, false);
    };
    assert.notEqual(leader.state.welcomes.at(-1).path, originalGuestPath);
    checked('two fresh registered accounts lead separate accepted private Colleges');

    step = 'cross-College Global chat and card';
    const greeting = `Social parity ${Date.now()}`;
    await sendGlobal(guest, greeting);
    await waitUntil(() => leader.state.chats.some(message => message.text === greeting
      && message.channel === 'Global'), 'cross-College Global delivery');
    await openCard(leader, greeting, 'Octavia');
    await ui.value(leader.page,'social/card-badge',`Registered · ${guest.account.username}`);
    assert.equal((await ui.widget(leader.page,'social/card-activity')).label, 'PRIVATE COLLEGE · IN COLLEGE');
    assert.equal(leader.state.cards.at(-1).account_username, guest.account.username);
    await ui.widget(leader.page,'social/card-message');
    await ui.widget(leader.page,'social/card-invite');
    await leader.page.screenshot({path: path.join(output, 'global-player-card.png')});
    checked('actual cross-College Global message resolves a registered Player Card with Message and Invite actions');

    step = 'first invitation and decline';
    await ui.click(leader.page,'social/card-invite');
    const invitation = await ui.widget(guest.page,'social/college-invitation');
    assert.ok(invitation.value.includes('Severin invited you to their private College.'));
    await guest.page.screenshot({path: path.join(output, 'college-invitation.png')});
    const invitationCount = guest.state.invitations.length;
    await ui.click(guest.page,'social/college-decline');
    await ui.noWidget(guest.page,'social/college-invitation');
    await waitUntil(() => guest.state.invitations.length > invitationCount
      && guest.state.invitations.at(-1).ids.length === 0, 'authority invitation dismissal');
    assert.equal(guest.state.welcomes.length, 1);
    assert.equal(guest.state.welcomes.at(-1).path, originalGuestPath);
    checked('declining dismisses the actual invitation and preserves the original College');

    step = 'second invitation and consent';
    await openCard(leader, greeting, 'Octavia');
    await ui.click(leader.page,'social/card-invite');
    await ui.widget(guest.page,'social/college-invitation');
    await ui.click(guest.page,'social/college-accept');
    const consent = await ui.widget(guest.page,'routing/dialog');
    const consentText = consent.value;
    assert.ok(consentText.includes("SEVERIN'S COLLEGE"));
    assert.ok(consentText.includes('LOCAL HALL ONLY · GLOBAL SCORES OFF'));
    assertSourceMembership();
    assert.ok(guest.state.requests.some(item => item.path.endsWith('/api/game/join/resolve') && item.status === 200));
    await guest.page.screenshot({path: path.join(output, 'college-consent.png')});
    checked('View & Join resolves the actual code and presents private-College consent before transfer');

    step = 'consent and accepted membership';
    const consentBoundary = guest.state.trace.length;
    const checkpointReleased = new Promise(resolve => {releaseCheckpointSave = resolve;});
    guest.state.heldCheckpointSave = {observed: false, released: false};
    await guest.page.route('**/api/game/saves/0', async route => {
      const requested = guest.state.trace.slice(consentBoundary).some(event =>
        event.direction === 'sent' && event.kind === 'save_before_leave'
        && event.connection === originalGuestPath);
      if (route.request().method() === 'PUT' && requested) {
        // Hold actual I/O without changing its URL, bytes, headers or response.
        guest.state.heldCheckpointSave.observed = true;
        await checkpointReleased;
      }
      await route.continue();
    });
    await ui.click(guest.page,'routing-continue');
    await waitUntil(() => guest.state.heldCheckpointSave.observed, 'actual old-authority checkpoint PUT');
    // Give the real render loop an opportunity to expose an early Create while
    // the persistence response is unavailable. This observes rAF; it steps no game.
    await guest.page.evaluate(() => new Promise(resolve =>
      requestAnimationFrame(() => requestAnimationFrame(resolve))));
    assertSourceMembership();
    assert.notEqual((await ui.readUi(guest.page)).surface, 'create');
    assert.equal(guest.state.trace.slice(consentBoundary).some(event => event.direction === 'sent'
      && event.kind === 'hello' && event.connection !== originalGuestPath), false);
    guest.state.heldCheckpointSave.sourceMembershipIntact = true;
    guest.state.heldCheckpointSave.createBlocked = true;
    await guest.page.screenshot({path: path.join(output, 'save-before-transfer-held.png')});
    checked('holding the real checkpoint PUT preserves the source College and prevents early Create');
    releaseCheckpointSave();
    guest.state.heldCheckpointSave.released = true;
    // The pinned source re-enters Create after consent even for an existing wizard.
    await ui.surface(guest.page,'create');
    const transfer = guest.state.trace.slice(consentBoundary);
    const saveRequested = transfer.findIndex(event => event.direction === 'sent'
      && event.kind === 'save_before_leave' && event.connection === originalGuestPath);
    assert.ok(saveRequested >= 0, 'source College must receive save_before_leave before Create');
    const saveAcknowledged = transfer.findIndex(event => event.direction === 'received'
      && event.kind === 'save_before_leave' && event.connection === originalGuestPath);
    assert.ok(saveAcknowledged > saveRequested, 'old authority must acknowledge its actual checkpoint');
    const savePersisted = transfer.findIndex((event, index) => index > saveAcknowledged
      && event.direction === 'response' && event.method === 'PUT'
      && event.path.endsWith('/api/game/saves/0') && event.status === 200);
    assert.ok(savePersisted > saveAcknowledged, 'actual checkpoint persistence must finish before Create');
    checked('consent saves and persists the old authority checkpoint before Create');
    await ui.fill(guest.page,'create-name','Octavia');
    await ui.click(guest.page,'element:fire');
    await ui.click(guest.page,'discipline:arcane');
    await waitUntil(() => guest.state.welcomes.length === 2 && guest.state.social?.party_state
      && guest.state.frame?.host.connected_player_ids.length === 2
      && leader.state.frame?.host.connected_player_ids.length === 2
      && guest.state.social.party_state.party.id === leader.state.social?.party_state?.party.id
      && leader.state.social.party_state.party.member_player_ids.length === 2,
    'shared accepted College authority');
    assert.equal(leader.state.id, originalLeaderId);
    assert.notEqual(guest.state.welcomes.at(-1).path, originalGuestPath);
    assert.equal(guest.state.welcomes.at(-1).path, leader.state.welcomes.at(-1).path);
    assert.equal(guest.state.social.party_state.party.id, leader.state.social.party_state.party.id);
    assert.deepEqual([...guest.state.frame.host.connected_player_ids].sort(), [originalLeaderId, guest.state.id].sort());
    assert.deepEqual([...leader.state.social.party_state.party.member_player_ids].sort(), [originalLeaderId, guest.state.id].sort());
    const finalTransfer = guest.state.trace.slice(consentBoundary);
    const newHello = finalTransfer.findIndex(event => event.direction === 'sent'
      && event.kind === 'hello' && event.connection !== originalGuestPath);
    assert.ok(newHello > savePersisted, 'new authority hello must follow old checkpoint persistence');
    await ui.surface(guest.page,'hidden');
    await ui.widget(guest.page,'social/party-settings-shortcut');
    await ui.click(guest.page,'social/party');
    await ui.widget(guest.page,'social/party-menu');
    await ui.widget(guest.page,'social/party-settings');
    await ui.widget(guest.page,'social/party-settings-shortcut');
    await ui.assertGpuOnly(guest.page);
    await guest.page.screenshot({path: path.join(output, 'joined-college-party.png')});
    checked('consent and actual character confirmation join the inviter authority with both connected party members');
    for (const client of clients) {
      await ui.settleErrors(client.page);
      assert.deepEqual(client.state.errors, []);
    }
    checked('both Metal browser clients completed without unexpected browser, API or host errors');
  } catch (error) {
    failure = {step, message: error.message, stack: error.stack};
    for (const client of clients) {
      await ui.settleErrors(client.page);
      await client.page.screenshot({path: path.join(output, `failure-${client.state.label}.png`)}).catch(() => {});
      client.state.failureUi = await ui.readUi(client.page).catch(() => null);
    }
    console.error(`Hosted social failed at ${step}: ${error.message}`);
    process.exitCode = 1;
  } finally {
    releaseCheckpointSave?.();
    const receipt = {passed: !failure, url, checks, failure,
      clients: clients.map(client => ({...publicState(client.state), failureUi: client.state.failureUi})),
      browsersClosed: false, servicesRestarted: false};
    // Preserve evidence before closing Chrome, including if the external bounded
    // job must terminate an unresponsive browser during cleanup.
    fs.writeFileSync(path.join(output, 'receipt.json'), JSON.stringify(receipt, null, 2));
    const closed = await Promise.allSettled(clients.map(closeClient));
    receipt.cleanup = closed.map(result => result.status === 'fulfilled'
      ? result.value : {error: result.reason.message});
    receipt.browsersClosed = closed.every(result => result.status === 'fulfilled' && result.value.exited);
    if (!receipt.browsersClosed) {
      receipt.passed = false;
      process.exitCode = 1;
    }
    receipt.clients = clients.map(client => ({...publicState(client.state), failureUi: client.state.failureUi}));
    fs.writeFileSync(path.join(output, 'receipt.json'), JSON.stringify(receipt, null, 2));
  }
})();
