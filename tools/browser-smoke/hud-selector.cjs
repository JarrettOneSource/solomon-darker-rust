// The real primary selector seals world movement, accepts a choice, and cancels.
const assert = require('node:assert/strict');
const path = require('node:path');
const fs = require('node:fs');
const { chromium } = require(path.resolve(process.env.PW));
const { startNewGame } = require('./menu-journey.cjs');
const ui = require('./gpu-ui.cjs');
const output = process.env.OUT || '/tmp/hud-selector';
fs.mkdirSync(output, {recursive:true});

(async () => {
  const browser = await chromium.launch({ headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  let page, observed;
  let phase = 'admission';
  try {
    page = await browser.newPage({ viewport: { width: 1600, height: 900 }, hasTouch: true });
    observed = ui.watchErrors(page);
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
    await ui.assertGpuOnly(page); assert.deepEqual(observed.errors, []);
    const receipt = {passed:true, generatedName:process.env.GENERATED_NAME==='1', errors:observed.errors};
    fs.writeFileSync(path.join(output,'receipt.json'),JSON.stringify(receipt,null,2));
    console.log(JSON.stringify(receipt));
  } catch(error) {
    if(page)await page.screenshot({path:path.join(output,'failure.png')});
    fs.writeFileSync(path.join(output,'receipt.json'),JSON.stringify({passed:false,phase,
      error:error.stack,errors:observed?.errors,ui:page?await ui.readUi(page).catch(failure=>({readError:failure.message})):null},null,2));
    throw error;
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
