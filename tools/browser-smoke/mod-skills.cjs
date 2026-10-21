// Unchanged Gravity Lesson, admitted through Cloud and exercised through real UI.
// Source: mod-ui/{ModSkillPicker,ModSkillBook,ModSkillQuickbar}.tsx and
// mod-skill-quickbar-keyboard.ts. Socket hooks only observe actual traffic.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const {reachCollegeLoadout} = require('./menu-journey.cjs');

const output = path.resolve(process.env.OUT);
const contentDirectory = path.resolve(process.env.CONTENT_DIR);
const account = JSON.parse(fs.readFileSync(path.join(contentDirectory, 'private-account.json')));
const acceptedPackage = JSON.parse(fs.readFileSync(path.join(contentDirectory, 'package.json')));
const url = new URL(process.env.URL || 'https://127.0.0.1:9449/game-stg/');
assert.equal(url.origin, 'https://127.0.0.1:9449');
assert.equal(url.pathname, '/game-stg/');
assert.equal(acceptedPackage.packageId, 'showcase.gravity-lesson');

const state = {id: null, frame: null, runtime: null, content: null, welcomes: [],
  actions: [], casts: [], luaRequests: [], luaResults: [], statuses: [], effects: [],
  checkpoints: 0, leaveReceipts: 0};
const checks = [], errors = [], requests = [], cancelledMedia = [], expectedNotFound = [];
const bundles = new Set();
let step = 'bootstrap';
let gravitySkill, gravitySpell;
const pause = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
const participant = () => state.frame?.snapshot.participants[state.id];
const player = () => participant()?.player;
const wizard = () => participant()?.wizard;
const lessonCasts = () => state.runtime?.state_values?.find(row =>
  row.key === 'gravity.lesson.casts' && row.scope_kind === 'participant-run'
    && row.scope_id === `${state.id}:${state.frame?.snapshot.boneyard?.run_id ?? 'profile'}`)?.value || 0;
const binding = () => state.runtime?.mod_quickbar?.find(row => row.content_id === gravitySpell);
const quickbar = page => page.getByRole('navigation', {name: 'Mod spell quickbar', exact: true});
const book = page => page.getByRole('complementary', {name: 'Mod skills', exact: true});
const editor = page => page.locator('#cheat-menu-lua-editor');

