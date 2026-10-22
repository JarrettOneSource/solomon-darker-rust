// Actual Website accounts, ordinary UI and read-only socket observation.
// Expected flow: MainMenuScene.tsx, PlayerCardDialog.tsx,
// CollegeInvitationDialog.tsx and PartyJoinConsentDialog.tsx in pinned Website.
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const {createNativeMessageReader} = require('./native-wire.cjs');
const obsoleteCreateImage = require('./obsolete-create-image.cjs');
const {chromium, request} = require(path.resolve(process.env.PW));

const url = process.env.URL || 'https://127.0.0.1:9449/game-stg/';
const output = path.resolve(process.env.OUT);
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
    if (predicate()) return;
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
    errors: [], cancelledMedia: [], cancelledImages: [], resourceChecks: [], wasm: new Set(), sockets: [], trace: []};
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
  page.on('requestfailed', item => {
    const check = (async () => {
      const cancelled = await obsoleteCreateImage(page, item);
      if (cancelled) { state.cancelledImages.push(cancelled); return; }
      const failure = `${new URL(item.url()).pathname}: ${item.failure()?.errorText}`;
      if (item.resourceType() === 'media' && item.failure()?.errorText === 'net::ERR_ABORTED') {
        state.cancelledMedia.push(failure);
      } else state.errors.push(failure);
    })();
    state.resourceChecks.push(check);
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
        state.chats.push({channel: message.message.channel, text: message.message.text,
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
  await context.addInitScript(token => localStorage.setItem('sdr.token:/game-stg', token), account.token);
  const page = await context.newPage();
  page.setDefaultTimeout(30000);
  page.setDefaultNavigationTimeout(30000);
  const state = observe(page, label);
  const client = {browser, server, page, state, account};
  clients.push(client);
  await page.goto(url, {waitUntil: 'load'});
  await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0
    && document.querySelector('#status')?.textContent === '', null, {timeout: coldStartTimeout});
  state.authBootstrap = await page.evaluate(() => ({
    unscopedAbsent: localStorage.getItem('sdr.token') === null,
    scopedPresent: Boolean(localStorage.getItem('sdr.token:/game-stg')),
  }));
  assert.deepEqual(state.authBootstrap, {unscopedAbsent: true, scopedPresent: true});
  assert.ok(state.requests.some(item => item.path.endsWith('/api/auth/me') && item.status === 200));
  state.renderer = await page.evaluate(() => {
    const gl = document.querySelector('canvas').getContext('webgl2');
    const info = gl.getExtension('WEBGL_debug_renderer_info');
    return info ? gl.getParameter(info.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER);
  });
  if (await page.locator('.title-prompt:not([hidden])').isVisible()) {
    await page.locator('.title-prompt:not([hidden]) [data-menu-action="prompt-secondary"]').click();
  }
  await page.locator('[data-menu-action="settings"]').click();
  await page.locator('[data-menu-action="settings-page:cloud"]').click();
  const shared = page.locator('[data-menu-action="settings-toggle:shared-hub"]');
  await shared.click();
  await page.waitForFunction(() => document.querySelector('[data-menu-action="settings-toggle:shared-hub"]')?.getAttribute('aria-pressed') === 'false');
  await page.locator('[data-menu-action="settings-back"]').click();
  await page.locator('[data-settings-page="root"]').waitFor();
  await page.locator('[data-menu-action="settings-back"]').click();
  await page.locator('.game-menus[data-menu-screen="title"]').waitFor();
  return client;
}

async function newWizard(client, name) {
  const {page, state} = client;
  await page.locator('[data-menu-action="play"]').click();
  await page.locator('[data-menu-action="new-game"]').click();
  await page.locator('.game-menus[data-menu-screen="create"], [role="dialog"][aria-label="Kill character?"]').first().waitFor();
  const replace = page.getByRole('dialog', {name: 'Kill character?', exact: true});
  if (await replace.isVisible()) {
    // Retries replace only these test accounts' own previously admitted wizard.
    await replace.locator('[data-menu-action="prompt-primary"]').click();
  }
  await page.locator('.game-menus[data-menu-screen="create"]').waitFor();
  await page.getByRole('textbox', {name: 'Wizard name', exact: true}).fill(name);
  await page.locator('[data-menu-action="element:fire"]').click();
  await page.locator('[data-menu-action="discipline:arcane"]').click();
  await waitUntil(() => {
    if (state.errors.length > 0) throw new Error(state.errors.join('; '));
    return state.welcomes.length > 0 && state.social?.party_state;
  }, `${state.label} accepted private College`);
  await page.locator('.game-menus[data-menu-screen="hidden"]').waitFor({state: 'attached'});
  // The paired Rust protocol retains its native enum spelling, independently of
  // the Website protocol's kebab-case session labels.
  assert.equal(state.welcomes.at(-1).sessionKind, 'PrivateCollege');
  assert.equal(state.social.world_kind, 'Hub');
  assert.equal(state.social.party_state.party.leader_player_id, state.id);
  assert.deepEqual(state.frame.host.connected_player_ids, [state.id]);
}

async function openGlobal(client) {
  const {page} = client;
  if (await page.locator('.game-chat').getAttribute('data-chat-open') !== 'true') {
    await page.locator('[data-social-action="chat-open"]').click();
  }
  await page.locator('[data-social-action="channel:global"]').click();
}

async function sendGlobal(client, text) {
  await openGlobal(client);
  await client.page.getByRole('textbox', {name: 'Chat message', exact: true}).fill(text);
  await client.page.getByRole('button', {name: 'Send', exact: true}).click();
}

async function openCard(client, text, name) {
  await openGlobal(client);
  const row = client.page.locator('.game-chat-message').filter({hasText: text});
  await row.getByRole('button', {name, exact: true}).click();
  const card = client.page.getByRole('dialog', {name, exact: true});
  await card.waitFor();
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
    wasm: [...state.wasm], errors: state.errors, cancelledMedia: state.cancelledMedia,
    cancelledImages: state.cancelledImages,
    sockets: state.sockets, trace: state.trace, heldCheckpointSave: state.heldCheckpointSave};
}

async function closeClient(client) {
  const process = client.server.process();
  let mode = 'graceful';
  try {
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
    let card = await openCard(leader, greeting, 'Octavia');
    // innerText applies the card CSS uppercase transform. textContent retains
    // the admitted account spelling used by the source profile and DOM producer.
    assert.equal(await card.locator('.hub-player-profile-badge').textContent(), `Registered · ${guest.account.username}`);
    assert.equal(await card.locator('.hub-player-profile-activity').textContent(), 'PRIVATE COLLEGE · IN COLLEGE');
    assert.equal(leader.state.cards.at(-1).account_username, guest.account.username);
    await card.getByRole('button', {name: 'Message', exact: true}).waitFor();
    await card.getByRole('button', {name: 'Invite to Party', exact: true}).waitFor();
    await leader.page.screenshot({path: path.join(output, 'global-player-card.png')});
    checked('actual cross-College Global message resolves a registered Player Card with Message and Invite actions');

    step = 'first invitation and decline';
    await card.getByRole('button', {name: 'Invite to Party', exact: true}).click();
    const invitation = guest.page.getByRole('dialog', {name: 'Private College invitation', exact: true});
    await invitation.waitFor();
    assert.ok((await invitation.textContent()).includes('Severin invited you to their private College.'));
    await guest.page.screenshot({path: path.join(output, 'college-invitation.png')});
    const invitationCount = guest.state.invitations.length;
    await invitation.getByRole('button', {name: 'DECLINE', exact: true}).click();
    await invitation.waitFor({state: 'hidden'});
    await waitUntil(() => guest.state.invitations.length > invitationCount
      && guest.state.invitations.at(-1).ids.length === 0, 'authority invitation dismissal');
    assert.equal(guest.state.welcomes.length, 1);
    assert.equal(guest.state.welcomes.at(-1).path, originalGuestPath);
    checked('declining dismisses the actual invitation and preserves the original College');

    step = 'second invitation and consent';
    card = await openCard(leader, greeting, 'Octavia');
    await card.getByRole('button', {name: 'Invite to Party', exact: true}).click();
    await invitation.waitFor();
    await invitation.getByRole('button', {name: 'VIEW & JOIN', exact: true}).click();
    const consent = guest.page.getByRole('dialog', {name: 'Join party consent', exact: true});
    await consent.waitFor();
    const consentText = await consent.textContent();
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
    await consent.getByRole('button', {name: 'JOIN PARTY', exact: true}).click();
    await waitUntil(() => guest.state.heldCheckpointSave.observed, 'actual old-authority checkpoint PUT');
    // Give the real render loop an opportunity to expose an early Create while
    // the persistence response is unavailable. This observes rAF; it steps no game.
    await guest.page.evaluate(() => new Promise(resolve =>
      requestAnimationFrame(() => requestAnimationFrame(resolve))));
    assertSourceMembership();
    assert.notEqual(await guest.page.locator('.game-menus').getAttribute('data-menu-screen'), 'create');
    assert.equal(guest.state.trace.slice(consentBoundary).some(event => event.direction === 'sent'
      && event.kind === 'hello' && event.connection !== originalGuestPath), false);
    guest.state.heldCheckpointSave.sourceMembershipIntact = true;
    guest.state.heldCheckpointSave.createBlocked = true;
    await guest.page.screenshot({path: path.join(output, 'save-before-transfer-held.png')});
    checked('holding the real checkpoint PUT preserves the source College and prevents early Create');
    releaseCheckpointSave();
    guest.state.heldCheckpointSave.released = true;
    // The pinned source re-enters Create after consent even for an existing wizard.
    await guest.page.locator('.game-menus[data-menu-screen="create"]').waitFor();
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
    await guest.page.getByRole('textbox', {name: 'Wizard name', exact: true}).fill('Octavia');
    await guest.page.locator('[data-menu-action="element:fire"]').click();
    await guest.page.locator('[data-menu-action="discipline:arcane"]').click();
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
    await guest.page.locator('.game-menus[data-menu-screen="hidden"]').waitFor({state: 'attached'});
    await guest.page.locator('[data-social-action="party"]').click();
    await guest.page.getByRole('dialog', {name: 'PARTY', exact: true}).waitFor();
    await guest.page.screenshot({path: path.join(output, 'joined-college-party.png')});
    checked('consent and actual character confirmation join the inviter authority with both connected party members');
    await Promise.all(clients.flatMap(client => client.state.resourceChecks));
    for (const client of clients) assert.deepEqual(client.state.errors, []);
    checked('both Metal browser clients completed without unexpected browser, API or host errors');
  } catch (error) {
    failure = {step, message: error.message, stack: error.stack};
    for (const client of clients) {
      await client.page.screenshot({path: path.join(output, `failure-${client.state.label}.png`)}).catch(() => {});
      client.state.failureUi = await client.page.evaluate(() => ({
        menu: document.querySelector('.game-menus')?.dataset.menuScreen,
        status: document.querySelector('#status')?.textContent,
        dialogs: [...document.querySelectorAll('[role="dialog"]')]
          .filter(node => node.getClientRects().length > 0).map(node => ({
            label: node.getAttribute('aria-label'), text: node.innerText,
          })),
      })).catch(() => null);
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
