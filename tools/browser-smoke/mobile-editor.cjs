// Real device-store edits through source desktop/touch gestures. No gameplay or
// layout fields are injected; draft cancellation/history and the save receipt are observed.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(process.env.PW || '/Users/jarrett/Projects/Solomon Dark/Website/frontend/node_modules/playwright-core');
const out = process.env.OUT || '/tmp';
const url = process.env.URL || 'https://127.0.0.1:9449/game-stg/';
const errors = [], failedRequests = [];

async function openEditor(page) {
  await page.goto(url, {waitUntil:'load'});
  await page.waitForFunction(() => window.wasmBindings?.wizard_state().length > 0 && document.getElementById('status').textContent === '', null, {timeout:60000});
  await page.getByRole('button', {name:'Settings', exact:true}).waitFor({timeout:60000});
  const decline = page.locator('[data-menu-action="prompt-secondary"]:visible');
  if (await decline.isVisible()) await decline.click();
  await page.getByRole('button', {name:'Settings', exact:true}).click();
  await page.getByRole('button', {name:'CUSTOMIZE MOBILE UI', exact:true}).click();
  await page.locator('.mobile-ui-editor').waitFor();
  await page.evaluate(() => document.fonts.ready);
}

function recordErrors(page) {
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => {if(message.type()==='error')errors.push(message.text());});
  page.on('response', response => {if(response.status()>=400)failedRequests.push({url:response.url(), status:response.status()});});
}
async function layoutStorage(page) {
  return page.evaluate(() => Object.fromEntries(Object.entries(localStorage).filter(([key]) => key.includes('mobile-ui-layout'))));
}
async function transform(node) {
  return node.evaluate(node => ({x:parseFloat(node.style.left), y:parseFloat(node.style.top), transform:node.style.transform}));
}
async function drag(page, node, delta) {
  const box = await node.boundingBox(); assert.ok(box);
  const x=box.x+box.width/2, y=box.y+box.height/2;
  await page.mouse.move(x,y); await page.mouse.down();
  await page.mouse.move(x+delta.x,y+delta.y,{steps:8}); await page.mouse.up();
}

