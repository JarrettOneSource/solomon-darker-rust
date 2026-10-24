// Controller-only admission and modal handoffs, followed by real touch input.
const assert = require('node:assert/strict');
const path = require('node:path');
const fs = require('node:fs');
const {chromium} = require(path.resolve(process.env.PW));
const {leaveCollegeOffice,reachCollegeLoadout} = require('./menu-journey.cjs');
const ui = require('./gpu-ui.cjs');
const {launchOwnedBrowser}=require('./browser-lifecycle.cjs');
const url = process.env.URL || 'http://127.0.0.1:4185/';
const output = process.env.OUT || '/tmp';
fs.mkdirSync(output,{recursive:true});
const pad=ui.controller;

async function screen(page,name) {
  try {return await ui.surface(page,name,{timeout:20000})}
  catch(error) {
    const actual=await page.evaluate(()=>({gpu:JSON.parse(window.wasmBindings.gpu_ui_state()),
      pad:window.gpuTestPad,wizard:Array.from(window.wasmBindings.wizard_state()),
      input:Array.from(window.wasmBindings.gameplay_input_state())}));
    await page.screenshot({path:path.join(output,`menu-input-failure-${name}.png`)});
    throw new Error(`Expected ${name}: ${JSON.stringify(actual)}`,{cause:error});
  }
}

async function ready(page) {
  await page.goto(url,{waitUntil:'domcontentloaded'});
  await ui.ready(page);
  await ui.assertGpuOnly(page);
}

async function focusAction(page,id,button=13) {
  for(let presses=0;presses<16;presses++) {
    if(ui.findWidget(await ui.readUi(page),id)?.focused)return;
    await pad(page);
    await pad(page,[button]);
  }
  assert.equal(ui.findWidget(await ui.readUi(page),id)?.focused,true,`controller focuses ${id}`);
}

async function controllerCollegeLoadout(page) {
  await ui.widget(page,'panel:dialogue',{timeout:120000});
  await pad(page);
  await pad(page,[0]);
  await ui.widget(page,'panel:dialogue-choice:0');
  // Done retains focus after speech. Reach the real question using navigation.
  await focusAction(page,'panel:dialogue-choice:0',5);
  await pad(page);
  await pad(page,[0]);
  await ui.value(page,'panel:dialogue-key','ARCH_Q1_0');
  await pad(page);
  await pad(page,[1]);
  await ui.noWidget(page,'panel:dialogue');
  await pad(page);
  await leaveCollegeOffice(page,(x,y)=>pad(page,[],x,y));
}

async function settledCollege(page) {
  await page.waitForFunction(()=>window.wasmBindings.wizard_state()[6]===0
    &&window.wasmBindings.wizard_state()[7]===-1,null,{timeout:30000});
}

async function modalFocus(page) {
  const snapshot=await ui.readUi(page);
  const focused=snapshot.widgets.filter(widget=>widget.focused);
  assert.ok(focused.length>0&&focused.every(widget=>widget.modal),
    'the visible settings modal owns controller focus');
  assert.ok(snapshot.capture.keyboard,'settings focus seals world keyboard input');
}