async function until(predicate, label) {
  const deadline = Date.now() + 30000;
  while (Date.now() < deadline) {
    if (await predicate()) return;
    await pause(40);
  }
  throw new Error(`Timed out waiting for ${label}`);
}
function mark(label) {step = label; console.log(label);}
function check(label) {checks.push(label); console.log(label);}
function snapshot() {
  return {tick: state.frame?.snapshot.ticks, playerId: state.id,
    name: player()?.selection.name, experience: player()?.progression.experience,
    sessionKind: state.welcomes.at(-1)?.session_kind, level: player()?.progression.level,
    pendingOffer: player()?.progression.pending_offer?.sequence,
    mana: player()?.combat.current_mana, position: wizard()?.position,
    headingIndex: wizard()?.heading_index, runtimeRevision: state.runtimeRevision,
    ranks: state.runtime?.skill_ranks, offers: state.runtime?.skill_offers,
    spells: state.runtime?.spells, bindings: state.runtime?.mod_quickbar,
    lessonCasts: lessonCasts()};
}
function expectedTarget(character) {
  const radians = character.heading_index * 15 * Math.PI / 180;
  return {x: character.position.x + Math.sin(radians) * 300,
    y: character.position.y - Math.cos(radians) * 300};
}
async function hiddenMenu(page) {
  await page.locator('.game-menus[data-menu-screen="hidden"]').waitFor({state: 'attached'});
}
async function settledAuthority() {
  await until(() => player() && !player().economy.college_intro_pending
    && !state.frame.host.loading_run_id && !state.frame.host.resume_grace
    && !state.frame.host.pause && !state.frame.host.level_up_barrier,
  'unheld accepted College authority');
}
async function openBook(page) {
  await page.locator('.hud-tome').click();
  await book(page).waitFor();
}
async function closeBook(page) {
  await page.keyboard.press('Escape');
  await book(page).waitFor({state: 'hidden'});
}
async function openConsole(page) {
  await page.keyboard.press('Backquote');
  await page.locator('[data-cheat-action="console"]').click();
  await editor(page).waitFor();
  await editor(page).fill('');
  await editor(page).focus();
}
async function closeConsole(page) {
  await page.keyboard.press('Backquote');
  await editor(page).waitFor({state: 'hidden'});
}
async function readyToCast() {
  await until(() => {
    const cooldown = state.runtime?.spell_cooldowns?.find(row => row.content_id === gravitySpell);
    return player()?.combat.current_mana >= 22
      && (!cooldown || state.frame.snapshot.ticks >= cooldown.ready_tick);
  }, 'actual spell mana and cooldown');
  const position = {...wizard().position};
  const tick = state.frame.snapshot.ticks;
  await until(() => state.frame.snapshot.ticks >= tick + 10, 'two later stationary snapshots');
  assert.deepEqual(wizard().position, position, 'cast target is sampled while the accepted wizard is stationary');
}
async function acceptedCast(input, label) {
  await readyToCast();
  const count = state.casts.length;
  const lesson = lessonCasts();
  await input();
  await until(() => state.casts.length > count, `${label} real ModCast command`);
  assert.equal(state.casts.length, count + 1, `${label} emits one command`);
  const command = state.casts[count];
  assert.equal(command.contentId, gravitySpell);
  assert.deepEqual(command.target, command.expectedTarget,
    'Source casts 300 units along the actual accepted heading, independent of pointer aim');
  await until(() => lessonCasts() === lesson + 1, `${label} actual reducer execution`);
  await until(() => state.effects.some(effect => effect.content_id === gravitySpell
    && effect.started_tick >= command.tick && effect.owner_player_id === state.id),
  `${label} actual area effect`);
  return command;
}
async function noCast(input, label) {
  const count = state.casts.length;
  await input();
  await pause(250);
  assert.equal(state.casts.length, count, label);
}
async function numericKey(page) {
  await page.keyboard.down('Shift');
  await page.keyboard.press('Numpad1');
  await page.keyboard.up('Shift');
}
async function keySamples(page) {
  return page.evaluate(() => window.modSkillKeySamples);
}
async function geometry(page, selector) {
  return page.locator(selector).evaluate(element => {
    const rect = element.getBoundingClientRect();
    const style = getComputedStyle(element);
    const image = element.querySelector('img');
    return {x: rect.x, y: rect.y, width: rect.width, height: rect.height,
      fontSize: style.fontSize, padding: style.padding, gap: style.gap,
      image: image && {loaded: image.complete && image.naturalWidth > 0,
        width: image.naturalWidth, height: image.naturalHeight,
        verifiedBlob: image.src.startsWith('blob:')}};
  });
}

