// Real supported Console grants/spawns, ordinary primary input and the normal
// Metal-rendered client. WS observation never sends or installs gameplay state.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const {startNewGame} = require('./menu-journey.cjs');
const ui = require('./gpu-ui.cjs');
const {createNativeMessageReader} = require('./native-wire.cjs');
const {walkSolomonEntrance} = require('./solomon-entrance.cjs');

const output = path.resolve(process.env.OUT);
const qualityOnly = process.env.QUALITY_ONLY === '1';
const account = JSON.parse(fs.readFileSync(process.env.ACCOUNTS, 'utf8')).find(row => row.user.id === 3);
assert.ok(account, 'the isolated configured developer account is required');
const url = new URL(process.env.URL || 'https://127.0.0.1:9449/game-stg/');
assert.ok(['127.0.0.1','localhost'].includes(url.hostname),
  'the developer Console journey requires the owned isolated local host');
const state = {frame: null, id: null, developer: false, shot: null, samples: [], lua: [],
  luaRequests: [], qualityRequests: [], sentInputs: [], frames: [], stormSamples: [], leaveReceipts: 0};
const errors = [], checks = [], requests = [], expectedNotFound = [], images = [];
const qualityImages = [];
const inputBoundaries = [];
const wasm = new Set();
let step = 'bootstrap';
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
const participant = () => state.frame?.snapshot.participants[state.id];
const electric = () => (state.frame?.snapshot.secondary.actors || []).filter(actor =>
  actor.owner_id === state.id && ['ElectricBurnArc', 'ElectricBurnFlare'].includes(actor.kind));
