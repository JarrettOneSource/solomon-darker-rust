// Actual College guidance, Party geometry, browser input and saved Continue.
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const ui = require('./gpu-ui.cjs');
const {startNewGame} = require('./menu-journey.cjs');
const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
const output = path.resolve(process.env.OUT);
const url = process.env.URL || 'http://127.0.0.1:4181/?host=1';
const touch = process.env.TOUCH === '1';
const state = page => page.evaluate(() => Array.from(window.wasmBindings.wizard_state()));

async function walkTo(page, x, y, arrivalRadius = 12) {
  const deadline = Date.now() + 30000;
  const held = new Set();
  try {
    while (Date.now() < deadline) {
      const [atX, atY] = await state(page);
      const dx = x - atX, dy = y - atY;
      if (Math.hypot(dx,dy) < arrivalRadius) return;
      const desired = new Set();
      if (Math.abs(dx) > Math.abs(dy) * 0.45) desired.add(dx > 0 ? 'KeyD' : 'KeyA');
      if (Math.abs(dy) > Math.abs(dx) * 0.45) desired.add(dy > 0 ? 'KeyS' : 'KeyW');
      for (const key of held) if (!desired.has(key)) {await page.keyboard.up(key); held.delete(key);}
      for (const key of desired) if (!held.has(key)) {await page.keyboard.down(key); held.add(key);}
      await page.waitForTimeout(30);
    }
    throw new Error(`walk to ${x},${y} stalled at ${await state(page)}`);
  } finally {
    for (const key of held) await page.keyboard.up(key);
  }
}

