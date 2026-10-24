// Controlled prerequisite funding through the actual private-hosted Cheats UI.
// A fresh owned wizard uses SET GOLD, then drags an owned Hat into the mirror,
// pays the Source650gold reference fee and actually purchases a5000–5700gold offer.
// These funds are not naturally earned. No save, seed or network action is injected.
// Set PW, URL, OUT, LOCAL_TLS and SOLOMON_WEBSITE_DIR; Node needs
// --experimental-strip-types for the unchanged Source equipment catalog.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {pathToFileURL} = require('node:url');
const {chromium} = require(path.resolve(process.env.PW));
const {startNewGame} = require('./menu-journey.cjs');
const {createNativeMessageReader} = require('./native-wire.cjs');
const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
const {revealSettingsControl} = require('./settings-journey.cjs');
const ui = require('./gpu-ui.cjs');

const output = path.resolve(process.env.OUT || 'private-dowsing');
const url = process.env.URL || 'https://127.0.0.1:9449/game-stg/';
const receipt = {passed:false, artifact:process.env.ARTIFACT, checks:[], welcomes:[],
  commands:[], results:[], errors:[], qualification:'controlled private-UI funding; not naturally earned'};
let page, observed, frame, playerId;
const player = () => frame?.snapshot.participants[playerId]?.player;

async function until(predicate, label, timeout = 30000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    assert.deepEqual(observed.errors, []);
    assert.deepEqual(receipt.errors, []);
    if (await predicate()) return;
    await page.waitForTimeout(50);
  }
  throw new Error(`Timed out: ${label}`);
}
async function shot(name) { await page.screenshot({path:path.join(output, `${name}.png`)}); }
async function gameplayReady() {
  await until(() => player() && !frame.host.loading_run_id && !frame.host.resume_grace,
    'the actual native participant is admitted', 120000);
  await ui.waitForUi(page, state => state.surface === 'hidden' && !state.inputScope
    && !state.capture.text && ui.findWidget(state, 'hud:health'), 'genuine gameplay input admission');
}
function observeAuthority() {
  page.on('websocket', socket => {
    const read = createNativeMessageReader();
    socket.on('framereceived', ({payload}) => {
      try {
        const message = read(JSON.parse(payload.toString()));
        if (!message) return;
        if (message.type === 'welcome') {
          playerId = message.credentials.player_id; frame = message.frame;
          receipt.welcomes.push({sessionKind:message.hosted?.session_kind,
            cheatsEnabled:message.hosted?.cheats_enabled, playerId});
        } else if (message.type === 'snapshot') frame = message.frame;
        else if (message.type === 'command_result') receipt.results.push(message);
        else if (message.type === 'hosted' && message.message.kind === 'lua_result')
          receipt.results.push(message);
        else if (message.type === 'error') receipt.errors.push({code:message.code, reason:message.reason});
      } catch (error) { receipt.errors.push({protocol:error.stack}); }
    });
    socket.on('framesent', ({payload}) => {
      const message = JSON.parse(payload.toString());
      if (message.type === 'command') receipt.commands.push({kind:message.command.kind,
        action:message.command.action, sequence:message.sequence});
      if (message.type === 'hosted') receipt.commands.push({kind:`hosted:${message.command.kind}`,
        requestId:message.command.request_id});
    });
  });
}

async function fundThroughPrivateControls() {
  const welcome = receipt.welcomes.at(-1);
  assert.equal(welcome?.sessionKind, 'PrivateCollege', 'the accepted host supplies private admission');
  assert.equal(welcome.cheatsEnabled, true, 'the accepted host grants the supported Cheats controls');
  const before = player().economy.gold;
  assert.equal(before,500,'this separate controlled case starts with a fresh stock balance');
  await page.keyboard.press('Backquote'); await ui.widget(page, 'cheats/dialog');
  await ui.fill(page, 'cheats/gold', '10000'); await ui.click(page, 'cheats/submit:gold');
  await until(() => player().economy.gold === 10000, 'the supported private Gold control is accepted');
  await ui.widget(page, 'cheats/submit:gold', {enabled:true});
  const action = await ui.widget(page, 'cheats/action-receipt');
  assert.equal(action.role,'status'); assert.ok(action.label.includes('Gold set')&&!action.label.endsWith('…'));
  receipt.funding = {qualification:receipt.qualification, before, after:player().economy.gold,
    welcome, action};
  await shot('controlled-private-gold-prerequisite');
  receipt.checks.push('the actual private welcome grants Cheats and SET GOLD accepts the controlled prerequisite');
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
  assert.equal(fee,650,'a fresh private wizard pays the first650gold fee');
  const commandsBefore = receipt.commands.length;
  await ui.click(page,'panel:dowse'); await until(() => player().economy.dowsing_rolled, 'actual reference roll');
  const request = receipt.commands.slice(commandsBefore).find(command => command.action?.Dowse);
  assert.equal(request?.action.Dowse.reference_item_id,reference.id,'real drag supplies the owned reference to the typed command');
  const offers = player().economy.dowsing_offers;
  assert.ok(offers.length===3||offers.length===4,'a no-set Hat skips the two optional matching-set slots');
  assert.ok(offers.every(offer => recipes[offer.recipe_index].nativeTypeId===reference.native_type_id));
  assert.ok(offers.every(offer => offer.price>=5000&&offer.price<=5700&&offer.price%50===0),
    'every rare offer uses the Source5000–5700gold price rule');
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
    page = await owned.browser.newPage({viewport:{width:1600,height:900},
      ignoreHTTPSErrors:process.env.LOCAL_TLS === '1'});
    observed = ui.watchErrors(page); await observed.ready; observeAuthority();
    await page.goto(url); await ui.ready(page);
    if (ui.findWidget(await ui.readUi(page), 'prompt-secondary')) {
      await ui.click(page, 'prompt-secondary'); await ui.noWidget(page, 'prompt-secondary');
    }
    await ui.click(page, 'settings'); await ui.click(page, 'settings-page:cloud');
    if ((await ui.widget(page, 'settings-toggle:shared-hub')).selected)
      await ui.click(page, 'settings-toggle:shared-hub');
    await revealSettingsControl(page, 'settings-toggle:cheats');
    if (!(await ui.widget(page, 'settings-toggle:cheats')).selected)
      await ui.click(page, 'settings-toggle:cheats');
    assert.equal((await ui.widget(page, 'settings-toggle:cheats')).selected,true);
    await ui.click(page, 'settings-back'); await ui.click(page, 'settings-back');
    await ui.surface(page, 'title');
    await startNewGame(page, {name:'Aureus',element:'fire',discipline:'arcane'}); await gameplayReady();
    receipt.initial = {tick:frame.snapshot.ticks, gold:player().economy.gold, selection:player().selection};
    await fundThroughPrivateControls(); await referenceAndPurchase();
    await page.keyboard.press('Escape'); await ui.surface(page, 'pause');
    await ui.click(page, 'leave'); await ui.surface(page, 'title');
    await ui.assertGpuOnly(page); await ui.settleErrors(page);
    assert.deepEqual(observed.errors, []); assert.deepEqual(receipt.errors, []); receipt.passed = true;
  } catch (error) {
    failure = error; receipt.failure = error.stack;
    if (page) {
      receipt.failedPlayer = player();
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
  console.log(JSON.stringify({passed:receipt.passed, checks:receipt.checks, cleanup:receipt.cleanup}));
  if (failure) throw failure;
})().catch(error => { console.error(error); process.exitCode = 1; });
