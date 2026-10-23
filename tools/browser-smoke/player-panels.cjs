// Real Inventory and Skillbook transactions through visible GPU widget geometry.
const assert = require('node:assert/strict');
const path = require('node:path');
const fs = require('node:fs');
const { chromium } = require(path.resolve(process.env.PW));
const { startNewGame } = require('./menu-journey.cjs');
const ui = require('./gpu-ui.cjs');
const output = process.env.OUT || '/tmp';
fs.mkdirSync(output,{recursive:true});
const state = page => page.evaluate(() => Array.from(window.wasmBindings.wizard_state()));
const shot = (page, name) => page.screenshot({ path: path.join(output, name) });
const item = (page, name) => ui.byLabel(page, name, { role: 'button', prefix: 'panel:item:' });
const notice = (page, title) => ui.waitForUi(page, snapshot => {
  const actual = ui.findWidget(snapshot, 'panel:notice-title');
  return actual && (typeof title === 'string' ? actual.value === title : title.test(actual.value));
}, `Inventory notice ${title}`);

(async () => {
  const browser = await chromium.launch({ headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  let page;
  try {
    page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
    const { errors } = ui.watchErrors(page);
    await page.goto(process.env.URL || 'http://127.0.0.1:4185/'); await ui.ready(page);
    await startNewGame(page, {name:process.env.GENERATED_NAME==='1'?null:'Felix'});
    await page.keyboard.press('KeyI'); await ui.widget(page, 'panel:surface:inventory');
    const frozen = await state(page);
    await page.keyboard.down('KeyS'); await ui.nextFrame(page); await page.keyboard.up('KeyS');
    assert.deepEqual((await state(page)).slice(0, 2), frozen.slice(0, 2), 'Inventory owns local movement');
    await shot(page,'inventory-rust.png');
    const health = await item(page, 'Health Potion, quantity 1');
    await ui.click(page, health.id);
    await ui.waitForUi(page, snapshot => ui.findWidget(snapshot, 'panel:tooltip')?.label.includes('Restores your health to maximum'), 'potion detail');
    await shot(page,'inventory-rust-potion.png');
    await ui.click(page, health.id, {clickCount: 2}); await ui.noWidget(page, health.id);
    const mana = await item(page, 'Mana Potion, quantity 1');
    const destination = ui.center(ui.bounds(await ui.widget(page, 'panel:cell:5')));
    await ui.drag(page, mana.id, 'panel:cell:5');
    await ui.waitForUi(page, snapshot => {
      const actual = ui.findWidget(snapshot, mana.id); if (!actual) return false;
      const point = ui.center(ui.bounds(actual));
      return Math.abs(point.x - destination.x) < 1 && Math.abs(point.y - destination.y) < 1;
    }, 'Mana Potion occupies addressed backpack slot 5');
    await ui.click(page, 'panel:equipment:hat', {clickCount: 2});
    await notice(page, 'A WIZARD WOULD NEVER REMOVE HIS HAT!');
    await ui.click(page, 'panel:notice-okay'); await ui.noWidget(page, 'panel:notice-title');
    await ui.click(page, 'panel:equipment:weapon', {clickCount: 2});
    await ui.waitForUi(page, snapshot => ui.findWidget(snapshot, 'panel:equipment:weapon')?.label.endsWith('empty'), 'unequipped weapon');
    const staff = await item(page, 'Staff, quantity 1');
    await ui.click(page, staff.id, {clickCount: 2});
    await ui.waitForUi(page, snapshot => !ui.findWidget(snapshot, 'panel:equipment:weapon')?.label.endsWith('empty'), 'equipped Staff');
    await ui.click(page, 'panel:equipment:weapon', {clickCount: 2}); await ui.widget(page, staff.id);
    await ui.click(page, staff.id); await ui.click(page, 'panel:unforge');
    await notice(page,'REALLY UNFORGE THIS?'); await shot(page,'inventory-rust-unforge-confirm.png');
    await ui.click(page,'panel:unforge-cancel'); await ui.widget(page,staff.id);
    await ui.click(page,'panel:unforge'); await ui.click(page,'panel:unforge-confirm');
    await notice(page,/STAFF UNFORGED|FAILED UNFORGING!/); await shot(page,'inventory-rust-unforge-result.png');
    await ui.click(page,'panel:notice-okay'); await ui.noWidget(page,staff.id);
    await page.keyboard.press('KeyK'); await ui.widget(page,'panel:surface:skills');
    assert.equal((await ui.readUi(page)).widgets.filter(widget => widget.id.startsWith('panel:skill:')).length,2,'fresh book contains only learned skills');
    await shot(page,'skillbook-rust.png');
    const primary = ui.center(ui.bounds(await ui.widget(page,'panel:skill:8')));
    await page.mouse.move(primary.x,primary.y);
    await ui.waitForUi(page, snapshot => ui.findWidget(snapshot,'panel:tooltip')?.label.includes('MAGIC MISSILE'), 'learned primary detail');
    await shot(page,'skillbook-rust-magic-missile.png');
    await ui.drag(page,'panel:skill:11','panel:belt:2');
    await ui.waitForUi(page, snapshot => ui.findWidget(snapshot,'panel:belt:2')?.label.includes('assign or drag away'),'real Skillbook belt binding');
    await page.keyboard.press('Escape'); await ui.noWidget(page,'panel:surface:skills');
    assert.equal((await ui.readUi(page)).surface,'hidden','panel Escape does not also open pause');
    await page.keyboard.press('KeyI');await ui.widget(page,'panel:surface:inventory');
    await page.keyboard.press('Escape');await ui.noWidget(page,'panel:surface:inventory');
    await page.keyboard.press('KeyK');await ui.widget(page,'panel:surface:skills');
    await page.keyboard.press('Escape');await ui.noWidget(page,'panel:surface:skills');
    await page.waitForFunction(tick => window.wasmBindings.wizard_state()[5] > tick,frozen[5]);
    await ui.assertGpuOnly(page); await ui.settleErrors(page); assert.deepEqual(errors,[]);
    console.log(JSON.stringify({passed:true,generatedName:process.env.GENERATED_NAME==='1',errors,screenshots:output}));
  } catch(error) {
    if(page){await ui.settleErrors(page);await shot(page,'player-panels-failure.png');fs.writeFileSync(path.join(output,'player-panels-failure.json'),JSON.stringify({error:error.stack,ui:await ui.readUi(page).catch(failure=>({readError:failure.message}))},null,2));}
    throw error;
  } finally { await browser.close(); }
})().catch(error=>{console.error(error);process.exitCode=1;});