(async () => {
  await fs.mkdir(output, {recursive:true});
  const owned = await launchOwnedBrowser(chromium, {
    headless:true,
    executablePath:process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=metal','--ignore-gpu-blocklist'],
  });
  const receipt = {passed:false, touch, observations:[], errors:[]};
  let page, observed;
  try {
    page = await owned.browser.newPage({viewport:{width:1600,height:900},hasTouch:touch});
    observed = ui.watchErrors(page, {errors:receipt.errors});
    await observed.ready;
    await page.goto(url, {waitUntil:'domcontentloaded'});
    await ui.ready(page);
    await startNewGame(page, {touch});
    await ui.widget(page, 'hud:pause');
    const canvas = page.locator('#game');
    const bounds = async () => JSON.parse(await canvas.getAttribute('data-npc-guidance-bounds'));
    async function capture(name, visibleHints = 0) {
      await page.waitForTimeout(400);
      if (visibleHints) {
        await page.waitForFunction(() => document.querySelector('#game').dataset.npcDirectionalHintCount === '0');
        await page.waitForFunction(count => document.querySelector('#game').dataset.npcDirectionalHintCount === String(count),visibleHints);
      }
      const guidance = await bounds();
      const partyWidget = ui.findWidget(await ui.readUi(page), 'social/party-chip-body');
      const party = partyWidget ? ui.bounds(partyWidget, {paint:true}) : null;
      if (url.includes('host=1')) assert.ok(party, 'native authority supplies the real Party card');
      if (party) for (const hint of guidance) {
        assert.ok(!(party.x < hint.x + hint.width && party.x + party.width > hint.x
          && party.y < hint.y + hint.height && party.y + party.height > hint.y),
        `${name}: Party leaves rendered native guidance clear`);
      }
      const observation = {name, viewport:page.viewportSize(), wizard:await state(page), party, guidance};
      receipt.observations.push(observation);
      await page.screenshot({path:path.join(output,`${name}.png`)});
      console.log(name, JSON.stringify({party,guidance}));
    }
    async function shortcut(name) {
      const button = await ui.byLabel(page, name, {role:'button',prefix:'hud:shortcut:'});
      await ui.click(page, button.id);
    }
    async function interact(name) {
      if (!touch) return shortcut(name);
      // Touch omits the desktop service shortcuts. These ordinary-input routes
      // approach the NPCs around the courtyard's authored collision geometry.
      const routes = {
        Provokatus:[[1110,530,7],[895.5,455.5,40]],
        Fomentius:[[1350,690,7],[1397,664,65]],
        Luthacus:[[1430,740,7],[1450,720,7],[1700.5,449.5,60]],
      };
      for (const [x,y,radius] of routes[name]) await walkTo(page,x,y,radius);
      await page.keyboard.press('KeyE');
    }
    assert.equal(await canvas.getAttribute('data-npc-walk-to-talk-visible'), 'true');
    await capture('fresh-guidance');
    // These corners walk around the authored College entrance and fountain wall.
    for (const [x,y] of [[1040,194],[1100,194],[1130,234],[1160,344],[1170,514],[1200,524],[1480,494]]) {
      await walkTo(page,x,y);
    }
    await capture('camera-overlap');
    for (const viewport of [{width:1024,height:768},{width:896,height:414},{width:414,height:896}]) {
      await page.setViewportSize(viewport);
      if (touch && viewport.height > viewport.width) {
        await ui.widget(page,'landscape-guidance');
        await page.screenshot({path:path.join(output,'portrait-orientation.png')});
        continue;
      }
      await capture(`size-${viewport.width}`);
      if (touch) {
        await ui.click(page,'social/party');
        await capture(`size-${viewport.width}-party-toggled`);
        await ui.click(page,'social/party');
      }
    }
    await page.setViewportSize({width:1600,height:900});
    await ui.noWidget(page,'landscape-guidance');
    await ui.widget(page,'hud:pause');
    if (ui.findWidget(await ui.readUi(page),'social/party-settings-shortcut')) {
      await ui.click(page,'social/party-settings-shortcut');
      await page.waitForFunction(() => document.querySelector('#game').dataset.npcGuidanceBounds === '[]');
      await page.screenshot({path:path.join(output,'party-settings.png')});
      await page.keyboard.press('Escape');
    }
    await ui.click(page,'hud:inventory');
    await page.waitForFunction(() => document.querySelector('#game').dataset.npcGuidanceBounds === '[]');
    await page.keyboard.press('Escape');
    await page.waitForFunction(() => document.querySelector('#game').dataset.npcWalkToTalkVisible === 'true');
    await interact('Provokatus');
    await ui.widget(page,'panel:dialogue');
    assert.deepEqual(await bounds(),[], 'dialogue suppresses onboarding');
    await page.keyboard.press('Escape');
    await ui.noWidget(page,'panel:dialogue');
    await page.waitForFunction(() => JSON.parse(document.querySelector('#game').dataset.npcGuidanceBounds).length === 2);
    let settled = false;
    let stableSamples = 0;
    let previous = await bounds();
    for (let sample=0; sample<100; sample++) {
      await page.waitForTimeout(100);
      const current = await bounds();
      const stable = current.length === 2 && current.every((hint,index) =>
        ['x','y','width','height'].every(field => Math.abs(hint[field] - previous[index][field]) < 0.01));
      stableSamples = stable ? stableSamples+1 : 0;
      previous = current;
      if (stableSamples >= 5) {settled=true;break;}
    }
    assert.ok(settled,'camera settles after the NPC approach');
    const phases = new Set();
    const firstBounds = await bounds();
    receipt.blinkSamples = [];
    for (let sample=0; sample<32; sample++) {
      const phase = await canvas.getAttribute('data-npc-directional-hint-count');
      phases.add(phase);
      const current = await bounds();
      receipt.blinkSamples.push({phase,bounds:current});
      assert.equal(current.length,2,'both arrows reserve space through blinking');
      for (let index=0;index<2;index++) for (const field of ['x','y','width','height']) {
        assert.ok(Math.abs(current[index][field] - firstBounds[index][field]) < 0.1,'blink does not move the reservation');
      }
      await page.waitForTimeout(80);
    }
    assert.deepEqual([...phases].sort(),['0','2']);
    await capture('both-directional-hints',2);
    for (const viewport of [{width:896,height:414},{width:414,height:896}]) {
      await page.setViewportSize(viewport);
      if (touch && viewport.height > viewport.width) {
        await ui.widget(page,'landscape-guidance');
        await page.screenshot({path:path.join(output,'hints-portrait-orientation.png')});
        continue;
      }
      await capture(`hints-size-${viewport.width}`,2);
      if (touch) {
        await ui.click(page,'social/party');
        await capture(`hints-size-${viewport.width}-party-toggled`,2);
        await ui.click(page,'social/party');
      }
    }
    await page.setViewportSize({width:1600,height:900});
    await ui.noWidget(page,'landscape-guidance');
    await ui.widget(page,'hud:pause');
    for (const [name,remaining] of [['Fomentius',1],['Luthacus',0]]) {
      await interact(name);
      const panel = touch ? 'panel:dialogue' : 'panel:service';
      await ui.widget(page,panel);
      assert.deepEqual(await bounds(),[]);
      await page.keyboard.press('Escape');
      await ui.noWidget(page,panel);
      await page.waitForFunction(count => JSON.parse(document.querySelector('#game').dataset.npcGuidanceBounds).length === count,remaining);
      await capture(`${name}-acknowledged`,remaining);
    }
    await page.keyboard.press('Escape');
    await ui.surface(page,'pause');
    await ui.click(page,'leave');
    await ui.surface(page,'title');
    await ui.click(page,'play');
    await ui.click(page,'continue');
    await ui.surface(page,'hidden');
    await ui.widget(page,'hud:pause');
    assert.deepEqual(await bounds(),[], 'saved Continue retains all three acknowledgements');
    await capture('continued');
    await page.keyboard.press('Escape');
    await ui.surface(page,'pause');
    await ui.click(page,'leave');
    await ui.surface(page,'title');
    await ui.settleErrors(page);
    assert.deepEqual(receipt.errors,[]);
    receipt.passed = true;
  } catch (error) {
    receipt.failure = error.stack;
    if (page) await page.screenshot({path:path.join(output,'failure.png')}).catch(()=>{});
    throw error;
  } finally {
    if (page) await ui.settleErrors(page);
    receipt.warnings = observed?.warnings;
    await fs.writeFile(path.join(output,'receipt.json'),JSON.stringify(receipt,null,2));
    await owned.close();
  }
})().catch(error => {console.error(error);process.exitCode=1;});
