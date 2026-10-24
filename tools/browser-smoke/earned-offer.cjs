// Natural combat/loot/picker/first Dowse, followed by separately qualified private
// Cheats UI funding for expensive reference/purchase. No save/seed is installed;
// Source navigation plans real keys and every game action comes from visible UI.
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
const {prepareBoneyardNavigation, walkSolomonEntrance} = require('./solomon-entrance.cjs');
const {assertWorldNotification} = require('./world-notifications.cjs');
const ui = require('./gpu-ui.cjs');

const output = path.resolve(process.env.OUT || 'earned-offer');
const url = process.env.URL || 'http://127.0.0.1:4190/?host=1';
const receipt = {passed:false, artifact:process.env.ARTIFACT, checks:[], runs:[],
  commands:[], results:[], errors:[], naturalGoldEvents:[],
  qualification:'natural earned rank/Gold/first roll; separate controlled private-UI funding for expensive reference/purchase'};
const held = new Set();
const seenGold = new Set();
let page, observed, frame, playerId, firing = false;
let naturalPhase = true, goldAnnouncementStart = 0;
let lastPrimaryPulse = 0, lastSecondaryTick = -1000;
const participant = () => frame?.snapshot.participants[playerId];
const player = () => participant()?.player;
const world = () => frame?.snapshot.boneyard;
const delay = milliseconds => page.waitForTimeout(milliseconds);

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
        if (naturalPhase) for (const event of world()?.loot_events || []) {
          const key = `${world().run_id}:${event.event_id}`;
          if (!seenGold.has(key) && event.event_type === 'LootPickup' && event.player_id === playerId
            && event.sound === 'PickupCoin' && /^\d+ GOLD$/.test(event.text || '')) {
            seenGold.add(key);
            receipt.naturalGoldEvents.push({...event, runId:world().run_id});
          }
        }
      } catch (error) { receipt.errors.push({protocol:error.stack}); }
    });
    socket.on('framesent', ({payload}) => {
      const message = JSON.parse(payload.toString());
      if (message.type === 'command') receipt.commands.push({kind:message.command.kind,
        action:message.command.action, sequence:message.sequence, phase:naturalPhase?'natural':'controlled'});
      if (message.type === 'hosted') receipt.commands.push({kind:`hosted:${message.command.kind}`,
        requestId:message.command.request_id, phase:naturalPhase?'natural':'controlled'});
    });
  });
}

async function captureNaturalGoldNotice() {
  if (receipt.goldNotification || !receipt.naturalGoldEvents.length
    || player().combat.life_state !== 'Alive') return;
  const current = await ui.readUi(page);
  if (current.surface !== 'hidden' || current.inputScope || player().progression.pending_offer) return;
  const visible = current.widgets.find(widget => widget.id.startsWith('world-notification:loot:')
    && /^\d+ GOLD$/.test(widget.label) && JSON.parse(widget.value).offset >= 0
    && JSON.parse(widget.value).alpha > 0);
  if (!visible) return;
  await release();
  await page.keyboard.press('Escape'); await ui.surface(page, 'pause');
  await until(() => frame.host.pause, 'ordinary pause retains the actually earned Gold notice');
  const snapshot = await ui.readUi(page);
  const notice = ui.findWidget(snapshot, visible.id);
  assert.ok(notice, 'the genuinely paused world retains its Gold notice');
  const presentation = JSON.parse(notice.value);
  const firstId = Number(presentation.key.slice('loot:'.length));
  const amount = Number(notice.label.split(' ')[0]);
  let sum = 0;
  const contributing = [];
  for (const event of receipt.naturalGoldEvents.filter(event => event.runId === world().run_id
    && event.event_id >= firstId).sort((a,b) => a.event_id-b.event_id)) {
    contributing.push(event); sum += Number(event.text.split(' ')[0]);
    if (sum >= amount) break;
  }
  assert.equal(sum, amount, 'coalesced Gold text is owned by the actual local pickup events');
  assert.equal(contributing[0].event_id, firstId, 'Gold keeps the first active native event key');
  receipt.goldNotification = {...await assertWorldNotification(page, {
    key:presentation.key, text:notice.label, afterAnnouncement:goldAnnouncementStart,
  }), contributing};
  await shot('naturally-earned-gold-notification');
  await ui.click(page, 'resume'); await gameplayReady();
}

