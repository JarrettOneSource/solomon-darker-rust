// A real developer Console summon, local actor invitation, and learned Boneyard play.
// WebSocket listeners observe accepted traffic only; all actions use ordinary UI.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const {startNewGame} = require('./menu-journey.cjs');
const {createNativeMessageReader} = require('./native-wire.cjs');
const ui = require('./gpu-ui.cjs');
const output = path.resolve(process.env.OUT);
const account = JSON.parse(fs.readFileSync(process.env.ACCOUNTS))[0];
assert.equal(account.user.id, 3, 'the isolated configured QA3 developer account');
const url = new URL(process.env.URL || 'https://127.0.0.1:9449/game-stg/');
assert.equal(url.hostname, '127.0.0.1', 'the isolated owned gateway');
const websiteMount = process.env.WEBSITE_MOUNT ?? url.pathname.replace(/\/$/, '');
const tokenKey = websiteMount ? `sdr.token:${websiteMount}` : 'sdr.token';
const state = {frame: null, id: null, botId: null, social: null, welcomes: [], lua: [],
  commands: [], samples: [], ownedSpells: [], leaveReceipts: 0, actualDeveloper: false};
const checks = [], errors = [], requests = [], expectedNotFound = [], wasm = new Set();
const summonCode = 'return sd.bots.summon({element="water",discipline="mind"})';
let step = 'bootstrap';
let page;
const pause = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
const player = id => state.frame?.snapshot.participants[id];
function mark(label) {step = label; console.log(label);}
function check(label) {checks.push(label); console.log(label);}
async function until(predicate, label, timeout = 30000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    if (await predicate()) return;
    if (errors.length) await ui.settleErrors(page);
    if (errors.length) throw new Error(`Browser/host failure while waiting for ${label}: ${JSON.stringify(errors[0])}`);
    await pause(40);
  }
  throw new Error(`Timed out waiting for ${label}`);
}
function owns(value, id) {
  if (!value || typeof value !== 'object') return false;
  if (value.owner_id === id) return true;
  return Object.values(value).some(child => child && typeof child === 'object' && owns(child, id));
}
function sample() {
  const bot = player(state.botId);
  if (!bot || !state.frame.snapshot.boneyard) return;
  const tick = state.frame.snapshot.ticks;
  const rows = state.frame.snapshot.spells;
  const primary = (rows.projectiles || []).filter(row => owns(row, state.botId));
  const transients = (rows.transients || []).filter(row => owns(row, state.botId));
  if (state.samples.at(-1)?.tick === tick) return;
  state.samples.push({tick, position: bot.wizard.position, health: bot.player.combat.current_health,
    life: bot.player.combat.life_state, mana: bot.player.combat.current_mana,
    heading: bot.wizard.heading_index, primaryCast: bot.wizard.primary_cast,
    primaryCount: primary.length, transientCount: transients.length,
    waterParticleCount: transients.filter(row => Object.hasOwn(row, 'Water')).length,
    hold: {grace: Boolean(state.frame.host.resume_grace), pause: Boolean(state.frame.host.pause),
      loading: state.frame.host.loading_run_id, barrier: Boolean(state.frame.host.level_up_barrier)}});
  if ((primary.length || transients.length) && state.ownedSpells.length < 20)
    state.ownedSpells.push({tick, primary, transients});
}
function message(message) {
  if (message.type === 'welcome') {
    state.id = message.credentials.player_id;
    state.frame = message.frame;
    state.welcomes.push({kind: message.hosted?.session_kind,
      developer: message.hosted?.developer_access, observer: message.hosted?.observer});
  } else if (message.type === 'snapshot') state.frame = message.frame;
  else if (message.type === 'social') state.social = message.snapshot;
  else if (message.type === 'error') errors.push(`host ${message.code}: ${message.reason}`);
  else if (message.type === 'hosted') {
    const event = message.message;
    if (event.kind === 'lua_result') state.lua.push(event);
    if (event.kind === 'save_before_leave') state.leaveReceipts++;
  }
  sample();
}
function decode(socket) {
  const readMessage = createNativeMessageReader();
  socket.on('framereceived', ({payload}) => {
    try {
      const accepted = readMessage(JSON.parse(payload.toString()));
      if (accepted) message(accepted);
    } catch (error) {errors.push(`observed WS decode: ${error.message}`);}
  });
  socket.on('framesent', ({payload}) => {
    const row = JSON.parse(payload.toString());
    if (row.type === 'hosted' && row.command.kind === 'lua_execute')
      state.commands.push({type: 'lua_execute', requestId: row.command.request_id, code: row.command.code});
    else if (row.type === 'command') state.commands.push(row.command);
    else if (row.type === 'hosted' && row.command.kind === 'start_match')
      state.commands.push({type: 'start_match', choice: row.command.boneyard_id});
  });
}
(async () => {
  fs.mkdirSync(output, {recursive: true});
  const server = await chromium.launchServer({headless: true, host: '127.0.0.1',
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
  const browser = await chromium.connect(server.wsEndpoint());
  const context = await browser.newContext({ignoreHTTPSErrors: true, viewport: {width: 1280, height: 720}});
  await context.addInitScript(({key, token}) => localStorage.setItem(key, token),
    {key: tokenKey, token: account.token});
  page = await context.newPage();
  await ui.watchRequestFailures(page, {errors}).ready;
  page.setDefaultTimeout(30000);
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', entry => {
    if (entry.type() !== 'error') return;
    if (entry.text().includes('404') && /\/api\/game\/(saves\/0|rejoin)$/.test(new URL(entry.location().url || url).pathname)) return;
    errors.push(entry.text());
  });
  page.on('response', async response => {
    const pathname = new URL(response.url()).pathname;
    if (pathname.endsWith('.wasm')) wasm.add(path.basename(pathname));
    if (pathname.includes('/api/')) requests.push({path: pathname, status: response.status()});
    if (response.status() === 404 && /\/api\/game\/(saves\/0|rejoin)$/.test(pathname)) expectedNotFound.push(pathname);
    else if (response.status() >= 400) errors.push(`${response.status()} ${pathname}`);
    if (pathname.endsWith('/api/auth/me') && response.status() === 200) {
      const row = await response.json();
      state.actualDeveloper = (row.user || row).developerAccess === true;
    }
  });
  page.on('websocket', decode);
  try {
    mark('actual configured developer and fresh College');
    await page.goto(url.href, {waitUntil: 'load'});
    await ui.ready(page);
    await until(() => state.actualDeveloper, 'real auth/me developer admission');
    await startNewGame(page, {name:'BotQA', element:'fire', discipline:'arcane'});
    await until(() => state.frame?.snapshot.participants[state.id]
      && !player(state.id).player.economy.college_intro_pending
      && !state.frame.host.resume_grace, 'accepted post-loadout College');
    await ui.widget(page, (await ui.byLabel(page,'Enter the Boneyard',{role:'button'})).id);
    await ui.assertGpuOnly(page);
    assert.equal(state.welcomes.at(-1).kind, 'GlobalHub');
    assert.equal(state.welcomes.at(-1).developer, true);
    check('the configured real QA3 account reaches the Global College with developer capability');

    mark('ordinary developer Console summon');
    await page.keyboard.press('Backquote');
    await ui.click(page,'cheats/tab:console');
    await ui.fill(page,'cheats/lua-editor',summonCode);
    await ui.click(page,'cheats/run');
    await until(() => state.commands.some(row => row.type === 'lua_execute' && row.code === summonCode),
      'actual Console summon request');
    const summon = state.commands.find(row => row.type === 'lua_execute' && row.code === summonCode);
    await until(() => state.lua.some(row => row.request_id === summon.requestId), 'actual Lua execution completion');
    const result = state.lua.find(row => row.request_id === summon.requestId);
    assert.equal(result.ok, true, JSON.stringify(result));
    state.botId = result.values[0].player_id;
    assert.ok(state.botId.startsWith('bot-'));
    await page.keyboard.press('Backquote');
    await ui.noWidget(page,'cheats/lua-editor');
    await until(() => player(state.botId), 'actual bot participant');
    assert.equal(player(state.botId).player.selection.element, 'Water');
    check('trusted Console input summons one actual Water/Mind learned bot through the native VM');

    mark('ordinary local actor popup and delayed Party invitation');
    assert.equal(player(state.id).wizard.region, 'Courtyard');
    assert.equal(player(state.botId).wizard.region, 'Courtyard');
    const admittedAt = state.frame.snapshot.ticks;
    // The accepted actor must also reach the retained display timeline. Use the
    // ordinary nearest-actor interaction, with its actual card identity checked.
    await until(() => state.frame.snapshot.ticks >= admittedAt + 20, 'displayed bot admission');
    await page.keyboard.press('KeyE');
    const card = await ui.widget(page,'social/player-card');
    assert.equal(card.label, player(state.botId).player.selection.name,
      'the actual nearest actor opens the bot card');
    await ui.widget(page,'social/card-invite');
    const invitedAt = Date.now();
    await ui.click(page,'social/card-invite');
    await until(() => state.social?.party_state?.party.member_player_ids.includes(state.botId), 'bot Party acceptance');
    assert.ok(Date.now() - invitedAt >= 2900);
    if (ui.findWidget(await ui.readUi(page),'social/card-close')) await ui.click(page,'social/card-close');
    await page.screenshot({path: path.join(output, 'bot-party-college.png')});
    check('the real actor Invite sends the bot invitation and it accepts after three seconds');

    mark('real Boneyard and learned policy decisions');
    await ui.click(page,(await ui.byLabel(page,'Enter the Boneyard',{role:'button'})).id);
    await until(() => state.frame?.snapshot.boneyard && player(state.botId)
      && !state.frame.host.loading_run_id && !state.frame.host.resume_grace, 'actual two-player Boneyard readiness', 120000);
    const runId = state.frame.snapshot.boneyard.run_id;
    await until(() => state.samples.some(row => row.waterParticleCount > 0
      && row.primaryCast.cast_sequence > 0 && row.mana < 100), 'actual bot-owned Water primary spell', 90000);
    await until(() => new Set(state.samples.map(row => `${row.position.x},${row.position.y}`)).size >= 10,
      'actual learned movement across accepted authority ticks');
    await page.screenshot({path: path.join(output, 'learned-bot-boneyard.png')});
    const active = state.samples.filter(row => !row.hold.grace && !row.hold.pause && !row.hold.loading);
    assert.ok(active.length >= 10);
    assert.ok(state.ownedSpells.length > 0);
    assert.equal(state.frame.snapshot.boneyard.run_id, runId);
    assert.equal(state.commands.filter(row => row.type === 'lua_execute' && row.code === summonCode).length, 1,
      'no gameplay state is synthesized after the real summon');
    check('the retained native policy drives real Boneyard movement and bot-owned primary spell births');

    mark('ordinary save-before-leave and bot disposal');
    await page.keyboard.press('Escape');
    await ui.click(page,'leave');
    await ui.surface(page,'title');
    await ui.assertGpuOnly(page);
    assert.ok(state.leaveReceipts > 0);
    await ui.settleErrors(page);
    assert.deepEqual(errors, []);
    await page.screenshot({path: path.join(output, 'bot-title-cleanup.png')});
    check('the ordinary save receipt returns to Title and closes the owning developer session');
    const receipt = {passed: true, checks, errors, requests, expectedNotFound, wasm: [...wasm],
      source: 'retained Source behavior; actual Rust GPU input, native policy, Console, actor/Party/Boneyard UI',
      credentialBootstrap: 'retained isolated QA3 account token; no identity/authority injection',
      botId: state.botId, runId, welcomes: state.welcomes, commands: state.commands, samples: state.samples,
      ownedSpells: state.ownedSpells, lua: state.lua.map(({ok, error, values}) => ({ok, error, values}))};
    fs.writeFileSync(path.join(output, 'learned-bot.json'), JSON.stringify(receipt, null, 2));
    console.log(JSON.stringify({passed: true, checks, errors}));
  } catch (error) {
    await ui.settleErrors(page);
    await page.screenshot({path: path.join(output, 'learned-bot-failure.png')});
    fs.writeFileSync(path.join(output, 'learned-bot-failure.json'), JSON.stringify({passed: false, step,
      error: error.stack, checks, errors, requests, expectedNotFound, wasm: [...wasm],
      botId: state.botId, social: state.social, welcomes: state.welcomes, commands: state.commands,
      samples: state.samples, ownedSpells: state.ownedSpells, lua: state.lua, frame: state.frame,
      ui: await ui.readUi(page)}, null, 2));
    throw error;
  } finally {
    let timeout;
    try {await Promise.race([(async () => {await browser.close(); await server.close();})(), new Promise((resolve, reject) => {
      timeout = setTimeout(() => reject(new Error('Owned browser close timed out')), 15000);
    })]);} catch {await server.kill();} finally {clearTimeout(timeout);}
  }
})().catch(error => {console.error(error.message); process.exitCode = 1;});
