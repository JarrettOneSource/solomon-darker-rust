// Real GPU controls and device-store receipts. No layout or controller state is injected.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(process.env.PW || '/Users/jarrett/Projects/Solomon Dark/Website/frontend/node_modules/playwright-core');
const ui = require('./gpu-ui.cjs');
const out = process.env.OUT || '/tmp';
const url = process.env.URL || 'https://127.0.0.1:9449/game-stg/';
const observations = [];

async function openEditor(page, touch = false) {
  await page.goto(url, {waitUntil:'load'});
  await ui.ready(page);
  const activate = touch ? ui.tap : ui.click;
  if (ui.findWidget(await ui.readUi(page), 'prompt-secondary')) await activate(page, 'prompt-secondary');
  await activate(page, 'settings');
  await activate(page, 'settings-page:mobile-ui');
  await ui.widget(page, 'mobile/editor');
  await ui.assertGpuOnly(page);
}
async function storage(page) {
  return page.evaluate(() => Object.fromEntries(Object.entries(localStorage)
    .filter(([key]) => key.includes('mobile-ui-layout'))));
}
async function transform(page, id) {
  const value = (await ui.widget(page, `mobile/control:${id}`)).value;
  const [x, y, scale, rotation] = value.split(',').map(Number);
  assert.ok([x, y, scale, rotation].every(Number.isFinite));
  return {x, y, scale, rotation};
}
async function changed(page, id, predicate, description) {
  return ui.waitForUi(page, snapshot => {
    const value = ui.findWidget(snapshot, `mobile/control:${id}`)?.value;
    return value && predicate(value.split(',').map(Number));
  }, description);
}
async function dragBy(page, id, dx, dy) {
  const point = ui.center(ui.bounds(await ui.widget(page, id)));
  await page.mouse.move(point.x, point.y); await page.mouse.down();
  await page.mouse.move(point.x + dx, point.y + dy, {steps:8}); await page.mouse.up();
  await ui.nextFrame(page);
}
async function saved(page) {
  await ui.noWidget(page, 'mobile/editor');
  await ui.widget(page, 'settings-page:mobile-ui');
}

