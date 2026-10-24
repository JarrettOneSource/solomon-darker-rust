// Genuine combat/loot/picker/Dowse journey, adapted from the retained earned-run
// driver. Source navigation plans real keys only; no game state is installed.
// Run against an owned native host with its matched client assets. Set PW, URL,
// OUT and SOLOMON_WEBSITE_DIR; Node needs --experimental-strip-types for Source.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {pathToFileURL} = require('node:url');
const {chromium} = require(path.resolve(process.env.PW));
const {startNewGame} = require('./menu-journey.cjs');
const {createNativeMessageReader} = require('./native-wire.cjs');
const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
const ui = require('./gpu-ui.cjs');

const output = path.resolve(process.env.OUT || 'earned-offer');
const url = process.env.URL || 'http://127.0.0.1:4190/?host=1';
const source = path.resolve(process.env.SOLOMON_WEBSITE_DIR, 'frontend/src/game');
const receipt = {passed:false, artifact:process.env.ARTIFACT, checks:[], runs:[],
  commands:[], results:[], errors:[], qualification:'ordinary earned inventory; no imported save or seed'};
const held = new Set();
let page, observed, frame, playerId, firing = false;
let lastPrimaryPulse = 0, lastSecondaryTick = -1000;
const participant = () => frame?.snapshot.participants[playerId];
const player = () => participant()?.player;
const world = () => frame?.snapshot.boneyard;
const delay = milliseconds => page.waitForTimeout(milliseconds);
const importSource = relative => import(pathToFileURL(path.join(source, relative)).href);

async function until(predicate, label, timeout = 30000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    assert.deepEqual(observed.errors, []);
    assert.deepEqual(receipt.errors, []);
    if (await predicate()) return;
    await delay(50);
  }
  throw new Error(`Timed out: ${label}`);
}

async function movement(x, y) {
  const desired = new Set();
  if (x < -0.25) desired.add('KeyA');
  if (x > 0.25) desired.add('KeyD');
  if (y < -0.25) desired.add('KeyW');
  if (y > 0.25) desired.add('KeyS');
  for (const key of held) if (!desired.has(key)) {
    await page.keyboard.up(key); held.delete(key);
  }
  for (const key of desired) if (!held.has(key)) {
    await page.keyboard.down(key); held.add(key);
  }
}

async function fire(enabled) {
  if (firing === enabled) return;
  firing = enabled;
  if (enabled) await page.mouse.down();
  else await page.mouse.up();
}
async function release() { await movement(0, 0); await fire(false); }
async function shot(name) { await page.screenshot({path:path.join(output, `${name}.png`)}); }

function observeAuthority() {
  page.on('websocket', socket => {
    const read = createNativeMessageReader();
    socket.on('framereceived', ({payload}) => {
      try {
        const message = read(JSON.parse(payload.toString()));
        if (!message) return;
        if (message.type === 'welcome') {
          playerId = message.credentials.player_id; frame = message.frame;
        } else if (message.type === 'snapshot') frame = message.frame;
        else if (message.type === 'command_result') receipt.results.push(message);
        else if (message.type === 'error') receipt.errors.push({code:message.code, reason:message.reason});
      } catch (error) { receipt.errors.push({protocol:error.stack}); }
    });
    socket.on('framesent', ({payload}) => {
      const message = JSON.parse(payload.toString());
      if (message.type === 'command') receipt.commands.push({kind:message.command.kind, sequence:message.sequence});
    });
  });
}

async function gameplayReady() {
  await until(() => player() && !frame.host.loading_run_id && !frame.host.resume_grace,
    'the actual native participant is admitted', 120000);
  await ui.waitForUi(page, state => state.surface === 'hidden' && !state.inputScope
    && !state.capture.text && ui.findWidget(state, 'hud:health'), 'genuine gameplay input admission');
}

async function prepareNavigation() {
  const [{NATIVE_GENERATED_BONEYARDS}, collision, navigation, camera] = await Promise.all([
    importSource('host/native-generated-boneyards.ts'),
    importSource('core-server/boneyard-collision.ts'),
    importSource('core-server/boneyard-enemy-navigation.ts'),
    importSource('renderer/boneyard-render-contract.ts'),
  ]);
  const template = NATIVE_GENERATED_BONEYARDS.find(row => row.sourceSha256 === world().source_sha256);
  assert.ok(template, 'the real scene has an unchanged Source template');
  assert.equal(template.geometrySha256, world().geometry_sha256);
  const geometry = collision.createBoneyardCollisionWorld(template.scene);
  const prepared = JSON.parse(fs.readFileSync(path.join(__dirname,
    '../../web/assets/navigation', `${world().source_sha256}.json`), 'utf8'));
  assert.equal(prepared.geometrySha256, world().geometry_sha256);
  for (const row of prepared.meshes) navigation.installPreparedBoneyardNavigationMesh(geometry, {
    key:[row.bounds.x,row.bounds.y,row.bounds.w,row.bounds.h,row.clearance].join(':'), mesh:row.mesh,
  });
  return {collision, navigation, geometry, scene:template.scene, camera};
}