function authority() {
  const row = participant();
  return {tick: state.frame?.snapshot.ticks, runId: state.frame?.snapshot.boneyard?.run_id,
    phase: state.frame?.snapshot.run.phase, ack: state.frame?.input_acknowledgements[state.id],
    holds: {loadingRunId: state.frame?.host.loading_run_id, grace: state.frame?.host.resume_grace,
      pause: state.frame?.host.pause, barrier: state.frame?.snapshot.level_up_barrier},
    pendingOffer: row?.player.progression.pending_offer?.sequence, life: row?.player.combat.life_state,
    health: row?.player.combat.current_health, mana: row?.player.combat.current_mana,
    maximumMana: row?.player.combat.maximum_mana,
    stormRanks: row && [row.player.book.effective_ranks[27], row.player.book.effective_ranks[28]],
    stormSlots: row?.player.belt.entries.flatMap((entry, index) =>
      entry?.Skill?.skill_id === 27 ? [index] : []),
    secondary: row && state.frame.snapshot.secondary.players[state.id],
    storms: (state.frame?.snapshot.secondary.actors || []).filter(actor =>
      actor.owner_id === state.id && actor.kind === 'StormCloud').map(actor =>
      ({id: actor.id, variant: actor.variant, ageTicks: actor.age_ticks})),
    primary: row?.player.book.primary_skill_id, weld: row?.player.book.weld_build_id,
    primaryCast: row?.wizard.primary_cast, position: row?.wizard.position,
    encounter: state.frame?.snapshot.boneyard?.encounter && {
      phase: state.frame.snapshot.boneyard.encounter.phase,
      runEventId: state.frame.snapshot.boneyard.encounter.run_event_id,
      position: state.frame.snapshot.boneyard.encounter.position},
    projectiles: state.frame?.snapshot.spells.projectiles.map(value => Object.keys(value)[0])};
}
async function observeInput(page, label) {
  const browser = {gpu:await ui.readUi(page),...await page.evaluate(() => ({
    input:Array.from(window.wasmBindings.gameplay_input_state()),
    wizard:Array.from(window.wasmBindings.wizard_state()),
    active:document.activeElement?.tagName,
    canvas:document.elementFromPoint(1120,360)?.id,
  }))};
  inputBoundaries.push({label, authority: authority(), browser});
  fs.writeFileSync(path.join(output, 'input-boundaries.json'), JSON.stringify(inputBoundaries, null, 2));
  return browser;
}
async function clientReady(page) {
  await ui.surface(page,'hidden');
  await ui.noWidget(page,'match-loading:label');
  await ui.noWidget(page,'cheats/dialog');
  await page.mouse.move(1120,360);
  await ui.nextFrame(page);
  await ui.waitForUi(page,snapshot=>!snapshot.capture.pointer&&!snapshot.capture.text&&!snapshot.inputScope
    &&!snapshot.widgets.some(widget=>widget.id.startsWith('panel:surface:')),
    'actual gameplay input ownership');
}
async function until(predicate, label, timeout = 30000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    if (await predicate()) return;
    if (errors.length) throw new Error(`${label}: ${errors[0]}`);
    await pause(20);
  }
  throw new Error(`Timed out waiting for ${label}`);
}
function message(row) {
  if (row.type === 'welcome') {
    state.id = row.credentials.player_id;
    state.frame = row.frame;
    state.developer = row.hosted?.developer_access === true;
  } else if (row.type === 'snapshot') state.frame = row.frame;
  else if (row.type === 'error') errors.push(`host ${row.code}: ${row.reason}`);
  else if (row.type === 'hosted') {
    if (row.message.kind === 'lua_result') state.lua.push(row.message);
    if (row.message.kind === 'save_before_leave') state.leaveReceipts++;
  }
  if (row.type === 'welcome' || row.type === 'snapshot') {
    state.frames.push(authority());
    if (state.frames.length > 500) state.frames.shift();
    for (const actor of state.frame.snapshot.secondary.actors) {
      if (actor.owner_id === state.id && actor.kind === 'StormCloud' && state.stormSamples.length < 100)
        state.stormSamples.push({tick: state.frame.snapshot.ticks, actor: structuredClone(actor)});
    }
  }
  for (const value of state.frame?.snapshot.spells.projectiles || []) {
    const shot = value.Weld;
    if (shot?.build_id === 1002 && shot.common.owner_id === state.id && !state.shot)
      state.shot = {tick: state.frame.snapshot.ticks, direction: shot.common.direction,
        origin: shot.common.position};
  }
  const actors = electric();
  if (actors.length && state.samples.length < 150) state.samples.push({
    tick: state.frame.snapshot.ticks, actors: structuredClone(actors),
  });
}
async function consoleRun(page, code) {
  await page.keyboard.press('Backquote');
  await ui.click(page,'cheats/tab:console');
  await ui.fill(page,'cheats/lua-editor',code);
  const before = state.luaRequests.length;
  await ui.click(page,'cheats/run');
  await until(() => state.luaRequests.length > before, 'actual Console request');
  const request = state.luaRequests.at(-1);
  await until(() => state.lua.some(row => row.request_id === request.requestId), 'actual Lua completion');
  const result = state.lua.find(row => row.request_id === request.requestId);
  assert.equal(result.ok, true, result.error || 'real Lua execution failed');
  await page.keyboard.press('Backquote');
  await ui.noWidget(page,'cheats/dialog');
  return result;
}
async function gpuStatus(page) {
  return page.locator('#game').evaluate(canvas => {
    const gl = canvas.getContext('webgl2');
    const debug = gl.getExtension('WEBGL_debug_renderer_info');
    return {error: gl.getError(), lost: gl.isContextLost(), width: canvas.width, height: canvas.height,
      renderer: gl.getParameter(debug ? debug.UNMASKED_RENDERER_WEBGL : gl.RENDERER)};
  });
}
async function setEnhancedEffects(page, enabled) {
  await page.keyboard.press('Escape');
  await ui.click(page,'pause-settings');
  await ui.click(page,'settings-page:performance');
  const toggle = await ui.widget(page,'settings-toggle:enhanced-effects');
  assert.equal(toggle.enabled,true,'the actual run owner can change its setting');
  assert.equal(toggle.selected,!enabled);
  await ui.click(page,toggle.id);
  await until(() => state.frame.snapshot.enhanced_effects === enabled, 'accepted Enhanced Effects change');
  assert.equal((await ui.widget(page,toggle.id)).selected,enabled);
  await ui.click(page,'settings-back');
  await ui.click(page,'settings-back');
  await ui.click(page,'resume');
  await until(() => !state.frame.host.pause && !state.frame.host.resume_grace, 'normal resumed draw after setting change');
  await clientReady(page);
}