(async () => {
  fs.mkdirSync(out, {recursive:true});
  const server = await chromium.launchServer({headless:true,host:'127.0.0.1',
    executablePath:process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist']});
  const browser = await chromium.connect(server.wsEndpoint());
  let current;
  try {
    const desktop = await browser.newContext({viewport:{width:1600,height:900},ignoreHTTPSErrors:true});
    const page = await desktop.newPage(); current = page;
    observations.push(ui.watchErrors(page));
    await observations.at(-1).ready; await openEditor(page);
    const initialUi = await ui.readUi(page);
    assert.equal(initialUi.widgets.filter(widget => widget.id.startsWith('mobile/control:')).length, 18);
    const before = await storage(page);
    await page.screenshot({path:path.join(out, 'mobile-editor-desktop-initial.png')});
    const initial = await transform(page, 'pause');
    await dragBy(page, 'mobile/control:pause', 90, 60);
    await changed(page, 'pause', ([x]) => x !== initial.x, 'desktop draft movement');
    const moved = await transform(page, 'pause');
    assert.deepEqual(await storage(page), before, 'drag changes the draft until Save');
    await ui.click(page, 'mobile/undo');
    await changed(page, 'pause', ([x,y]) => x === initial.x && y === initial.y, 'undo restores movement');
    await ui.click(page, 'mobile/redo');
    await changed(page, 'pause', ([x,y]) => x === moved.x && y === moved.y, 'redo restores movement');
    await ui.click(page, 'mobile/control:pause');
    await ui.focused(page, 'mobile/control:pause');
    const beforeNudge = await transform(page, 'pause');
    await page.keyboard.press('ArrowRight');
    await changed(page, 'pause', ([x]) => x > beforeNudge.x, 'focused native keyboard nudge');
    await ui.focused(page, 'mobile/control:pause');
    await page.keyboard.press(']');
    await changed(page, 'pause', ([,,,rotation]) => rotation === 5, 'keyboard five-degree rotation');
    const beforeResize = await transform(page, 'pause');
    await dragBy(page, 'mobile/resize:pause:east', 25, 0);
    await changed(page, 'pause', ([,,scale]) => scale !== beforeResize.scale, 'dragged resize handle');
    await page.screenshot({path:path.join(out, 'mobile-editor-desktop-edited.png')});
    await ui.click(page, 'settings-back'); await saved(page);
    assert.notDeepEqual(await storage(page), before, 'Save commits the device-store write');
    await ui.click(page, 'settings-page:mobile-ui');
    await ui.waitForUi(page, snapshot => ui.findWidget(snapshot, 'mobile/editor')?.selected === true,
      'saved customized layout reopens');
    await ui.click(page, 'mobile/reset-layout');
    await ui.waitForUi(page, snapshot => ui.findWidget(snapshot, 'mobile/editor')?.selected === false,
      'reset is a default-layout draft');
    await page.keyboard.press('Escape'); await saved(page);
    assert.deepEqual(await storage(page), before, 'Escape saves reset through the same store');
    await ui.assertGpuOnly(page); await ui.settleErrors(page); await desktop.close();

    const touch = await browser.newContext({viewport:{width:896,height:414},isMobile:true,
      hasTouch:true,deviceScaleFactor:1,ignoreHTTPSErrors:true});
    const phone = await touch.newPage(); current = phone;
    observations.push(ui.watchErrors(phone));
    await observations.at(-1).ready; await openEditor(phone, true);
    const root = ui.bounds(await ui.widget(phone, 'mobile/editor'));
    assert.deepEqual(root, {x:0,y:0,width:896,height:414}, 'touch editor fills the actual canvas');
    const initialJoystick = await transform(phone, 'leftJoystick');
    const box = ui.bounds(await ui.widget(phone, 'mobile/control:leftJoystick'));
    const {x,y} = ui.center(box); const client = await touch.newCDPSession(phone);
    const input = (type, points) => client.send('Input.dispatchTouchEvent', {type,
      touchPoints:points.map(([id,x,y]) => ({id,x,y}))});
    try {
      await input('touchStart', [[1,x,y]]);
      await input('touchMove', [[1,x+80,y-55]]); await input('touchEnd', []);
      await changed(phone, 'leftJoystick', ([x,y]) => x !== initialJoystick.x || y !== initialJoystick.y,
        'touch drag updates the joystick draft');
      const shifted = ui.center(ui.bounds(await ui.widget(phone, 'mobile/control:leftJoystick')));
      const beforePinch = await transform(phone, 'leftJoystick');
      await input('touchStart', [[2,shifted.x-12,shifted.y],[3,shifted.x+12,shifted.y]]);
      await input('touchMove', [[2,shifted.x-22,shifted.y],[3,shifted.x+22,shifted.y]]);
      await input('touchEnd', []);
      await changed(phone, 'leftJoystick', ([,,scale]) => scale !== beforePinch.scale,
        'two real touch pointers resize the same joystick');
    } finally {await input('touchEnd', []); await client.detach();}
    await ui.tap(phone, 'mobile/tools'); await ui.tap(phone, 'mobile/rotate-right');
    await changed(phone, 'leftJoystick', ([,,,rotation]) => rotation === 15,
      'touch inspector rotation retains selected control');
    const savedJoystick = await transform(phone, 'leftJoystick');
    await phone.screenshot({path:path.join(out, 'mobile-editor-touch-edited.png')});
    await ui.tap(phone, 'mobile/save'); await saved(phone);
    assert.ok(Object.keys(await storage(phone)).length > 0);
    await ui.tap(phone, 'settings-page:mobile-ui');
    await ui.waitForUi(phone, snapshot => ui.findWidget(snapshot, 'mobile/editor')?.selected === true,
      'saved touch layout is customized');
    assert.deepEqual(await transform(phone, 'leftJoystick'), savedJoystick,
      'reopen retains the exact saved joystick transform');
    await ui.assertGpuOnly(phone);
    await phone.screenshot({path:path.join(out, 'mobile-editor-touch-restored.png')});
    await ui.settleErrors(phone); await touch.close(); current = null;
    const errors = observations.flatMap(observation => observation.errors);
    assert.deepEqual(errors, []);
    const result = {checks:['18 real GPU controls','desktop drag/history/key rotation/resize',
      'draft isolated from storage','Save and Escape reset receipts',
      'touch fullscreen/drag/pinch/rotation','saved touch layout restored'],errors};
    fs.writeFileSync(path.join(out, 'mobile-editor-results.json'), JSON.stringify(result,null,2));
    console.log(JSON.stringify(result));
  } catch (error) {
    if (current) {
      await ui.settleErrors(current);
      await current.screenshot({path:path.join(out, 'mobile-editor-failure.png')});
      fs.writeFileSync(path.join(out, 'mobile-editor-failure.json'), JSON.stringify({error:String(error),
        observations,ui:await ui.readUi(current)}, null, 2));
    }
    throw error;
  } finally {
    let timer;
    try {
      await Promise.race([browser.close(), new Promise((_, reject) => {
        timer = setTimeout(() => reject(new Error('Playwright disconnect timed out')), 5000);
      })]);
      clearTimeout(timer);
      await Promise.race([server.close(), new Promise((_, reject) => {
        timer = setTimeout(() => reject(new Error('Chrome close timed out')), 15000);
      })]);
    } catch {
      clearTimeout(timer);
      await Promise.race([server.kill(), new Promise((_, reject) => {
        timer = setTimeout(() => reject(new Error('Chrome termination timed out')), 5000);
      })]);
    } finally {clearTimeout(timer);}
  }
})().catch(error => {console.error(error); process.exitCode = 1;});