async function approachSolomon(plan, run) {
  const initial = world();
  const start = participant().wizard.position;
  const gates = new Map();
  for (const leaf of initial.gate_leaves) {
    const pair = gates.get(leaf.fence_eid) || [];
    pair.push(leaf.hinge); gates.set(leaf.fence_eid, pair);
  }
  const gate = [...gates.values()].filter(pair => pair.length === 2)
    .map(pair => ({x:(pair[0].x+pair[1].x)/2, y:(pair[0].y+pair[1].y)/2}))
    .sort((a,b) => Math.hypot(a.x-start.x,a.y-start.y)-Math.hypot(b.x-start.x,b.y-start.y))[0];
  assert.ok(gate, 'the ordinary entrance retains its actual gate');
  const direction = initial.arena_transition.entry_side === 'North' ? 1 : -1;
  const targets = [{x:gate.x,y:gate.y-direction*75}, {x:gate.x,y:gate.y+direction*150}];
  let waypoint = 0, target, lastPlanTick = 0;
  const deadline = Date.now() + 270000;
  try {
    while (world().encounter.phase === 'Digging' && Date.now() < deadline) {
      assert.deepEqual(observed.errors, []);
      const at = participant().wizard.position;
      if (waypoint < targets.length) target = targets[waypoint];
      else {
        const end = initial.encounter.position;
        if (Math.hypot(at.x-end.x,at.y-end.y) < 90) { await release(); await delay(50); continue; }
        if (!target || Math.hypot(at.x-target.x,at.y-target.y) < 15
          || frame.snapshot.ticks-lastPlanTick > 100) {
          const route = plan.navigation.findBoneyardEnemyRoute({bodyRadius:25,
            bounds:plan.scene.bounds, clearance:25, start:at, end, endBodyRadius:0,
            world:plan.geometry, ignoredSourceIds:new Set(plan.collision.boneyardBodyCollisionSourceIds(end,plan.geometry,30))});
          assert.ok(route && route.length > 1, 'unchanged Source navigation can reach Solomon');
          target = route.find(point => Math.hypot(point.x-at.x,point.y-at.y) >= 12) || end;
          lastPlanTick = frame.snapshot.ticks;
        }
      }
      const dx = target.x-at.x, dy = target.y-at.y, close = Math.hypot(dx,dy) < 12;
      if (close && waypoint < targets.length) { waypoint++; target = null; }
      await movement(!close && Math.abs(dx)>2 ? Math.sign(dx) : 0,
        !close && Math.abs(dy)>5 ? Math.sign(dy) : 0);
      run.approach.push({tick:frame.snapshot.ticks, position:at, target, waypoint});
      await delay(50);
    }
    assert.notEqual(world().encounter.phase, 'Digging', 'real movement wakes Solomon');
  } finally { await release(); }
}

function screenPoint(position, plan) {
  const bounds = world().arena_transition.camera_bounds;
  const camera = plan.camera.boneyardCamera(participant().wizard.position,
    {x:bounds.x,y:bounds.y,w:bounds.width,h:bounds.height});
  const origin = plan.camera.boneyardWorldPosition(camera);
  return [Math.max(50,Math.min(1450,origin.x+position.x*camera.zoom)),
    Math.max(80,Math.min(750,origin.y+position.y*camera.zoom))];
}

async function chooseOffer(offer) {
  await release();
  const preferences = [64,16,17,18,21,72,73,67,0];
  let index = 0;
  for (const skill of preferences) {
    const available = offer.options.findIndex(option => option.skill_id === skill);
    if (available >= 0) { index = available; break; }
  }
  const option = offer.options[index];
  const before = player().book.permanent_ranks[option.skill_id];
  await ui.waitForUi(page, state => ui.findWidget(state, `picker:choice:${index}`)?.enabled,
    'the genuinely earned picker reveal admits input');
  assert.equal((await ui.widget(page, `picker:choice:${index}`)).value, String(option.skill_id));
  const detail = ui.bounds(await ui.widget(page, `picker:detail:${index}`));
  await page.mouse.move(detail.x+detail.width/2, detail.y+detail.height/2);
  await ui.widget(page, 'picker:tooltip');
  await shot(`earned-picker-${receipt.checks.length}`);
  await ui.click(page, `picker:choice:${index}`);
  await until(() => player().book.permanent_ranks[option.skill_id] > before
    && player().progression.pending_offer?.sequence !== offer.sequence, 'the real earned rank is accepted');
  receipt.checks.push({earnedChoice:option, before, after:player().book.permanent_ranks[option.skill_id]});
}

