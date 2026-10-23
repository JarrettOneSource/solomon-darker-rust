// Actual browser listeners and the public sampled input seam, using a fresh local wizard.
const assert=require('node:assert/strict');
const path=require('node:path');
const {chromium}=require(path.resolve(process.env.PW));
const {startNewGame}=require('./menu-journey.cjs');
const fs=require('node:fs');
const ui=require('./gpu-ui.cjs');
const url=process.env.URL||'http://127.0.0.1:4185/';
const input=page=>page.evaluate(()=>Array.from(window.wasmBindings.gameplay_input_state()));
async function frames(page,count=3){await page.evaluate(async count=>{for(let i=0;i<count;i++)await new Promise(requestAnimationFrame)},count);}
async function pad(page,buttons=[],axes=[0,0,0,0],analog={}){
  await ui.controller(page,buttons,axes[0],axes[1],{rightX:axes[2],rightY:axes[3],analog});
  await frames(page);
}
(async()=>{
  const browser=await chromium.launch({headless:true,
    executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
  const errors=[];
  try{
    const context=await browser.newContext({viewport:{width:1600,height:900}});
    const page=await context.newPage();
    ui.watchErrors(page,{errors});
    await ui.observeGpu(page);
    await page.goto(url,{waitUntil:'domcontentloaded'});
    await ui.ready(page);
    await startNewGame(page,{name:null});
    await page.waitForFunction(()=>window.wasmBindings.gameplay_input_state().length===11);
    await page.mouse.move(1160,270);await frames(page);
    assert.equal((await input(page))[2],0);
    await page.mouse.down({button:'right'});await frames(page);
    let sample=await input(page);
    assert.equal(sample[3],0,'Mouse2 holds native belt zero');
    assert.equal(sample[4],1,'held desktop casts request pointer projection');
    assert.equal(sample[5],.725);assert.equal(sample[6],.3);
    await page.mouse.up({button:'right'});await frames(page);
    const released=await input(page);
    assert.equal(released[4],4,'released aim requests the retained world point');
    await page.keyboard.down('KeyS');await frames(page,10);await page.keyboard.up('KeyS');await frames(page);
    sample=await input(page);
    assert.deepEqual(sample.slice(9,11),released.slice(9,11),'a released pointer target does not drift with the camera');
    await page.mouse.down();await frames(page);
    assert.equal((await input(page))[2],0,'the College disables primary attacks');
    await page.mouse.up();await frames(page);
    const beforePotion=(await input(page))[8];
    await page.keyboard.down('Digit3');await frames(page,12);
    sample=await input(page);
    assert.equal(sample[3],3);assert.equal(sample[8],beforePotion+1,'holding a potion queues one activation');
    assert.equal((await ui.widget(page,'hud:belt:3')).enabled,true,'the consumed potion alias remains removable through the source belt gesture');
    await page.keyboard.up('Digit3');await frames(page);
    const manaBounds=ui.bounds(await ui.widget(page,'hud:belt:4'));
    const beforeTap=(await input(page))[8];
    await page.mouse.move(manaBounds.x+manaBounds.width/2,manaBounds.y+manaBounds.height/2);
    await page.mouse.down();await frames(page);
    assert.equal((await input(page))[8],beforeTap,'a belt GUI press waits for its release');
    assert.equal((await ui.widget(page,'hud:belt:4')).enabled,true);
    await page.mouse.up();await frames(page);
    assert.equal((await input(page))[8],beforeTap+1);
    assert.equal((await ui.widget(page,'hud:belt:4')).enabled,true,'the consumed mana alias remains removable');
    await page.keyboard.down('Digit1');await frames(page);
    await page.mouse.move(1160,270);
    await page.mouse.down({button:'right'});await frames(page);
    assert.equal((await input(page))[3],0,'the latest input source owns the held belt');
    await page.mouse.up({button:'right'});await frames(page);
    assert.equal((await input(page))[3],1,'releasing one source restores the still-held keyboard source');
    const focusTarget=await page.context().newPage();
    await focusTarget.goto('about:blank');
    await ui.useNativeFocus(page,focusTarget);
    await focusTarget.bringToFront();
    await page.waitForFunction(()=>!document.hasFocus());
    await frames(page);
    assert.ok(await page.evaluate(()=>__solomonFocusEvents.some(event=>event.type==='blur'&&event.trusted)),
      'Chrome emits a real trusted blur event for the actual tab switch');
    sample=await input(page);assert.equal(sample[3],-1);assert.equal(sample[4],0);
    await page.bringToFront();
    await page.keyboard.up('Digit1');
    await focusTarget.close();
    console.log('mouse, keyboard, potion edge and blur passed');
    await ui.installController(page);
    await frames(page);
    const reads=await page.evaluate(async()=>{
      const before=window.gpuTestPad.reads;
      for(let i=0;i<8;i++)await new Promise(requestAnimationFrame);
      return window.gpuTestPad.reads-before;
    });
    assert.equal(reads,8,'gameplay reads gamepads once per animation frame');
    await pad(page,[],[0,0,.8,-.6],{7:.7});
    sample=await input(page);assert.equal(sample[4],2);assert.equal(sample[5],.8);assert.equal(sample[6],-.6);assert.equal(sample[2],0);
    await pad(page,[5]);assert.equal((await input(page))[7],1);
    assert.equal((await ui.widget(page,'hud:belt:1')).label,'Empty belt slot, controller selected',
      'the actual selected empty belt widget remains painted');
    await pad(page,[2]);assert.equal((await input(page))[3],1);
    await pad(page,[2,5]);assert.equal((await input(page))[7],1,'the selected belt slot stays fixed during a held cast');
    await pad(page,[]);
    await pad(page,[8]);
    await ui.widget(page,'panel:surface:inventory');
    await pad(page,[2,7],[1,0,1,0]);
    sample=await input(page);assert.equal(sample[2],0);assert.equal(sample[3],-1);assert.equal(sample[0],0);
    await page.keyboard.press('Escape');await ui.noWidget(page,'panel:surface:inventory');await frames(page);
    sample=await input(page);assert.equal(sample[3],-1);assert.equal(sample[0],0,'held controls require a neutral handoff after the panel');
    await pad(page,[]);await pad(page,[],[0,0,1,0]);
    assert.equal((await input(page))[4],2);
    await pad(page,[]);await pad(page,[9]);
    await ui.surface(page,'pause');
    await ui.assertGpuOnly(page);
    if(process.env.OUT)fs.mkdirSync(process.env.OUT,{recursive:true});
    if(process.env.OUT)await page.screenshot({path:path.join(process.env.OUT,'gameplay-input-pause.png')});
    const receipt={passed:errors.length===0,errors,gamepadReads:reads,gamepadFrames:8,
      gamepadReadsPerFrame:reads/8,selection:(await ui.readUi(page)).acceptedSelection,
      focusEvents:await page.evaluate(()=>__solomonFocusEvents),submissions:await ui.gpuDraws(page),
      primaryContext:'College-disabled; live Boneyard acceptance follows coordinator integration'};
    if(process.env.OUT)fs.writeFileSync(path.join(process.env.OUT,'gameplay-input.json'),JSON.stringify(receipt,null,2));
    console.log(JSON.stringify(receipt));
    assert.deepEqual(errors,[]);
  }finally{await browser.close()}
})().catch(error=>{console.error(error);process.exitCode=1});