(async () => {
  fs.mkdirSync(out,{recursive:true});
  const browser = await chromium.launch({headless:true,executablePath:process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
  let current;
  try {
    const desktop = await browser.newContext({viewport:{width:1600,height:900},ignoreHTTPSErrors:true});
    const page=await desktop.newPage();current=page;recordErrors(page);await openEditor(page);
    const editor=page.locator('.mobile-ui-editor');
    assert.equal(await editor.getAttribute('data-editor-presentation'),'windowed');
    assert.equal(await editor.locator('[data-mobile-ui-editor-element]').count(),18);
    const before=await layoutStorage(page);
    await page.screenshot({path:path.join(out,'mobile-editor-desktop-initial.png')});
    const pause=editor.locator('[data-mobile-ui-editor-element="pause"]');
    const initial=await transform(pause);
    await drag(page,pause,{x:90,y:60});
    await page.waitForFunction(initialX => parseFloat(document.querySelector('[data-mobile-ui-editor-element="pause"]').style.left)!==initialX,initial.x);
    const moved=await transform(pause);
    assert.deepEqual(await layoutStorage(page),before,'a gesture only changes the draft');
    await editor.getByRole('button',{name:'UNDO',exact:true}).click();
    await page.waitForFunction(initialX => parseFloat(document.querySelector('[data-mobile-ui-editor-element="pause"]').style.left)===initialX,initial.x);
    await editor.getByRole('button',{name:'REDO',exact:true}).click();
    await page.waitForFunction(movedX => parseFloat(document.querySelector('[data-mobile-ui-editor-element="pause"]').style.left)===movedX,moved.x);
    await pause.focus();await page.keyboard.press(']');
    await page.waitForFunction(() => document.querySelector('[data-mobile-ui-editor-element="pause"]').style.transform.includes('rotate(5deg)'));
    const east=pause.locator('[data-resize-handle="east"]');
    await drag(page,east,{x:25,y:0});
    await page.screenshot({path:path.join(out,'mobile-editor-desktop-edited.png')});
    await page.locator('[data-menu-action="settings-back"]').click();
    await page.locator('[data-settings-page="root"]').waitFor();
    assert.notDeepEqual(await layoutStorage(page),before,'Save waits for the actual device-store write');
    await page.getByRole('button',{name:'CUSTOMIZE MOBILE UI',exact:true}).click();
    await page.waitForFunction(() => document.querySelector('.mobile-ui-editor')?.dataset.restoringDefault==='false');
    await editor.getByRole('button',{name:'RESET LAYOUT',exact:true}).click();
    await page.waitForFunction(() => document.querySelector('.mobile-ui-editor')?.dataset.restoringDefault==='true');
    await page.keyboard.press('Escape');
    await page.locator('[data-settings-page="root"]').waitFor();
    assert.deepEqual(await layoutStorage(page),before,'Escape commits reset through the same store');
    await desktop.close();

    const touch=await browser.newContext({viewport:{width:896,height:414},isMobile:true,hasTouch:true,deviceScaleFactor:1,ignoreHTTPSErrors:true});
    const phone=await touch.newPage();current=phone;recordErrors(phone);await openEditor(phone);
    const mobile=phone.locator('.mobile-ui-editor');
    assert.equal(await mobile.getAttribute('data-editor-presentation'),'fullscreen');
    const joystick=mobile.locator('[data-mobile-ui-editor-element="leftJoystick"]');
    const box=await joystick.boundingBox();assert.ok(box);
    const client=await touch.newCDPSession(phone);
    const x=box.x+box.width/2,y=box.y+box.height/2;
    const touchAt=async(type,points)=>client.send('Input.dispatchTouchEvent',{type,touchPoints:points.map(([id,x,y])=>({id,x,y}))});
    const original=await transform(joystick);
    await touchAt('touchStart',[[1,x,y]]);await touchAt('touchMove',[[1,x+80,y-55]]);await touchAt('touchEnd',[]);
    await phone.waitForFunction(initialX => parseFloat(document.querySelector('[data-mobile-ui-editor-element="leftJoystick"]').style.left)!==initialX,original.x);
    const shifted=await joystick.boundingBox();const cx=shifted.x+shifted.width/2,cy=shifted.y+shifted.height/2;
    await touchAt('touchStart',[[2,cx-12,cy],[3,cx+12,cy]]);
    await touchAt('touchMove',[[2,cx-22,cy],[3,cx+22,cy]]);
    await touchAt('touchEnd',[]);
    await phone.waitForFunction(() => !document.querySelector('[data-mobile-ui-editor-element="leftJoystick"]').style.transform.endsWith('scale(1)'));
    await mobile.getByRole('button',{name:'ADJUST +',exact:true}).tap();
    await mobile.getByRole('button',{name:'Rotate control right',exact:true}).tap();
    await phone.screenshot({path:path.join(out,'mobile-editor-touch-edited.png')});
    await phone.getByRole('button',{name:'SAVE',exact:true}).tap();
    await phone.locator('[data-settings-page="root"]').waitFor();
    assert.ok(Object.keys(await layoutStorage(phone)).length>0);
    await phone.getByRole('button',{name:'CUSTOMIZE MOBILE UI',exact:true}).tap();
    await phone.waitForFunction(() => document.querySelector('.mobile-ui-editor')?.dataset.restoringDefault==='false');
    await phone.screenshot({path:path.join(out,'mobile-editor-touch-restored.png')});
    await touch.close();current=null;
    assert.deepEqual(errors,[]);assert.deepEqual(failedRequests,[]);
    const result={checks:['18 actual controls','desktop drag/history/key rotation/resize','draft isolated from storage','Save and Escape reset receipts','touch fullscreen/drag/pinch/rotation','saved touch layout restored'],errors,failedRequests};
    fs.writeFileSync(path.join(out,'mobile-editor-results.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
  } catch(error) {
    if(current){await current.screenshot({path:path.join(out,'mobile-editor-failure.png')});fs.writeFileSync(path.join(out,'mobile-editor-failure.json'),JSON.stringify({error:String(error),errors,failedRequests,body:await current.locator('body').innerText()},null,2));}
    throw error;
  } finally {await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