function available(kind, items = player().economy.backpack) {
  return items.some(item => item.kind === kind && item.quantity > 0
    || item.contents && available(kind, item.contents));
}

async function combat(plan, run) {
  const deadline = Date.now() + 240000;
  let lastSample = 0;
  try {
    while (Date.now() < deadline && frame.snapshot.run.phase !== 'GameOver') {
      assert.deepEqual(observed.errors, []);
      if (player().progression.pending_offer) { await chooseOffer(player().progression.pending_offer); continue; }
      const at = participant().wizard.position;
      const living = world().enemies.actors.filter(actor => actor.life_state === 'Alive' && actor.current_health > 0)
        .sort((a,b) => Math.hypot(a.position.x-at.x,a.position.y-at.y)-Math.hypot(b.position.x-at.x,b.position.y-at.y));
      const enemy = living[0], distance = enemy ? Math.hypot(enemy.position.x-at.x,enemy.position.y-at.y) : Infinity;
      if (player().combat.current_health < 25 && available('HealthPotion')) await page.keyboard.press('Digit3');
      if (player().combat.current_mana < 20 && available('ManaPotion')) await page.keyboard.press('Digit4');
      if (enemy) {
        await page.mouse.move(...screenPoint(enemy.position, plan));
        if (Date.now()-lastPrimaryPulse > 1000) { await fire(false); lastPrimaryPulse = Date.now(); }
        await fire(true);
        if (living.length >= 4 && distance < 300 && player().combat.current_mana >= 40
          && frame.snapshot.ticks-lastSecondaryTick > 500) {
          await page.mouse.down({button:'right'}); await delay(60); await page.mouse.up({button:'right'});
          lastSecondaryTick = frame.snapshot.ticks;
        }
      } else await fire(false);
      const loot = world().loot.actors.filter(actor => ['Gold','Orb','Sack','Bonus'].includes(actor.kind)
        && actor.activation_delay_ticks < 1)
        .sort((a,b) => Math.hypot(a.position.x-at.x,a.position.y-at.y)-Math.hypot(b.position.x-at.x,b.position.y-at.y))[0];
      if (loot && (distance > 105 || !enemy)) {
        const dx = loot.position.x-at.x, dy = loot.position.y-at.y, length = Math.hypot(dx,dy) || 1;
        await movement(dx/length, dy/length);
      } else if (enemy && distance < 160) {
        const dx = at.x-enemy.position.x, dy = at.y-enemy.position.y, length = Math.hypot(dx,dy) || 1;
        const bounds = world().arena_transition.combat_bounds;
        let x = (dx-dy*0.9)/length, y = (dy+dx*0.9)/length;
        if (at.x < bounds.x+120 || at.x > bounds.x+bounds.width-120) x = Math.sign(bounds.x+bounds.width/2-at.x);
        if (at.y < bounds.y+120 || at.y > bounds.y+bounds.height-120) y = Math.sign(bounds.y+bounds.height/2-at.y);
        await movement(x,y);
      } else await movement(0,0);
      if (Date.now()-lastSample > 1000) {
        const sample = {tick:frame.snapshot.ticks, level:player().progression.level,
          gold:player().economy.gold, health:player().combat.current_health, mana:player().combat.current_mana,
          enemies:living.length, position:at};
        run.samples.push(sample); lastSample = Date.now(); console.log(JSON.stringify(sample));
        fs.writeFileSync(path.join(output,'progress.json'), JSON.stringify(receipt,null,2));
      }
      if (player().economy.gold >= 650 && receipt.checks.some(check => check.earnedChoice)) break;
      await delay(100);
    }
  } finally { await release(); }
}