(async () => {
  fs.mkdirSync(output, {recursive: true});
  const server = await chromium.launchServer({headless: true, host: '127.0.0.1',
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
  const browser = await chromium.connect(server.wsEndpoint());
  const context = await browser.newContext({ignoreHTTPSErrors: process.env.LOCAL_TLS === '1',
    viewport: {width: 1280, height: 720}});
  await context.addInitScript(token => {
    // This mod journey reuses its real isolated API account. Mounted form sign-in
    // has its own acceptance driver; no credential or authority is fabricated.
    localStorage.setItem('sdr.token:/game-stg', token);
    window.modSkillKeySamples = [];
    window.addEventListener('keydown', event => {
      if (event.code !== 'Digit1' && event.code !== 'Numpad1') return;
      const row = {key: event.key, code: event.code, repeat: event.repeat,
        shift: event.shiftKey, control: event.ctrlKey, alt: event.altKey,
        trusted: event.isTrusted, target: event.target.id || event.target.tagName};
      window.modSkillKeySamples.push(row);
      // Chrome can flush a capture listener's microtasks before bubbling. Read
      // the final cancellation after the complete dispatch, without changing it.
      setTimeout(() => {row.defaultPrevented = event.defaultPrevented;}, 0);
    }, true);
  }, account.token);
  const page = await context.newPage();
  page.setDefaultTimeout(30000);
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error') return;
    const pathname = new URL(message.location().url || url).pathname;
    if (message.text().includes('404') && (/\/api\/game\/saves\/0$/.test(pathname)
      || pathname.endsWith('/api/game/rejoin'))) return;
    errors.push(message.text());
  });
  page.on('requestfailed', request => {
    const failure = {path: new URL(request.url()).pathname, error: request.failure()?.errorText};
    if (request.resourceType() === 'media' && failure.error === 'net::ERR_ABORTED') cancelledMedia.push(failure);
    else errors.push(`${failure.path}: ${failure.error}`);
  });
  page.on('request', request => {
    const requested = new URL(request.url());
    if (requested.protocol === 'https:' && requested.origin === url.origin
      && !requested.pathname.startsWith('/game-stg/'))
      errors.push(`out-of-mount request ${requested.pathname}`);
  });
  page.on('response', response => {
    const pathname = new URL(response.url()).pathname;
    if (pathname.includes('/api/')) requests.push({path: pathname,
      status: response.status(), method: response.request().method()});
    if (pathname.endsWith('.wasm')) bundles.add(path.basename(pathname));
    if (response.status() === 404 && (/\/api\/game\/saves\/0$/.test(pathname)
      || pathname.endsWith('/api/game/rejoin'))) expectedNotFound.push(pathname);
    else if (response.status() >= 400) errors.push(`${response.status()} ${pathname}`);
  });
  page.on('websocket', socket => {
    socket.on('framesent', ({payload}) => {
      const message = JSON.parse(payload.toString());
      if (message.type !== 'hosted') return;
      const command = message.command;
      if (command.kind === 'mod_action') state.actions.push({action: command.action,
        target: command.target, arguments: command.arguments, requestId: command.request_id});
      else if (command.kind === 'mod_cast') state.casts.push({contentId: command.content_id,
        target: {x: command.target_x, y: command.target_y}, requestId: command.request_id,
        tick: state.frame.snapshot.ticks, expectedTarget: expectedTarget(wizard())});
      else if (command.kind === 'lua_execute') state.luaRequests.push({requestId: command.request_id,
        code: command.code});
    });
    socket.on('framereceived', ({payload}) => {
      const message = JSON.parse(payload.toString());
      if (message.type === 'welcome') {
        state.id = message.credentials.player_id;
        state.frame = message.frame;
        state.welcomes.push(message.hosted);
      } else if (message.type === 'snapshot') state.frame = message.frame;
      else if (message.type === 'error') errors.push(`host ${message.code}: ${message.reason}`);
      else if (message.type === 'hosted') {
        const event = message.message;
        if (event.kind === 'mod_runtime') {
          state.runtime = event.projection;
          state.runtimeRevision = event.revision;
          for (const effect of event.projection.spell_effects || []) {
            if (!state.effects.some(previous => previous.id === effect.id)) state.effects.push(effect);
          }
        } else if (event.kind === 'mod_content') {
          state.content = event.projection;
          for (const status of event.projection.statuses || []) {
            if (!state.statuses.some(previous => previous.instanceId === status.instanceId))
              state.statuses.push(status);
          }
        } else if (event.kind === 'lua_result') state.luaResults.push(event);
        else if (event.kind === 'save_before_leave') state.leaveReceipts++;
        else if (event.kind === 'save_checkpoint') state.checkpoints++;
      }
    });
  });
  try {
    await page.goto(url.href, {waitUntil: 'load'});
    await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0
      && document.querySelector('#status')?.textContent === '');
    const prompt = page.locator('.title-prompt:not([hidden])');
    if (await prompt.isVisible()) await prompt.locator('[data-menu-action="prompt-secondary"]').click();
    assert.equal(await page.evaluate(() => localStorage.getItem('sdr.token')), null);

    mark('enable real private cheats and subscribe through Cloud');
    await page.locator('[data-menu-action="settings"]').click();
    await page.locator('[data-menu-action="settings-page:cloud"]').click();
    const cheats = page.locator('[data-menu-action="settings-toggle:cheats"]');
    if (await cheats.getAttribute('aria-pressed') === 'false') await cheats.click();
    const shared = page.locator('[data-menu-action="settings-toggle:shared-hub"]');
    if (await shared.getAttribute('aria-pressed') === 'true') await shared.click();
    await page.locator('[data-menu-action="settings-back"]').click();
    await page.locator('[data-menu-action="settings-back"]').click();
    await page.getByRole('button', {name: 'Explore the Dark Cloud', exact: true}).click();
    await page.locator('.dark-cloud-rows[aria-busy="false"]').waitFor();
    const subscribe = page.locator(`[data-menu-action="cloud-subscription:subscribe:${acceptedPackage.slug}"]`);
    if (await subscribe.count()) await subscribe.click();
    await page.locator(`[data-mod-slug="${acceptedPackage.slug}"] .dark-cloud-row-state.enabled`).waitFor();
    await page.screenshot({path: path.join(output, 'gravity-enabled.png')});
    check('the real QA account subscribes to the unchanged Source Gravity Lesson package');

    mark('start the real private College with its accepted package');
    await page.keyboard.press('Escape');
    await page.getByRole('button', {name: 'MAIN MENU', exact: true}).click();
    await page.locator('[data-menu-action="play"]').click();
    await page.locator('[data-menu-action="new-game"]').click();
    const consent = page.getByRole('dialog', {name: 'Local play is active', exact: true});
    // A previous failed attempt may already have saved this QA wizard. Source
    // requires its real kill-character confirmation before starting another.
    await until(async () => await prompt.isVisible() || await consent.isVisible(),
      'the real New Game confirmation or content consent');
    if (await prompt.isVisible()) await prompt.locator('[data-menu-action="prompt-primary"]').click();
    await consent.waitFor();
    assert.ok((await consent.innerText()).includes('Gravity Lesson'));
    await consent.getByRole('button', {name: 'CONTINUE LOCAL', exact: true}).click();
    await until(async () => await page.locator('.game-menus[data-menu-screen="create"]').count()
      || await page.locator('.player-panels:not([hidden]) [data-player-panel="dialogue"]').count(),
    'actual College admission or Create');
    if (await page.locator('.player-panels:not([hidden]) [data-player-panel="dialogue"]').count())
      await reachCollegeLoadout(page);
    const name = page.getByRole('textbox', {name: 'Wizard name', exact: true});
    await name.fill('Gravitas');
    assert.equal(await name.inputValue(), 'Gravitas', 'the real Source-width name input accepts this name');
    await page.locator('[data-menu-action="element:ether"]').click();
    await page.locator('[data-menu-action="discipline:arcane"]').click();
    await hiddenMenu(page);
    await until(() => state.runtime?.skills?.some(row => row.name === 'Gravity Student') && player(),
      'the accepted prepared skills runtime');
    await settledAuthority();
    assert.equal(player().selection.name, 'Gravitas');
    const welcome = state.welcomes.at(-1);
    assert.equal(welcome.session_kind, 'PrivateCollege');
    assert.equal(welcome.cheats_enabled, true);
    assert.equal(welcome.content.mods.find(mod => mod.id === acceptedPackage.packageId)?.contentSha256,
      acceptedPackage.contentSha256);
    gravitySkill = state.runtime.skills.find(row => row.name === 'Gravity Student').content_id;
    assert.ok(!state.runtime.skill_ranks.some(row => row.content_id === gravitySkill),
      'use a fresh QA profile: Source intentionally retains authored ranks across New Game');
    check('the actual authority admits private cheats and the exact accepted content digest');

    mark('earn the native offer with the real stock Grant XP control');
    // Source grantPlayerExperience requires XP strictly above the threshold.
    const experience = Math.floor(player().progression.next_threshold - player().progression.experience) + 1;
    assert.ok(experience > 0);
    await page.keyboard.press('Backquote');
    await page.getByRole('spinbutton', {name: 'EXPERIENCE', exact: true}).fill(String(experience));
    const luaBefore = state.luaRequests.length;
    await page.getByRole('button', {name: 'GRANT XP', exact: true}).click();
    await until(() => state.luaRequests.length > luaBefore, 'real stock Cheat XP Lua request');
    await until(() => player()?.progression.level >= 2 && player().progression.pending_offer,
      'the real derived native level offer');
    const picker = page.getByRole('complementary', {name: 'Mod skill choices', exact: true});
    await picker.waitFor();
    await picker.getByRole('button', {name: /Gravity Student/}).waitFor();
    const pickerGeometry = await geometry(page, '.mod-skill-picker');
    assert.ok(Math.abs(pickerGeometry.width - 400) <= 1 / 64, 'Source picker width is 25 × 16px');
    await page.screenshot({path: path.join(output, 'gravity-picker.png')});
    const nativeSequence = player().progression.pending_offer.sequence;
    const modSequence = state.runtime.skill_offers.find(row => row.player_id === state.id).sequence;
    await picker.getByRole('button', {name: /Gravity Student/}).click();
    await until(() => state.runtime.skill_ranks.some(row => row.content_id === gravitySkill && row.rank === 1)
      && state.runtime.spells.some(row => row.name === 'Gravity Well')
      && !player().progression.pending_offer, 'accepted authored skill choice and spell unlock');
    gravitySpell = state.runtime.spells.find(row => row.name === 'Gravity Well').content_id;
    assert.ok(state.actions.some(action => action.action === 'skill-choose' && action.target === gravitySkill
      && action.arguments.native_offer_sequence === nativeSequence
      && action.arguments.mod_offer_sequence === modSequence));
    await settledAuthority();
    check('real Cheat XP produces a native offer, Source-sized mod picker, and accepted Gravity Student unlock');

    mark('bind Gravity Well through the real Skill Book');
    await openBook(page);
    assert.ok((await book(page).innerText()).includes('Rank 1 / 3'));
    const spellButton = book(page).getByRole('button', {name: /Gravity Well/});
    assert.ok((await spellButton.innerText()).includes('Add to slot 1'));
    await spellButton.click();
    await until(() => binding()?.slot === 0, 'actual slot-one binding');
    await quickbar(page).waitFor();
    const bookGeometry = await geometry(page, '.mod-skill-book');
    assert.ok(Math.abs(bookGeometry.width - 288) <= 1 / 64, 'Source Book width is 18 × 16px');
    const quickbarGeometry = await geometry(page, '.mod-skill-quickbar button');
    assert.ok(Math.abs(quickbarGeometry.width - 41.6) <= 1 / 64,
      'Source quickbar width retains Chromium layout-unit quantization only');
    assert.ok(quickbarGeometry.image.loaded && quickbarGeometry.image.verifiedBlob);
    assert.equal(quickbarGeometry.image.width, 100);
    assert.equal(quickbarGeometry.image.height, 100);
    await page.screenshot({path: path.join(output, 'gravity-book-bound.png')});
    await closeBook(page);
    check('the real Skill Book binds its unlocked spell, verified icon and Source-sized quickbar');

    mark('cast the actual authored area with the quickbar button');
    await acceptedCast(() => quickbar(page).getByTitle('Gravity Well (Shift+1)', {exact: true}).click(),
      'quickbar click');
    await page.locator('.mod-spell-effect__area').waitFor();
    await until(() => state.statuses.some(status => status.targetId === state.id), 'actual Steady caster status');
    await page.screenshot({path: path.join(output, 'gravity-area-cast.png')});
    check('a real quickbar click reaches the heading target, area actor, Steady status and lesson reducer');

    mark('check Source keyboard behavior with actual editable Console focus');
    await openConsole(page);
    await noCast(() => page.keyboard.press('Shift+Digit1'), 'US Shift+1 is !, so Source sends no cast');
    assert.equal(await editor(page).inputValue(), '!');
    await editor(page).fill('');
    await noCast(() => page.keyboard.press('Control+Shift+Numpad1'), 'Control excludes the quickbar shortcut');
    await editor(page).fill('');
    await acceptedCast(async () => {
      const count = state.casts.length;
      await page.keyboard.down('Shift');
      await page.keyboard.down('Numpad1');
      await until(() => state.casts.length === count + 1, 'the trusted numeric keypad cast');
      await noCast(() => page.keyboard.down('Numpad1'), 'a repeated held key sends no second cast');
      await page.keyboard.up('Numpad1');
      await page.keyboard.up('Shift');
    }, 'focused Console keypad');
    const focusedKeys = await keySamples(page);
    assert.ok(focusedKeys.some(row => row.key === '1' && row.code === 'Numpad1'
      && row.trusted && row.defaultPrevented && row.target === 'cheat-menu-lua-editor' && !row.repeat));
    assert.ok(focusedKeys.some(row => row.repeat && row.code === 'Numpad1' && !row.defaultPrevented));
    assert.equal(await editor(page).evaluate(element => element === document.activeElement), true);
    await page.screenshot({path: path.join(output, 'gravity-focused-shortcut.png')});
    await closeConsole(page);
    check('trusted editable-focus numeric key casts; US !, Control and repeated keys preserve Source exclusions');

    mark('unbind and rebind through the real Skill Book');
    await openBook(page);
    assert.ok((await spellButton.innerText()).includes('Remove from slot 1'));
    await spellButton.click();
    await until(() => !binding(), 'actual binding removal');
    await quickbar(page).waitFor({state: 'hidden'});
    await closeBook(page);
    await openConsole(page);
    const keysBefore = (await keySamples(page)).length;
    await noCast(() => numericKey(page), 'an empty quickbar has no stale shortcut');
    const unboundKeys = (await keySamples(page)).slice(keysBefore);
    assert.ok(unboundKeys.some(row => row.key === '1' && !row.defaultPrevented));
    await closeConsole(page);
    await openBook(page);
    await spellButton.click();
    await until(() => binding()?.slot === 0, 'actual slot-one rebinding');
    await closeBook(page);
    await openConsole(page);
    await acceptedCast(() => numericKey(page), 'rebound Console keypad');
    await closeConsole(page);
    check('real unbind removes the listener and rebinding restores exactly one cast listener');

    mark('leave the actual session and check quickbar disposal');
    const leaveBefore = state.leaveReceipts;
    await page.keyboard.press('Escape');
    await page.locator('[data-menu-action="leave"]').click();
    await page.locator('.game-menus[data-menu-screen="title"]').waitFor();
    assert.ok(state.leaveReceipts > leaveBefore);
    assert.ok(state.checkpoints > 0);
    await quickbar(page).waitFor({state: 'hidden'});
    await noCast(() => numericKey(page), 'leaving the session removes the quickbar listener');
    assert.equal(await page.evaluate(() => localStorage.getItem('sdr.token')), null);
    assert.ok(await page.evaluate(() => Boolean(localStorage.getItem('sdr.token:/game-stg'))));
    await page.screenshot({path: path.join(output, 'gravity-title-reset.png')});
    check('real save-before-leave reaches Title and disposes quickbar input without unscoped credential writes');
    assert.deepEqual(errors, []);
    const receipt = {passed: true, source: acceptedPackage.source, package: acceptedPackage,
      credentialBootstrap: 'actual separate local API account, scoped token; form acceptance is separate',
      checks, errors, requests, expectedNotFound, cancelledMedia, bundles: [...bundles],
      geometry: {picker: pickerGeometry, book: bookGeometry, quickbar: quickbarGeometry},
      actions: state.actions, casts: state.casts, keys: await keySamples(page),
      effects: state.effects, statuses: state.statuses, accepted: snapshot(),
      lua: state.luaResults.map(({request_id, ok, error}) => ({request_id, ok, error}))};
    fs.writeFileSync(path.join(output, 'mod-skills.json'), JSON.stringify(receipt, null, 2));
    console.log(JSON.stringify({passed: true, checks, errors}));
  } catch (error) {
    await page.screenshot({path: path.join(output, 'mod-skills-failure.png')});
    const ui = await page.evaluate(() => ({menu: document.querySelector('.game-menus')?.dataset.menuScreen,
      status: document.querySelector('#status')?.textContent, text: document.body.innerText.slice(-3500)}));
    fs.writeFileSync(path.join(output, 'mod-skills-failure.json'), JSON.stringify({passed: false,
      step, error: error.stack, checks, errors, requests, expectedNotFound, cancelledMedia,
      bundles: [...bundles], accepted: snapshot(), actions: state.actions, casts: state.casts,
      lua: state.luaResults.map(({request_id, ok, values, error}) => ({request_id, ok, values, error})),
      keys: await keySamples(page), ui}, null, 2));
    throw error;
  } finally {
    let timeout;
    try {
      await Promise.race([server.close(), new Promise((resolve, reject) => {
        timeout = setTimeout(() => reject(new Error('Chrome close timed out')), 15000);
      })]);
    } catch {
      await Promise.race([server.kill(), pause(5000)]);
    } finally {clearTimeout(timeout);}
  }
})().catch(error => {console.error(error.message); process.exitCode = 1;});
