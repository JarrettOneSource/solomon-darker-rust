// The real primary selector seals world movement, accepts a choice, and cancels.
const assert = require('node:assert/strict');
const path = require('node:path');
const fs = require('node:fs');
const { chromium } = require(path.resolve(process.env.PW));
const { startNewGame } = require('./menu-journey.cjs');
const ui = require('./gpu-ui.cjs');
const {launchOwnedBrowser}=require('./browser-lifecycle.cjs');
const output = process.env.OUT || '/tmp/hud-selector';
fs.mkdirSync(output, {recursive:true});

(async () => {
  const owned = await launchOwnedBrowser(chromium,{ headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=metal', '--ignore-gpu-blocklist'] });
  const browser=owned.browser;
  const receipt={passed:false,artifact:process.env.ARTIFACT,generatedName:process.env.GENERATED_NAME==='1'};
  let page, observed;
  let phase = 'admission';
  try {
    page = await browser.newPage({ viewport: { width: 1600, height: 900 }, hasTouch: true });
    observed = ui.watchErrors(page);await observed.ready;
    await page.goto(process.env.URL || 'http://127.0.0.1:4185/');
    await ui.ready(page);
    await startNewGame(page,{name:process.env.GENERATED_NAME==='1'?null:'Helvidius'});
    phase = 'desktop selector';
    const open = async () => { await ui.click(page, 'hud:skill-binding:12'); await ui.widget(page, 'panel:skill-selector'); };
    await open();
    assert.equal((await ui.widget(page, 'panel:hud-select-skill:8')).selected, true);
    const before = await page.evaluate(() => Array.from(wasmBindings.wizard_state()));
    await page.keyboard.down('KeyS'); await ui.nextFrame(page); await page.keyboard.up('KeyS');
    const after = await page.evaluate(() => Array.from(wasmBindings.wizard_state()));
    assert.deepEqual(after.slice(0, 2), before.slice(0, 2), 'the selector seals player movement');
    if (process.env.OUT) await page.screenshot({ path: path.join(process.env.OUT, 'hud-primary-selector-rust.png') });
    await ui.click(page, 'panel:hud-select-skill:8'); await ui.noWidget(page, 'panel:skill-selector');
    await open(); await page.keyboard.press('Escape'); await ui.noWidget(page, 'panel:skill-selector');
    phase = 'landscape touch selector';
    await page.setViewportSize({ width: 844, height: 390 }); await open();
    const bounds = ui.bounds(await ui.widget(page, 'panel:hud-select-skill:8'));
    assert.ok(bounds.width >= 44 && bounds.height >= 44 && bounds.x >= 0 && bounds.x + bounds.width <= 844);
    if (process.env.OUT) await page.screenshot({ path: path.join(process.env.OUT, 'phone-hud-primary-selector-rust.png') });
    await ui.tap(page, 'panel:hud-select-skill:8'); await ui.noWidget(page, 'panel:skill-selector');
    await ui.assertGpuOnly(page); await ui.settleErrors(page); assert.deepEqual(observed.errors, []);
    receipt.passed=true;
  } catch(error) {
    if(page){await ui.settleErrors(page);await page.screenshot({path:path.join(output,'failure.png')});}
    Object.assign(receipt,{phase,error:error.stack,ui:page?await ui.readUi(page).catch(failure=>({readError:failure.message})):null});
    throw error;
  } finally {
    try {if(page)await ui.settleErrors(page)}
    catch(error){receipt.passed=false;receipt.failure??=error.stack;throw error}
    finally {
      try{receipt.cleanup=await owned.close()}
      catch(error){receipt.passed=false;receipt.cleanup=error.cleanup;throw error}
      finally {
        receipt.errors=observed?.errors||[];
        if(receipt.errors.length)receipt.passed=false;
        fs.writeFileSync(path.join(output,'receipt.json'),JSON.stringify(receipt,null,2));
      }
    }
  }
  assert.deepEqual(receipt.errors,[]);
  console.log(JSON.stringify(receipt));
})().catch(error => { console.error(error); process.exitCode = 1; });