async function gameplayReady() {
  await until(() => player() && !frame.host.loading_run_id && !frame.host.resume_grace,
    'the actual native participant is admitted', 120000);
  await ui.waitForUi(page, state => state.surface === 'hidden' && !state.inputScope
    && !state.capture.text && ui.findWidget(state, 'hud:health'), 'genuine gameplay input admission');
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
      await captureNaturalGoldNotice();
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

async function fundThroughPrivateControls() {
  naturalPhase = false;
  receipt.naturalPhase = {endTick:frame.snapshot.ticks, gold:player().economy.gold,
    earnedChoices:receipt.checks.filter(check => check.earnedChoice), firstRoll:receipt.checks.at(-1)};
  await page.keyboard.press('Escape'); await ui.surface(page, 'pause');
  await ui.click(page, 'pause-settings'); await ui.click(page, 'settings-page:cloud');
  if (!(await ui.widget(page, 'settings-toggle:cheats')).selected) await ui.click(page, 'settings-toggle:cheats');
  const result = await ui.waitForUi(page, state => ui.findWidget(state, 'routing/dialog')
    || ui.findWidget(state, 'settings-toggle:cheats')?.selected, 'actual private-cheat preference/consent');
  if (ui.findWidget(result, 'routing/dialog')) {
    await ui.click(page, 'routing-continue'); await ui.surface(page, 'title');
    await ui.click(page, 'play'); await ui.click(page, 'continue');
  } else {
    await ui.click(page, 'settings-back'); await ui.click(page, 'settings-back');
    await ui.surface(page, 'pause'); await ui.click(page, 'resume');
  }
  await gameplayReady();
  await page.keyboard.press('Backquote'); await ui.widget(page, 'cheats/dialog');
  const before = player().economy.gold;
  await ui.fill(page, 'cheats/gold', '10000'); await ui.click(page, 'cheats/submit:gold');
  await until(() => player().economy.gold === 10000, 'the supported private Gold control is accepted');
  await ui.widget(page, 'cheats/submit:gold', {enabled:true});
  const action = await ui.widget(page, 'cheats/action-receipt');
  assert.equal(action.role,'status'); assert.ok(action.label.includes('Gold set')&&!action.label.endsWith('…'));
  receipt.funding = {qualification:'controlled private-UI funding; not naturally earned',
    before, after:player().economy.gold, action};
  await shot('controlled-private-gold-prerequisite');
  await ui.click(page, 'cheats/close'); await ui.noWidget(page, 'cheats/dialog');
  await gameplayReady();
}

async function referenceAndPurchase() {
  const {DOWSING_EQUIPMENT_RECIPES:recipes} = await import(pathToFileURL(path.resolve(
    process.env.SOLOMON_WEBSITE_DIR, 'frontend/src/game/core-kernels/hub-economy.ts')).href);
  const shlorio = await ui.byLabel(page, 'Shlorio', {role:'button',prefix:'hud:shortcut:'});
  await ui.click(page, shlorio.id);
  const panel = ui.bounds(await ui.widget(page, 'panel:service'));
  const reference = player().economy.equipment.hat;
  assert.ok(reference, 'the actual owned Hat supplies a no-set reference');
  assert.equal(reference.recipe_index, null);
  const from = ui.center(ui.bounds(await ui.widget(page, 'panel:equipment:hat')));
  // The painted service rectangle projects the Source mirror's native center.
  const to = {x:panel.x+panel.width*302/604, y:panel.y+panel.height*175.5/420};
  await page.mouse.move(from.x,from.y); await page.mouse.down();
  await page.mouse.move(to.x,to.y,{steps:15}); await page.mouse.up(); await ui.nextFrame(page);
  await shot('actual-owned-hat-reference');
  const before = player().economy.gold, fee = player().economy.dowsing_fee;
  assert.equal(fee,650,'Done without purchase retains the first650gold fee');
  const commandsBefore = receipt.commands.length;
  await ui.click(page,'panel:dowse'); await until(() => player().economy.dowsing_rolled, 'actual reference roll');
  const request = receipt.commands.slice(commandsBefore).find(command => command.action?.Dowse);
  assert.equal(request?.action.Dowse.reference_item_id,reference.id,'real drag supplies the owned reference to the typed command');
  const offers = player().economy.dowsing_offers;
  assert.ok(offers.length===3||offers.length===4,'a no-set Hat skips the two optional matching-set slots');
  assert.ok(offers.every(offer => recipes[offer.recipe_index].nativeTypeId===reference.native_type_id));
  assert.equal(player().economy.gold,before-fee);
  assert.equal(player().economy.equipment.hat.id,reference.id,'reference use preserves the owned Hat');
  await ui.waitForUi(page,state => state.widgets.filter(widget => widget.id.startsWith('panel:shop:')).length===offers.length,
    'every accepted reference offer is painted');
  await shot('controlled-reference-offers');
  const offer = [...offers].sort((a,b) => a.price-b.price)[0];
  const beforeGold = player().economy.gold, beforeIds = new Set(player().economy.backpack.map(item => item.id));
  await ui.click(page,`panel:shop:${offer.id}`); await ui.click(page,`panel:shop:${offer.id}`);
  await until(() => player().economy.backpack.some(item => !beforeIds.has(item.id)
    &&item.recipe_index===offer.recipe_index), 'the real paid offer enters owned inventory');
  const purchased = player().economy.backpack.find(item => !beforeIds.has(item.id)&&item.recipe_index===offer.recipe_index);
  assert.equal(player().economy.gold,beforeGold-offer.price);
  assert.equal(player().economy.dowsing_offers.length,0);
  const nextFee = player().economy.dowsing_fee;
  assert.ok(nextFee>=500&&nextFee<=950&&nextFee%50===0,'purchase draws the canonical next fee');
  await ui.widget(page,`panel:item:${purchased.id}`); await ui.value(page,'panel:gold',String(beforeGold-offer.price));
  await shot('controlled-paid-dowsing-purchase');
  receipt.checks.push({controlledReferencePurchase:{qualification:receipt.funding.qualification,
    reference,requested:request,fee,offers,purchased,price:offer.price,beforeGold,afterGold:player().economy.gold,nextFee}});
  await ui.click(page,'panel:close'); await ui.noWidget(page,'panel:service');
  await until(() => !player().economy.dowsing_rolled,'real Done retires the purchased roll');
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
    await ui.observeAnnouncements(page);
    await page.goto(url); await ui.ready(page);
    await startNewGame(page, {name:'Aureus',element:'fire',discipline:'arcane'}); await gameplayReady();
    assert.equal(player().economy.gold,500,'the natural phase starts with the unmodified fresh-profile balance');
    receipt.naturalStart = {tick:frame.snapshot.ticks,gold:player().economy.gold,selection:player().selection};
    goldAnnouncementStart = (await ui.announcements(page)).length;
    for (let attempt = 0; attempt < 3; attempt++) {
      lastPrimaryPulse = 0; lastSecondaryTick = -1000;
      await ui.click(page, 'hud:map');
      await until(() => world() && !frame.host.loading_run_id && !frame.host.resume_grace && !frame.host.pause,
        'ordinary Boneyard admission', 120000);
      await gameplayReady(); assert.equal(world().tutorial, null);
      const run = {runId:world().run_id, sourceSha256:world().source_sha256,
        geometrySha256:world().geometry_sha256, seed:world().seed, approach:[], samples:[]};
      receipt.runs.push(run);
      const plan = await prepareBoneyardNavigation(world());
      const runOutput = path.join(output, `run-${attempt+1}`);
      fs.mkdirSync(runOutput, {recursive:true});
      run.approach = await walkSolomonEntrance(page, {snapshot:() => frame.snapshot,
        playerId:() => playerId, output:runOutput, plan});
      await combat(plan,run);
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
    assert.ok(receipt.goldNotification,'an actual local Gold pickup paints and announces its native notice');
    assert.ok(player().economy.gold >= 650, 'actual pickups pay the Source650gold fee');
    const shlorio = await ui.byLabel(page, 'Shlorio', {role:'button',prefix:'hud:shortcut:'});
    await ui.click(page, shlorio.id); await ui.widget(page, 'panel:service');
    const before = player().economy.gold, fee = player().economy.dowsing_fee;
    assert.equal(fee, 650, 'the first unpaid Source Dowse fee is exactly650gold');
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
    await fundThroughPrivateControls(); await referenceAndPurchase();
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