(async () => {
  fs.mkdirSync(output, {recursive: true});
  const server = await chromium.launchServer({headless: true, host: '127.0.0.1',
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
  const browser = await chromium.connect(server.wsEndpoint());
  const context = await browser.newContext({ignoreHTTPSErrors: true, viewport: {width: 1600, height: 900}});
  const mount=url.pathname.replace(/\/$/,'');
  await context.addInitScript(({key,token})=>localStorage.setItem(key,token),
    {key:mount?`sdr.token:${mount}`:'sdr.token',token:account.token});
  const page = await context.newPage();
  page.setDefaultTimeout(30000);
  ui.watchRequestFailures(page,{errors});
  await ui.observeGpu(page);
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', entry => {
    if (entry.type() !== 'error') return;
    const pathname = new URL(entry.location().url || url).pathname;
    if (entry.text()==='Failed to load resource: the server responded with a status of 404 (Not Found)'
      && /\/api\/game\/(saves\/0|rejoin)$/.test(pathname)) {
      expectedNotFound.push({type:'console',url:entry.location().url,message:entry.text(),
        ownership:'A fresh configured account has no checkpoint or rejoin ticket; only these exact discovery404s are expected.'});
      return;
    }
    errors.push(entry.text());
  });
  page.on('response', response => {
    const pathname = new URL(response.url()).pathname;
    if (pathname.endsWith('.wasm')) wasm.add(path.basename(pathname));
    if (pathname.includes('/api/')) requests.push({path: pathname, status: response.status()});
    if (response.status() === 404 && /\/api\/game\/(saves\/0|rejoin)$/.test(pathname)) expectedNotFound.push({type:'response',url:response.url(),status:404,
      ownership:'A fresh configured account has no checkpoint or rejoin ticket; only these exact discovery404s are expected.'});
    else if (response.status() >= 400) errors.push(`${response.status()} ${pathname}`);
  });
  page.on('websocket', socket => {
    const read = createNativeMessageReader();
    socket.on('framereceived', ({payload}) => {
      try {const row = read(JSON.parse(payload.toString())); if (row) message(row);}
      catch (error) {errors.push(`observed WS: ${error.message}`);}
    });
    socket.on('framesent', ({payload}) => {
      const row = JSON.parse(payload.toString());
      if (row.type === 'hosted' && row.command.kind === 'lua_execute')
        state.luaRequests.push({requestId: row.command.request_id, code: row.command.code});
      if (row.type === 'hosted' && row.command.kind === 'set_enhanced_effects')
        state.qualityRequests.push({enabled: row.command.enabled, tick: state.frame?.snapshot.ticks});
      if (row.type === 'input') {
        state.sentInputs.push({sequence: row.sequence, targetTick: row.target_tick,
          input: row.input, authority: authority()});
        if (state.sentInputs.length > 150) state.sentInputs.shift();
      }
    });
  });
  try {
    step = 'ordinary Create and real developer admission';
    await page.goto(url.href, {waitUntil: 'load'});
    await ui.ready(page);
    if (process.env.RESUME === '1') {
      if(ui.findWidget(await ui.readUi(page),'prompt-secondary')) {
        await ui.click(page,'prompt-secondary');
        await ui.noWidget(page,'prompt-secondary');
      }
      await ui.click(page,'play');
      await ui.click(page,'continue');
    } else await startNewGame(page, {element: 'air', discipline: 'arcane', name: 'Electric'});
    await until(() => participant() && state.developer, 'accepted developer College');
    if (process.env.RESUME !== '1') assert.equal(participant().player.selection.name, 'Electric');
    checks.push(process.env.RESUME === '1' ? 'ordinary Continue retains the actual QA wizard'
      : 'real developer account and ordinary accepted Air/Arcane Create');

    step = 'supported Console grant and ordinary Boneyard entry';
    if (!qualityOnly) {
    const grants = [];
    const chaining = participant().player.book.permanent_ranks[25];
    if (chaining < 2) grants.push(`sd.dev.grant_skill(25, ${2 - chaining})`);
    if (participant().player.book.weld_build_id !== 1002 || participant().player.book.primary_skill_id !== 52)
      grants.push('sd.dev.grant_weld(1002)');
    if (grants.length) await consoleRun(page, `return ${grants.join(', ')}`);
    await until(() => participant().player.book.weld_build_id === 1002
      && participant().player.book.primary_skill_id === 52, 'actual BallLightning selection');
    }
    if (!state.frame.snapshot.boneyard) await ui.click(page,'hud:map');
    await until(() => state.frame?.snapshot.boneyard && !state.frame.host.loading_run_id
      && !state.frame.host.resume_grace && !state.frame.host.pause, 'actual Boneyard readiness', 120000);
    await observeInput(page, 'server holds cleared before client admission');
    await clientReady(page);
    step = 'ordinary Solomon entrance and combat admission';
    if (state.frame.snapshot.boneyard.encounter?.run_event_id === 0) {
      const response = await context.request.get(new URL('api/game/saves/0', url).href,
        {headers: {Authorization: `Bearer ${account.token}`}});
      assert.equal(response.status(), 200, 'the actual resumed Source document is available');
      const saved = await response.json();
      await walkSolomonEntrance(page, {document: saved.save.document,
        snapshot: () => state.frame.snapshot, playerId: () => state.id, output});
    }
    checks.push('ordinary keyboard movement completes the real Solomon entrance before combat');
    await clientReady(page);
    let gpu = null;
    if (!qualityOnly) {
    step = 'ordinary primary cast after actual combat admission';
    await observeInput(page, 'actual client input admitted before primary press');
    await page.mouse.move(1120, 360);
    await page.mouse.down();
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    await observeInput(page, 'ordinary primary held');
    await until(() => state.shot, 'actual normal-input BallLightning projectile');
    await page.mouse.up();
    checks.push('supported grant produces a real BallLightning projectile through ordinary held input');

    step = 'supported nearby target spawn';
    const d = state.shot.direction;
    assert.ok(Number.isFinite(d.x) && Number.isFinite(d.y));
    await consoleRun(page, `local p=sd.player.get_state()
local dx,dy=${d.x},${d.y}
for _,offset in ipairs({{120,0},{140,-35},{140,35},{175,0}}) do
 sd.enemies.spawn('zombie',{x=p.x+dx*offset[1]-dy*offset[2],y=p.y+dy*offset[1]+dx*offset[2]})
end
return true`);
    await page.screenshot({path: path.join(output, 'before-electric.png')});
    await clientReady(page);
    await page.mouse.move(1120, 360);
    await page.mouse.down();
    await until(() => state.samples.some(sample => sample.actors.some(actor => actor.kind === 'ElectricBurnArc')),
      'actual ElectricBurn arc birth');
    await until(() => state.samples.some(sample => sample.actors.some(actor => actor.kind === 'ElectricBurnFlare')),
      'actual ElectricBurn flare birth');
    for (let index = 0; index < 6; index++) {
      const tick = state.frame.snapshot.ticks;
      const actors = structuredClone(electric());
      const file = `electric-${index}.png`;
      await page.screenshot({path: path.join(output, file)});
      images.push({file, acceptedTick: tick, actors});
      await pause(35);
    }
    await page.mouse.up();
    gpu = await gpuStatus(page);
    assert.equal(gpu.error, 0);
    assert.equal(gpu.lost, false);
    assert.ok(images.some(row => row.actors.some(actor => actor.kind === 'ElectricBurnArc')));
    checks.push('accepted private Electric births pass the real Metal canvas with no WebGL error/context loss');
    fs.writeFileSync(path.join(output, 'electric-burn-accepted.json'), JSON.stringify({
      passed: true, checks: [...checks], errors: [...errors], gpu, images,
      shot: state.shot, samples: state.samples,
      scope: 'Actual ElectricBurn Arc/Flare GPU acceptance before the separate retained-quality check.'}, null, 2));
    }

    step = 'retained stock Storm live-quality toggle';
    const stormGrants = [];
    if (participant().player.book.effective_ranks[27] === 0) stormGrants.push('sd.dev.grant_skill(27, 1)');
    if (participant().player.book.effective_ranks[28] === 0) stormGrants.push('sd.dev.grant_skill(28, 1)');
    await consoleRun(page, `return ${[...stormGrants, 'sd.player.restore_health(100000)',
      'sd.player.restore_mana()'].join(', ')}`);
    await until(() => participant().player.book.effective_ranks[28] > 0
      && participant().player.combat.current_mana >= 90
      && participant().player.belt.entries.some(entry => entry?.Skill?.skill_id === 27),
      'actual Tornado grant, restored mana and Storm belt binding');
    const stormSlot = participant().player.belt.entries.findIndex(entry => entry?.Skill?.skill_id === 27);
    assert.ok(stormSlot >= 0 && stormSlot < 8, 'the real auto-bound Storm is in the visible hotbar');
    await clientReady(page);
    await observeInput(page, 'actual Storm rank, resources and admission before cast');
    // Source's first belt binding is right mouse; the remaining seven are 1..7.
    // Slot+1 incorrectly activates a different skill, even though that input ACKs.
    if (stormSlot === 0) {
      await page.mouse.move(1120, 360);
      await page.mouse.down({button: 'right'});
      await page.waitForTimeout(100);
      await page.mouse.up({button: 'right'});
    } else await page.keyboard.press(`Digit${stormSlot}`, {delay: 100});
    const storm = () => state.frame.snapshot.secondary.actors.find(actor =>
      actor.kind === 'StormCloud' && actor.owner_id === state.id);
    await until(() => storm()?.variant === 1 && storm().age_ticks >= 20, 'actual moving Storm cast', 10000);
    await observeInput(page, 'actual accepted Storm birth');
    const stormId = storm().id;
    const birthEnhanced = storm().enhanced;
    assert.equal(state.frame.snapshot.enhanced_effects, true);
    for (const enabled of [true, false, true]) {
      if (state.frame.snapshot.enhanced_effects !== enabled) await setEnhancedEffects(page, enabled);
      assert.equal(storm()?.id, stormId, 'the same stock Storm remains alive across the setting change');
      assert.equal(storm().enhanced, birthEnhanced, 'the retained birth setting is unchanged');
      const row = {enabled, acceptedTick: state.frame.snapshot.ticks, actor: structuredClone(storm()),
        file: `storm-quality-${qualityImages.length}-${enabled}.png`, gpu: await gpuStatus(page)};
      await page.screenshot({path: path.join(output, row.file)});
      assert.equal(row.gpu.error, 0);
      assert.equal(row.gpu.lost, false);
      qualityImages.push(row);
    }
    assert.deepEqual(state.qualityRequests.map(row => row.enabled), [false, true]);
    checks.push('ordinary Settings changes live quality off/on for the same retained Storm without replacing its birth state');

    step = 'normal saved Leave and disposal';
    await page.keyboard.press('Escape');
    await ui.click(page,'leave');
    await ui.surface(page,'title');
    assert.ok(state.leaveReceipts > 0);
    await ui.assertGpuOnly(page);
    await page.screenshot({path:path.join(output,'electric-final-title.png')});
    await ui.settleErrors(page);
    assert.deepEqual(errors, []);
    checks.push('ordinary saved Leave returns to Title');
    fs.writeFileSync(path.join(output, qualityOnly ? 'storm-quality.json' : 'electric-burn.json'), JSON.stringify({passed: true,
      sourceRevision: 'fa08ae249ff5563570bdb2a653e2ef9358ce7b83', checks, errors, requests,
      expectedNotFound, wasm: [...wasm], gpu, images, qualityImages,
      qualityRequests: state.qualityRequests, shot: state.shot, samples: state.samples,
      stormSamples: state.stormSamples,
      sentInputs: state.sentInputs, inputBoundaries, frames: state.frames,
      commands: state.luaRequests, leaveReceipts: state.leaveReceipts,
      scope: 'Real normal client GPU acceptance; exact Source body/painter plans are independently replayed. No screenshot pixel-equivalence claim.'}, null, 2));
    console.log(JSON.stringify({passed: true, checks, images: images.length, gpu, errors}));
  } catch (error) {
    await observeInput(page, 'failure before releasing the held input').catch(() => {});
    await page.mouse.up().catch(() => {});
    await page.screenshot({path: path.join(output, 'failure.png')}).catch(() => {});
    fs.writeFileSync(path.join(output, 'failure.json'), JSON.stringify({passed: false, step,
      error: error.stack, errors, requests, expectedNotFound, wasm: [...wasm],
      images, qualityImages, qualityRequests: state.qualityRequests,
      stormSamples: state.stormSamples,
      shot: state.shot, samples: state.samples, commands: state.luaRequests,
      sentInputs: state.sentInputs, inputBoundaries, frames: state.frames,
      ui: await ui.readUi(page).catch(() => null)}, null, 2));
    throw error;
  } finally {
    await context.close();
    await browser.close();
    await server.close();
  }
})().catch(error => {console.error(error.message); process.exitCode = 1;});
