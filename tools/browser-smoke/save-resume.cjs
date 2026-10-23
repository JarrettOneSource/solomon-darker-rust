// Actual IndexedDB writes and full page reloads. The second tab must never overwrite
// the first tab's newer run; the test reads the stored document rather than a UI counter.
const assert = require('node:assert/strict');
const path = require('node:path');
const ui = require('./gpu-ui.cjs');
const { chromium } = require(path.resolve(process.env.PW));
const { startNewGame } = require('./menu-journey.cjs');
const { readSave, waitForSave: saved, localParticipant, savedWizardState } = require('./saved-game.cjs');
const url = process.env.URL || 'http://127.0.0.1:4181/';
const output = process.env.OUT || '/tmp';
const screen = ui.surface;
const wizard = page => page.evaluate(() => Array.from(window.wasmBindings.wizard_state()));

async function loaded(page) {
  await ui.ready(page);
  await screen(page, 'title');
}

async function resume(page) {
  await ui.click(page,'play');
  assert.equal((await ui.widget(page,'continue')).enabled,true);
  await ui.click(page,'continue');
  await screen(page, 'hidden');
}

(async () => {
  const browser = await chromium.launch({headless:true,
    executablePath:process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
  const errors=[];
  try {
    const context=await browser.newContext({viewport:{width:1600,height:900}});
    context.on('page',page=>ui.watchErrors(page,{errors}));
    const page=await context.newPage();
    await page.goto(url); await loaded(page);
    await startNewGame(page,{name:'Felix'});
    await ui.click(page,(await ui.byLabel(page,'Fomentius',{prefix:'hud:',role:'button'})).id);
    const potion=await ui.byLabel(page,'Buy Health Potion for 150 gold',{prefix:'panel:',role:'button'});
    await ui.click(page,potion.id);await ui.click(page,potion.id);
    await ui.byLabel(page,'Health Potion, quantity 2',{prefix:'panel:item:',role:'button'});
    await saved(page,record=>localParticipant(record).player.economy.gold===350);
    await ui.click(page,'panel:close');
    await ui.noWidget(page,'panel:close');
    await page.keyboard.down('KeyS'); await page.waitForTimeout(700); await page.keyboard.up('KeyS');
    await page.waitForTimeout(300);
    await page.keyboard.press('Escape'); await screen(page,'pause');
    const paused=await wizard(page);
    const hubBeforeReload=await saved(page,record=>record.checkpoint.ticks>=paused[5]);
    await ui.click(page,'pause-settings');await screen(page,'settings');
    await ui.setSlider(page,'settings-range:ui-scale',125);
    const settingsKey=await page.evaluate(()=>{
      const mount=(document.querySelector('meta[name="solomon-website-mount"]')?.content||'').replace(/\/+$/,'');
      return `solomon-dark-game-settings-v1${mount?`:${mount}`:''}`;
    });
    await page.waitForFunction(key=>JSON.parse(localStorage.getItem(key))?.uiScalePercent===125,settingsKey);
    await ui.observeDocumentReplacement(page,()=>page.reload()); await loaded(page);
    // College keeps ticking behind Pause/Settings. Compare with the completed
    // document actually loaded at Title, including any final pagehide write.
    const hubSave=await readSave(page);
    assert.ok(hubSave.revision>=hubBeforeReload.revision);
    assert.ok(hubSave.checkpoint.ticks>=paused[5]);
    assert.deepEqual((await wizard(page)).slice(0,6),savedWizardState(hubSave),
      'reload restores the persisted participant and completed College tick');
    assert.equal((await wizard(page))[6],paused[6],'reload preserves the current College room');
    await resume(page);
    await ui.click(page,'hud:inventory');
    await ui.byLabel(page,'Health Potion, quantity 2',{prefix:'panel:item:',role:'button'});
    assert.equal((await ui.widget(page,'panel:gold')).value,'350');
    await page.screenshot({path:path.join(output,'saved-inventory.png')});
    await ui.click(page,'panel:close');
    await ui.noWidget(page,'panel:close');
    const meter=ui.bounds(await ui.widget(page,'hud:health'));
    assert.ok(Math.abs(meter.width-137.5)<0.1,'stored UI scale is applied to the real HUD');

    await ui.click(page,'hud:map');
    await saved(page,record=>record.checkpoint.boneyard!==null);
    await page.keyboard.down('KeyS'); await page.waitForTimeout(700); await page.keyboard.up('KeyS');
    await page.waitForTimeout(300);
    await page.keyboard.press('Escape'); await screen(page,'pause');
    const boneyardPaused=await wizard(page);
    const runSave=await saved(page,record=>record.checkpoint.boneyard && record.checkpoint.ticks===boneyardPaused[5]);
    await ui.observeDocumentReplacement(page,()=>page.reload()); await loaded(page);
    assert.deepEqual((await wizard(page)).slice(0,6),savedWizardState(runSave),
      'reload restores the same Boneyard participant and tick');
    assert.equal((await wizard(page))[6],boneyardPaused[6],'reload preserves the current region');
    await page.screenshot({path:path.join(output,'saved-title.png')});

    const other=await context.newPage(); await other.goto(url); await loaded(other);
    await resume(page);
    await page.keyboard.down('KeyD'); await page.waitForTimeout(450); await page.keyboard.up('KeyD');
    await page.waitForTimeout(250);
    await page.keyboard.press('Escape'); await screen(page,'pause');
    const newerState=await wizard(page);
    const newer=await saved(page,record=>record.checkpoint.ticks===newerState[5] && record.revision>runSave.revision);
    await resume(other);
    await other.keyboard.down('KeyA'); await other.waitForTimeout(300); await other.keyboard.up('KeyA');
    await other.keyboard.press('Escape'); await screen(other,'pause');
    await other.waitForFunction(()=>JSON.parse(window.wasmBindings.gpu_ui_state()).error?.includes('another tab'));
    const protectedSave=await readSave(page);
    assert.equal(protectedSave.document,newer.document,'a stale writer preserves the newer complete document');
    assert.equal(protectedSave.revision,newer.revision);
    await ui.assertGpuOnly(page);
    await ui.settleErrors(page);
    await ui.settleErrors(other);
    assert.deepEqual(errors,[]);
    console.log(JSON.stringify({passed:true,checks:['College reload','inventory and gold','saved preferences','Boneyard reload','atomic stale-tab rejection'],revisions:[hubSave.revision,runSave.revision,newer.revision],errors}));
  } finally {await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
