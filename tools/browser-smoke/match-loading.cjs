// Hold an actual host admission, then close that transport and retry through Title.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(process.env.PW);
const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
const {startNewGame} = require('./menu-journey.cjs');
const {createNativeMessageReader} = require('./native-wire.cjs');
const ui = require('./gpu-ui.cjs');
const output = path.resolve(process.env.OUT);
const receipt = {passed:false, checks:[], layouts:[], styleErrors:[], admissions:[]};
const faultReason = 'Owned loading connection interruption';
let armed = false, held, page, step = 'ordinary startup';

(async () => {
  fs.mkdirSync(output, {recursive:true});
  const owner = await launchOwnedBrowser(chromium, {headless:true,
    executablePath:process.env.CHROME, args:['--use-angle=metal', '--ignore-gpu-blocklist']});
  let failure;
  const context = await owner.browser.newContext({ignoreHTTPSErrors:true,
    viewport:{width:1600, height:900}});
  page = await context.newPage();
  const observed = ui.watchErrors(page);
  await observed.ready;
  await page.routeWebSocket('**', peer => {
    const server = peer.connectToServer();
    const read = createNativeMessageReader();
    const hold = armed;
    if (hold) {
      armed = false;
      held = {peer, server, bytes:0, types:[], welcome:false, closed:false};
    }
    server.onMessage(message => {
      const raw = JSON.parse(message.toString());
      const decoded = read(raw);
      if (hold) {
        held.bytes += Buffer.byteLength(message);
        if (!held.types.includes(raw.type)) held.types.push(raw.type);
        held.welcome ||= decoded?.type === 'welcome';
      } else {
        if (decoded?.type === 'welcome') receipt.admissions.push({
          kind:decoded.hosted?.session_kind,
          selection:decoded.frame.snapshot.participants[decoded.credentials.player_id].selection,
        });
        peer.send(message);
      }
    });
  });
  const closeHeld = async () => {
    if (!held || held.closed) return;
    held.closed = true;
    await Promise.all([held.peer.close({code:4000, reason:faultReason}),
      held.server.close({code:4000, reason:faultReason})]);
  };
  try {
    await page.goto(process.env.URL);
    await ui.ready(page);
    if (ui.findWidget(await ui.readUi(page), 'prompt-secondary')) await ui.click(page, 'prompt-secondary');
    await ui.click(page, 'play');
    armed = true;
    await ui.click(page, 'new-game');
    step = 'held actual native Welcome';
    await ui.waitForUi(page, state => held?.welcome && state.inputScope === 'match-loading'
      && ui.findWidget(state, 'match-loading:label'), 'real host Welcome held behind the visible loading screen');
    receipt.held = {bytes:held.bytes, types:held.types, welcome:held.welcome};
    const before = await ui.readUi(page);
    assert.equal(before.acceptedSelection, null);
    await page.keyboard.press('Escape');
    await page.keyboard.press('KeyI');
    await ui.waitForUi(page, state => state.inputScope === 'match-loading', 'loading retains its input scope');
    assert.equal((await ui.widget(page, 'match-loading:progress')).value, '52');
    assert.equal(receipt.admissions.length, 0);
    receipt.checks.push('an actual native Welcome remains unaccepted while the delayed loading screen seals keyboard input');

    step = 'Source loading label and track at desktop, phone and wide sizes';
    for (const [width, height] of [[1600,900], [390,844], [2200,1100]]) {
      await page.setViewportSize({width,height});
      await ui.nextFrame(page);
      const label = ui.bounds(await ui.widget(page, 'match-loading:label'));
      const track = ui.bounds(await ui.widget(page, 'match-loading:progress'));
      const lineHeight = Math.max(22.4, Math.min(48, height * .02962963));
      const bottom = height - height * .075 - Math.max(12, height * .01296296) - .5;
      const comparisons = {lineHeight:[label.height,lineHeight],
        labelBottom:[label.y+label.height,bottom],
        trackLeft:[track.x,width*.2-.5], trackTop:[track.y,height*.925-.5],
        trackWidth:[track.width,width*.6], trackHeight:[track.height,Math.max(8,Math.min(10,height*.0083))]};
      for (const [name, [actual, expected]] of Object.entries(comparisons)) {
        if (Math.abs(actual-expected)>.05) receipt.styleErrors.push({width,height,name,actual,expected});
      }
      receipt.layouts.push({width,height,label,track});
      await page.screenshot({path:path.join(output, `loading-${width}x${height}.png`)});
    }
    await ui.assertGpuOnly(page);
    step = 'declared transport failure and ordinary retry';
    await closeHeld();
    await ui.surface(page, 'title');
    await ui.noWidget(page, 'match-loading:label');
    await ui.value(page, 'runtime:status', faultReason);
    await page.setViewportSize({width:1600,height:900});
    await page.screenshot({path:path.join(output, 'loading-connection-error.png')});
    receipt.checks.push('the declared real transport interruption retires loading and restores Title with its actual reason');
    await startNewGame(page, {name:'Lucian', element:'air', discipline:'arcane'});
    assert.equal(receipt.admissions.length, 1);
    await ui.noWidget(page, 'match-loading:label');
    await ui.noWidget(page, 'runtime:status');
    await page.keyboard.press('Escape');
    await ui.click(page, 'leave');
    await ui.surface(page, 'title');
    receipt.checks.push('ordinary retry completes College admission, Create and a saved Leave');
    assert.deepEqual(receipt.styleErrors, [], 'loading label and progress follow the pinned Source geometry');
    receipt.checks.push('desktop, phone and wide loading label line boxes and tracks match Source geometry');
    receipt.passed = true;
  } catch (error) {
    failure = error;
    receipt.step = step;
    receipt.failure = error.stack;
    receipt.failedUi = await ui.readUi(page).catch(error => ({error:error.message}));
    await page.screenshot({path:path.join(output,'failure.png')}).catch(()=>{});
  } finally {
    try {await closeHeld(); await ui.settleErrors(page);} catch (error) {failure ||= error;}
    receipt.errors = observed.errors;
    if (receipt.errors.length) {failure ||= new Error('Unexpected browser errors'); receipt.passed = false;}
    try {receipt.cleanup = await owner.close();}
    catch (error) {failure ||= error; receipt.passed = false; receipt.cleanup = error.cleanup;}
    fs.writeFileSync(path.join(output,'receipt.json'),JSON.stringify(receipt,null,2));
  }
  console.log(JSON.stringify({passed:receipt.passed,checks:receipt.checks,step,styleErrors:receipt.styleErrors,cleanup:receipt.cleanup}));
  if (failure) throw failure;
})().catch(error=>{console.error(error);process.exitCode=1});