(async () => {
  fs.mkdirSync(output, {recursive:true});
  const owned = await launchOwnedBrowser(chromium, {headless:true,
    executablePath:process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=metal','--ignore-gpu-blocklist']});
  let failure;
  try {
    page = await owned.browser.newPage({viewport:{width:1600,height:900}, ignoreHTTPSErrors:process.env.LOCAL_TLS === '1'});
    observed = ui.watchErrors(page); await observed.ready; observeAuthority();
    await page.goto(url); await ui.ready(page);
    await startNewGame(page, {name:'Aureus',element:'fire',discipline:'arcane'}); await gameplayReady();
    for (let attempt = 0; attempt < 3; attempt++) {
      lastPrimaryPulse = 0; lastSecondaryTick = -1000;
      await ui.click(page, 'hud:map');
      await until(() => world() && !frame.host.loading_run_id && !frame.host.resume_grace && !frame.host.pause,
        'ordinary Boneyard admission', 120000);
      await gameplayReady(); assert.equal(world().tutorial, null);
      const run = {runId:world().run_id, sourceSha256:world().source_sha256,
        geometrySha256:world().geometry_sha256, seed:world().seed, approach:[], samples:[]};
      receipt.runs.push(run);
      const plan = await prepareNavigation(); await approachSolomon(plan,run); await combat(plan,run);
      await shot(`earned-run-${attempt}`);
      await until(async () => {
        if (player().progression.pending_offer) await chooseOffer(player().progression.pending_offer);
        return frame.snapshot.run.phase === 'GameOver';
      }, 'normal enemy-caused Game Over', 180000);
      await ui.click(page, 'game-over:continue'); await ui.surface(page, 'create');
      await ui.click(page, 'element:fire'); await ui.click(page, 'discipline:arcane');
      await until(() => !world() && frame.snapshot.run.phase === 'Hub', 'normal post-run College return', 120000);
      await gameplayReady();
      if (player().economy.gold >= 650 && receipt.checks.some(check => check.earnedChoice)) break;
    }
    assert.ok(receipt.checks.some(check => check.earnedChoice), 'combat actually earns and accepts a skill offer');
    assert.ok(player().economy.gold >= 650, 'actual pickups pay the Source650gold fee');
    const shlorio = await ui.byLabel(page, 'Shlorio', {role:'button',prefix:'hud:shortcut:'});
    await ui.click(page, shlorio.id); await ui.widget(page, 'panel:service');
    const before = player().economy.gold, fee = player().economy.dowsing_fee;
    await ui.click(page, 'panel:dowse');
    await until(() => player().economy.dowsing_rolled, 'the genuinely earned paid Dowse is accepted');
    const offers = player().economy.dowsing_offers;
    assert.ok(offers.length === 3 || offers.length === 4, 'Source no-reference Dowse offers three or four items');
    assert.equal(player().economy.gold, before-fee);
    await ui.waitForUi(page, state => state.widgets.filter(widget => widget.id.startsWith('panel:shop:')).length === offers.length,
      'all accepted paid offers are actually painted');
    await ui.value(page, 'panel:gold', String(before-fee)); await shot('paid-earned-offers');
    receipt.checks.push({paidDowse:{before,fee,after:player().economy.gold,offers}});
    await ui.click(page, 'panel:close'); await ui.noWidget(page, 'panel:service');
    await until(() => !player().economy.dowsing_rolled, 'real Done retires the rolled lock');
    await page.keyboard.press('Escape'); await ui.surface(page, 'pause');
    await ui.click(page, 'leave'); await ui.surface(page, 'title');
    await ui.assertGpuOnly(page); await ui.settleErrors(page);
    assert.deepEqual(observed.errors, []); assert.deepEqual(receipt.errors, []); receipt.passed = true;
  } catch (error) {
    failure = error; receipt.failure = error.stack;
    if (page) {
      await release().catch(() => {});
      receipt.failedPlayer = player(); receipt.failedWorld = world();
      receipt.failedUi = await ui.readUi(page).catch(error => ({error:error.message}));
      await shot('failure').catch(() => {});
    }
  } finally {
    if (page) try {
      await ui.settleErrors(page); assert.deepEqual(observed.errors, []); assert.deepEqual(receipt.errors, []);
    }
    catch (error) { failure ||= error; receipt.passed = false; receipt.observerFailure = error.stack; }
    receipt.browserErrors = observed?.errors || [];
    try { receipt.cleanup = await owned.close(); }
    catch (error) { failure ||= error; receipt.passed = false; receipt.cleanup = error.cleanup; }
    fs.writeFileSync(path.join(output,'receipt.json'), JSON.stringify(receipt,null,2));
  }
  console.log(JSON.stringify({passed:receipt.passed, runs:receipt.runs.length, checks:receipt.checks, cleanup:receipt.cleanup}));
  if (failure) throw failure;
})().catch(error => { console.error(error); process.exitCode = 1; });
