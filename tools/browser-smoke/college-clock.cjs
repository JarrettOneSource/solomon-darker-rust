// Real onboarding: the Archchancellor panel seals input without freezing the College.
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const ui = require('./gpu-ui.cjs');
const { chromium } = require(path.resolve(process.env.PW));
const { startNewGame } = require('./menu-journey.cjs');
(async () => {
  const out = process.env.OUT || path.resolve('college-clock-smoke');
  await fs.mkdir(out, {recursive:true});
  const browser = await chromium.launch({headless:true, executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'});
  const errors = [];
  const receipt = {errors};
  const page = await browser.newPage({viewport:{width:1600,height:900}});
  ui.watchErrors(page,{errors});
  try {
    await page.goto(process.env.URL || 'http://127.0.0.1:4181/');
    await ui.ready(page);
    await startNewGame(page, {name:'Vitus',introOnly:true});
    await ui.widget(page,'panel:dialogue',{timeout:120000});
    receipt.before = await page.evaluate(() => ({wizard:Array.from(window.wasmBindings.wizard_state()), npcs:Array.from(window.wasmBindings.npc_state())}));
    await page.waitForTimeout(1000);
    receipt.after = await page.evaluate(() => ({wizard:Array.from(window.wasmBindings.wizard_state()), npcs:Array.from(window.wasmBindings.npc_state())}));
    await page.screenshot({path:path.join(out,'archchancellor-dialogue.png')});
    assert.ok(receipt.after.wizard[5] > receipt.before.wizard[5], 'College ticks continue during the Archchancellor conversation');
    assert.notDeepEqual(receipt.after.npcs, receipt.before.npcs, 'College students continue their world epoch while a conversation blocks local input');
    await ui.assertGpuOnly(page);
    assert.deepEqual(errors, []);
    receipt.status='passed';
  } catch (error) {
    receipt.status='failed'; receipt.failure=error.stack;
    await page.screenshot({path:path.join(out,'failure.png')}); process.exitCode=1;
  } finally {
    await fs.writeFile(path.join(out,'receipt.json'), JSON.stringify(receipt,null,2)+'\n');
    await browser.close();
  }
  console.log(JSON.stringify(receipt));
})().catch(e=>{console.error(e);process.exitCode=1;});
