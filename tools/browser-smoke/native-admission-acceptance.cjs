// Diagnostic: ordinary browser creation and fresh-document Continue against the real host.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
const {createNativeMessageReader} = require('./native-wire.cjs');
const {startNewGame} = require('./menu-journey.cjs');
const ui = require('./gpu-ui.cjs');
const output = path.resolve(process.env.OUT);
fs.mkdirSync(output, {recursive:true});
const receipt = {pass:false, checks:[], admissions:[], saves:0};
(async () => {
  const owner = await launchOwnedBrowser(chromium, {headless:true, executablePath:process.env.CHROME,
    args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
  let page, observed;
  try {
    page = await owner.browser.newPage({viewport:{width:1600,height:900}});
    observed = ui.watchErrors(page);
    await observed.ready;
    let statusCount = 0;
    page.on('websocket', socket => {
      const read = createNativeMessageReader();
      socket.on('framereceived', ({payload}) => {
        const message = read(JSON.parse(payload.toString()));
        if (message?.type === 'session_status') statusCount++;
        if (message?.type === 'save_committed') receipt.saves++;
        if (message?.type === 'welcome') receipt.admissions.push({
          playerId:message.credentials.player_id,
          selection:message.frame.snapshot.participants[message.credentials.player_id].player.selection,
        });
      });
    });
    await page.goto(process.env.URL);
    await ui.ready(page);
    await page.screenshot({timeout:120000,path:path.join(output,'title.png')});
    await ui.waitForUi(page, () => statusCount > 0, 'real host title inspection');
    const selection = await startNewGame(page,{name:'Lucian',element:'air'});
    assert.ok(receipt.admissions.length > 0);
    const firstId = receipt.admissions.at(-1).playerId;
    await page.screenshot({timeout:120000,path:path.join(output,'created.png')});
    receipt.checks.push('real InspectSession and Hello admit the selected wizard');
    const leave = async () => {
      const before = receipt.saves;
      await page.keyboard.press('Escape');
      await ui.surface(page,'pause');
      await ui.click(page,'leave');
      await ui.surface(page,'title');
      assert.ok(receipt.saves > before,'Leave waits for the real durable host acknowledgement');
    };
    await leave();
    receipt.checks.push('first Leave receives durable host SaveCommitted');
    await ui.settleErrors(page);
    const inspectionsBeforeReload = statusCount;
    await ui.observeDocumentReplacement(page, () => page.reload());
    await ui.ready(page);
    await page.screenshot({timeout:120000,path:path.join(output,'reloaded-title.png')});
    await ui.waitForUi(page, () => statusCount > inspectionsBeforeReload, 'fresh-document host inspection');
    await ui.click(page,'play');
    const admissionsBeforeContinue = receipt.admissions.length;
    await ui.click(page,'continue');
    await ui.surface(page,'hidden');
    await ui.waitForUi(page, state => state.acceptedSelection?.name === selection.name
      && !state.inputScope && ui.findWidget(state,'hud:health'), 'accepted continued wizard');
    assert.ok(receipt.admissions.length > admissionsBeforeContinue);
    assert.equal(receipt.admissions.at(-1).playerId,firstId);
    assert.deepEqual((await ui.readUi(page)).acceptedSelection,selection);
    await page.screenshot({timeout:120000,path:path.join(output,'continued.png')});
    receipt.checks.push('fresh-document Continue retains the real host identity and accepted selection');
    await leave();
    await ui.settleErrors(page);
    assert.deepEqual(observed.errors,[]);
    receipt.checks.push('continued Leave persists with no unexpected browser errors');
    receipt.behaviorPassed = true;
  } catch (error) {
    receipt.failure = String(error.stack || error);
    if (page && !page.isClosed()) {
      try { await page.screenshot({timeout:120000,path:path.join(output,'failure.png')}); }
      catch (capture) { receipt.captureFailure = String(capture); }
    }
    throw error;
  } finally {
    try { if (page && !page.isClosed()) await ui.settleErrors(page); }
    catch (error) { receipt.settleFailure = String(error); throw error; }
    finally {
      try { receipt.cleanup = await owner.close(); }
      finally {
        receipt.errors = observed?.errors || [];
        receipt.pass = receipt.behaviorPassed === true && receipt.errors.length === 0
          && receipt.cleanup?.exitCode === 0 && !receipt.settleFailure;
        fs.writeFileSync(path.join(output,'receipt.json'),JSON.stringify(receipt,null,2)+'\n');
      }
    }
  }
})().catch(error => {console.error(error);process.exitCode = 1;});
