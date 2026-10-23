// Stock HUD geometry and production stick, keyboard, focus and touch input paths.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {chromium}=require(path.resolve(process.env.PW));
const {startNewGame}=require('./menu-journey.cjs');
const ui=require('./gpu-ui.cjs');
const url=process.env.URL||'http://127.0.0.1:4185/';
const output=process.env.OUT||'/tmp';
fs.mkdirSync(output,{recursive:true});

async function ready(page,{touch=false}={}) {
  await page.goto(url,{waitUntil:'domcontentloaded'});
  await ui.ready(page);
  if(touch) {
    assert.match((await ui.widget(page,'landscape-guidance')).label,/Rotate your device to landscape/);
    await screenshot(page,'hud-touch-before-admission-portrait-guidance.png');
    await page.setViewportSize({width:844,height:390});
    await ui.noWidget(page,'landscape-guidance');
  }
  await startNewGame(page,{name:null,touch});
  await ui.widget(page,'hud:health');
}

const state=page=>page.evaluate(()=>Array.from(window.wasmBindings.wizard_state()));
async function ticks(page,count) {
  const start=(await state(page))[5];
  await page.waitForFunction(target=>window.wasmBindings.wizard_state()[5]>=target,
    start+count,{timeout:20000});
}
const screenshot=(page,name)=>page.screenshot({path:path.join(output,name)});

(async()=>{
  const browser=await chromium.launch({headless:true,
    executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
  const errors=[];
  try {
    const desktopContext=await browser.newContext({viewport:{width:1600,height:900}});
    const desktop=await desktopContext.newPage();
    await ui.watchErrors(desktop,{errors}).ready;
    await ui.observeGpu(desktop);
    await ready(desktop);
    const bounds=[];
    for(const id of ['hud:health','hud:mana','hud:inventory','hud:skills']) {
      bounds.push((await ui.widget(desktop,id)).rect);
    }
    assert.deepEqual(bounds,[[640,14.5,110,20],[850,14.5,110,20],
      [730.5,825,58,62],[810.5,825,58,62]]);
    await ui.assertGpuOnly(desktop);
    await screenshot(desktop,'hud-desktop.png');
    await desktop.setViewportSize({width:1920,height:1080});
    await ticks(desktop,3);
    assert.equal(ui.bounds(await ui.widget(desktop,'hud:inventory')).y,1005);
    await screenshot(desktop,'hud-wide.png');

    // Native prototype-backed samples traverse the shipped navigator adapter.
    await ui.installController(desktop);
    const beforePad=await state(desktop);
    await ui.controller(desktop,[],0,1);
    await ticks(desktop,70);
    const afterPad=await state(desktop);
    assert.ok(afterPad[1]>beforePad[1]+25,'the left stick walks south');
    await ui.controller(desktop,[15],-1,0);
    await ticks(desktop,70);
    const afterDpad=await state(desktop);
    assert.ok(afterDpad[0]>afterPad[0]+25,'d-pad right wins over stick left');
    await ui.controller(desktop,[],0.1,0);
    await ticks(desktop,40);
    const driftStart=await state(desktop);
    await ticks(desktop,40);
    const driftEnd=await state(desktop);
    assert.ok(Math.hypot(driftEnd[0]-driftStart[0],driftEnd[1]-driftStart[1])<0.1,
      'stick drift stops');
    await desktop.evaluate(()=>{window.gpuTestPad.connected=false});
    await desktop.keyboard.down('KeyD');
    await ticks(desktop,30);
    // A real tab activation produces the browser blur; no synthetic blur event is sent.
    const focusTarget=await desktop.context().newPage();
    await focusTarget.goto('about:blank');
    await ui.useNativeFocus(desktop,focusTarget);
    await focusTarget.bringToFront();
    await desktop.waitForFunction(()=>!document.hasFocus());
    assert.ok(await desktop.evaluate(()=>__solomonFocusEvents.some(event=>event.type==='blur'&&event.trusted)),
      'the actual tab switch emits Chrome\'s trusted blur event');
    await ticks(desktop,40);
    const blurStart=await state(desktop);
    await ticks(desktop,40);
    const blurEnd=await state(desktop);
    assert.ok(Math.hypot(blurEnd[0]-blurStart[0],blurEnd[1]-blurStart[1])<0.1,
      'actual browser blur releases held keys');
    await desktop.bringToFront();
    await desktop.keyboard.up('KeyD');
    await focusTarget.close();
    await screenshot(desktop,'hud-input-desktop-final.png');
    const desktopSubmissions=await ui.gpuDraws(desktop);
    await ui.settleErrors(desktop);
    await desktopContext.close();

    const touch=await browser.newPage({viewport:{width:390,height:844},hasTouch:true,isMobile:true});
    await ui.watchErrors(touch,{errors}).ready;
    await ui.observeGpu(touch);
    await ready(touch,{touch:true});
    await touch.setViewportSize({width:390,height:844});
    await ui.widget(touch,'landscape-guidance');
    await screenshot(touch,'hud-touch-portrait-guidance.png');
    await touch.setViewportSize({width:844,height:390});
    await ui.noWidget(touch,'landscape-guidance');
    await ticks(touch,3);
    await screenshot(touch,'hud-touch-landscape.png');
    const joystick=ui.bounds(await ui.widget(touch,'hud:joystick:movement'));
    assert.ok(joystick.width>=90,'the actual touch joystick remains usable');
    const client=await touch.context().newCDPSession(touch);
    try {
      const point={x:joystick.x+joystick.width*0.8,y:joystick.y+joystick.height*0.7,id:1};
      const beforeTouch=await state(touch);
      await client.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[point]});
      await ticks(touch,70);
      const afterTouch=await state(touch);
      assert.ok(afterTouch[0]>beforeTouch[0]+20,'touch walks east');
      assert.equal((await ui.widget(touch,'hud:joystick:movement')).selected,true,
        'the GPU joystick shows the active touch');
      assert.equal((await ui.widget(touch,'hud:joystick:movement')).focused,false,
        'the source drag-only joystick does not take keyboard focus');
      await client.send('Input.dispatchTouchEvent',{type:'touchCancel',touchPoints:[]});
      await ticks(touch,40);
      const cancelStart=await state(touch);
      await ticks(touch,40);
      const cancelEnd=await state(touch);
      assert.ok(Math.hypot(cancelEnd[0]-cancelStart[0],cancelEnd[1]-cancelStart[1])<0.1,
        'real touch cancellation stops movement');
      assert.equal((await ui.widget(touch,'hud:joystick:movement')).selected,false,
        'the GPU joystick returns to its neutral presentation');
      assert.equal((await ui.widget(touch,'hud:joystick:movement')).focused,false,
        'touch cancellation leaves keyboard focus with the ordinary owner');
      const movement=await touch.evaluate(()=>Array.from(wasmBindings.gameplay_input_state()).slice(0,2));
      assert.deepEqual(movement,[0,0],'cancel clears the accepted movement input');
      await ui.assertGpuOnly(touch);
      await screenshot(touch,'hud-input-touch-final.png');
      await ui.settleErrors(touch);
      assert.deepEqual(errors,[]);
      console.log(JSON.stringify({checks:['native GPU HUD bounds','wide layout','gamepad stick',
        'd-pad priority','stick drift','actual tab blur release','source portrait guidance','landscape touch movement',
        'touch cancellation and GPU neutral state'],desktopSubmissions,
        touchSubmissions:await ui.gpuDraws(touch),errors}));
    } finally {
      await client.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});
      await client.detach();
    }
  } finally{await browser.close()}
})().catch(error=>{console.error(error);process.exitCode=1});