(async()=>{
  const owned=await launchOwnedBrowser(chromium,{headless:true,
    executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
  const {browser}=owned;
  const errors=[];
  try {
    const page=await browser.newPage({viewport:{width:1600,height:900}});
    const desktopErrors=ui.watchErrors(page);errors.push(desktopErrors.errors);
    await desktopErrors.ready;
    await ui.observeGpu(page);
    await ready(page);
    await ui.installController(page);
    await pad(page);
    if(ui.findWidget(await ui.readUi(page),'prompt-secondary')) {
      await pad(page,[1]);
      await ui.noWidget(page,'prompt-secondary');
      await pad(page);
    }
    await focusAction(page,'settings');
    await pad(page);
    await pad(page,[0]);
    await screen(page,'settings');
    await pad(page);
    await modalFocus(page);
    await pad(page,[1]);
    await screen(page,'title');
    await pad(page);
    await focusAction(page,'play');
    await pad(page);
    await pad(page,[0]);
    await screen(page,'play');
    await page.waitForTimeout(400);
    await screen(page,'play');
    await pad(page);
    await pad(page,[0]);
    await screen(page,'hidden');
    await controllerCollegeLoadout(page);
    await screen(page,'create');
    assert.equal((await ui.widget(page,'create-back')).enabled,false);
    await pad(page,[1]);
    await screen(page,'create');
    await pad(page);
    await ui.widget(page,'element:earth');
    await pad(page,[0]);
    await ui.widget(page,'discipline:arcane');
    await pad(page);
    await pad(page,[0]);
    await screen(page,'hidden');
    await pad(page);
    await settledCollege(page);
    await pad(page,[9]);
    await screen(page,'pause');
    await pad(page);
    await pad(page,[13]);
    assert.equal(ui.findWidget(await ui.readUi(page),'pause-settings')?.focused,true);
    await pad(page);
    await pad(page,[0]);
    await screen(page,'settings');
    await pad(page);
    await modalFocus(page);
    await pad(page,[1]);
    await screen(page,'pause');
    await pad(page);
    await pad(page,[1]);
    await screen(page,'hidden');
    await ui.assertGpuOnly(page);
    await page.screenshot({path:path.join(output,'menu-controller-game.png')});
    await ui.settleErrors(page);
    assert.deepEqual(desktopErrors.errors,[]);
    await page.close();

    const touch=await browser.newPage({viewport:{width:390,height:844},hasTouch:true,isMobile:true});
    const touchErrors=ui.watchErrors(touch);errors.push(touchErrors.errors);
    await touchErrors.ready;
    await ready(touch);
    assert.match((await ui.widget(touch,'landscape-guidance')).label,/Rotate your device to landscape/);
    await touch.screenshot({path:path.join(output,'menu-touch-portrait-guidance.png')});
    await touch.setViewportSize({width:844,height:390});
    await ui.noWidget(touch,'landscape-guidance');
    if(ui.findWidget(await ui.readUi(touch),'prompt-secondary'))await ui.tap(touch,'prompt-secondary');
    await touch.screenshot({path:path.join(output,'menu-touch-title.png')});
    await ui.tap(touch,'play');
    await ui.tap(touch,'new-game');
    await screen(touch,'hidden');
    await reachCollegeLoadout(touch,{touch:true});
    await ui.fill(touch,'create-name','Felix',{touch:true});
    await ui.value(touch,'create-name','Felix');
    await ui.widget(touch,'element:water');
    const element=ui.bounds(await ui.widget(touch,'element:air'));
    assert.ok(element.width>=43.99&&element.height>=43.99,'element art has a real finger-sized target');
    await touch.screenshot({path:path.join(output,'menu-touch-create.png')});
    await ui.tap(touch,'element:water');
    await ui.tap(touch,'discipline:body');
    await screen(touch,'hidden');
    await settledCollege(touch);
    await ui.tap(touch,'hud:pause');
    await screen(touch,'pause');
    await touch.waitForTimeout(350);
    const resume=ui.bounds(await ui.widget(touch,'resume'));
    assert.ok(resume.height>=44,'pause rows remain usable on a phone');
    await touch.screenshot({path:path.join(output,'menu-touch-pause.png')});
    await ui.tap(touch,'resume');
    await screen(touch,'hidden');
    await ui.assertGpuOnly(touch);
    await ui.settleErrors(touch);
    assert.deepEqual(touchErrors.errors,[]);
    console.log(JSON.stringify({checks:['controller College admission and sole loadout',
      'locked loadout Back','neutral handoff','controller pause','spatial focus',
      'modal focus ownership','source portrait guidance','landscape phone title/admission/Create',
      'phone pause targets','actual GPU UI'],errors:errors.flat()}));
  } finally {
    try {
      fs.writeFileSync(path.join(output,'browser-cleanup.json'),JSON.stringify(await owned.close(),null,2));
    } catch(error) {
      fs.writeFileSync(path.join(output,'browser-cleanup-failure.json'),JSON.stringify(error.cleanup||{error:error.stack},null,2));
      throw error;
    }
  }
})().catch(error=>{console.error(error);process.exitCode=1});
