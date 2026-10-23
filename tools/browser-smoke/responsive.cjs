// Real touch gestures exercise the GPU inventory and ordinary College services.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {chromium}=require(path.resolve(process.env.PW));
const {startNewGame}=require('./menu-journey.cjs');
const ui=require('./gpu-ui.cjs');
const {launchOwnedBrowser}=require('./browser-lifecycle.cjs');
const output=process.env.OUT||'/tmp';
const url=process.env.URL||'http://127.0.0.1:4185/';
fs.mkdirSync(output,{recursive:true});
const action=(page,name)=>ui.tap(page,`panel:${name}`);
const settle=page=>page.waitForTimeout(450);
const shot=(page,name)=>page.screenshot({path:path.join(output,name)});
const item=(page,label)=>ui.byLabel(page,label,{prefix:'panel:item:',role:'button'});

function fitsViewport(bounds,width,height) {
  return bounds.x>=0&&bounds.y>=0&&bounds.x+bounds.width<=width
    &&bounds.y+bounds.height<=height;
}

// The pinned courtyard path generated these clear corners. The production touch
// joystick moves the wizard; its accepted position is only observed, never changed.
async function walkToFomentius(page) {
  const session=await page.context().newCDPSession(page);
  const box=ui.bounds(await ui.widget(page,'hud:joystick:movement'));
  const center=ui.center(box);
  const route=[[1050,200],[1090,190],[1120,210],[1160,370],[1170,520],[1360,730]];
  await session.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{...center,id:1}]});
  try {
    for(const [targetX,targetY]of route) {
      const deadline=Date.now()+30000;
      let arrived=false;
      while(Date.now()<deadline) {
        const [x,y]=await page.evaluate(()=>Array.from(wasmBindings.wizard_state()));
        const dx=targetX-x,dy=targetY-y,distance=Math.hypot(dx,dy);
        if(distance<5){arrived=true;break}
        const length=Math.min(70,Math.max(9,distance/2));
        await session.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{
          x:center.x+dx/distance*length,y:center.y+dy/distance*length,id:1,
        }]});
        await page.waitForTimeout(40);
      }
      assert.ok(arrived,`touch movement reached ${targetX},${targetY}`);
    }
  } finally {
    await session.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});
    await session.detach();
  }
  const [x,y]=await page.evaluate(()=>Array.from(wasmBindings.wizard_state()));
  const viewport=page.viewportSize();
  // Source game-viewport.ts and hub-math.ts expand the world view before its
  // regional camera clamp; the world does not use the fixed UI stage's offset.
  const scale=Math.min(1,viewport.width/1600,viewport.height/900);
  const logical={width:viewport.width/scale,height:viewport.height/scale};
  const boundedScale=Math.max(scale,viewport.width/Math.max(1600,2000*1.2),
    viewport.height/Math.max(900,1024*1.2));
  const cameraScale=1.2*boundedScale/scale;
  const view={width:logical.width/cameraScale,height:logical.height/cameraScale};
  const originX=Math.max(0,Math.min(2000-view.width,x-view.width/2));
  const originY=Math.max(0,Math.min(1024-view.height,y-view.height/2));
  const target={x:(1397-originX)*cameraScale*scale,
    y:(664-originY)*cameraScale*scale};
  const entry={before:[x,y],viewport,logical,cameraScale,origin:[originX,originY],target};
  await page.touchscreen.tap(target.x,target.y);
  try {
    const dialogue=await ui.widget(page,'panel:dialogue',{timeout:3000});
    assert.equal(dialogue.label,'Fomentius');
    fs.writeFileSync(path.join(output,'phone-world-entry.json'),JSON.stringify({...entry,dialogue},null,2));
  } catch(error) {
    const after=await page.evaluate(()=>({wizard:Array.from(wasmBindings.wizard_state()),
      input:Array.from(wasmBindings.gameplay_input_state()),gpu:JSON.parse(wasmBindings.gpu_ui_state())}));
    fs.writeFileSync(path.join(output,'phone-world-entry-failure.json'),JSON.stringify({...entry,after},null,2));
    await shot(page,'phone-world-entry-failure.png');
    throw error;
  }
  await action(page,'dialogue-next');
  await ui.tap(page,(await ui.byLabel(page,'Buy',{prefix:'panel:',role:'button'})).id);
}

(async()=>{
  const owned=await launchOwnedBrowser(chromium,{headless:true,
    executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
  const {browser}=owned;
  let page,observed;
  try {
    page=await browser.newPage({viewport:{width:390,height:844},hasTouch:true,isMobile:true,deviceScaleFactor:1});
    observed=ui.watchErrors(page);
    await observed.ready;
    await ui.observeGpu(page);
    await page.goto(url,{waitUntil:'domcontentloaded'});
    await ui.ready(page);
    assert.match((await ui.widget(page,'landscape-guidance')).label,/Rotate your device to landscape/);
    await shot(page,'phone-title-portrait-guidance.png');
    await page.setViewportSize({width:844,height:390});
    await ui.noWidget(page,'landscape-guidance');
    await startNewGame(page,{name:'Felix',touch:true});
    await ui.tap(page,'hud:inventory');await settle(page);
    await action(page,'view-backpack');
    const health=await item(page,'Health Potion, quantity 1');
    assert.ok(ui.bounds(health).width>=46,'phone slots keep a usable physical size');
    await shot(page,'phone-inventory-rust.png');
    await ui.tap(page,health.id);
    const tooltip=ui.bounds(await ui.widget(page,'panel:tooltip'),{paint:true});
    assert.ok(fitsViewport(tooltip,844,390),'touch inspection stays on screen');
    await shot(page,'phone-item-tooltip-rust.png');
    await page.touchscreen.tap(832,195);
    const destination=ui.bounds(await ui.widget(page,'panel:cell:5'));
    await ui.drag(page,health.id,'panel:cell:5',{touch:true});
    await ui.waitForUi(page,snapshot=>{
      const moved=ui.findWidget(snapshot,health.id);
      if(!moved)return false;
      const point=ui.center(ui.bounds(moved)),target=destination;
      return point.x>=target.x&&point.x<=target.x+target.width
        &&point.y>=target.y&&point.y<=target.y+target.height;
    },'real Health Potion moved into column-first cell 5');
    const afterDrag=await ui.readUi(page);
    const toolbarActions=new Set(['panel:view-stats','panel:view-backpack',
      'panel:view-equipment','panel:skills','panel:close']);
    for(const widget of afterDrag.widgets.filter(widget=>widget.enabled
      &&widget.id.startsWith('panel:')&&!toolbarActions.has(widget.id))) {
      assert.ok(widget.rect[1]>=48,'scrolled native body hits stay below its fixed toolbar');
    }
    await action(page,'view-equipment');
    await ui.widget(page,'panel:equipment:hat');
    await ui.noWidget(page,health.id);
    fs.writeFileSync(path.join(output,'phone-navigation-after-drag.json'),JSON.stringify({
      before:afterDrag,afterFirstEquipTap:await ui.readUi(page),
    },null,2));
    await shot(page,'phone-equipment-first-tap-rust.png');
    await action(page,'equipment:hat');await action(page,'equipment:hat');
    const notice=await ui.widget(page,'panel:notice-title');
    assert.equal(notice.label,'A WIZARD WOULD NEVER\nREMOVE HIS HAT!');
    assert.ok(fitsViewport(ui.bounds(notice,{paint:true}),844,390),'authored notices fit the phone');
    await shot(page,'phone-notice-rust.png');
    await action(page,'notice-okay');
    await action(page,'skills');await settle(page);await settle(page);
    const skills=(await ui.readUi(page)).widgets.filter(widget=>widget.id.startsWith('panel:skill:'));
    assert.ok(skills.length>0,'the real learned Skills page is present');
    await ui.tap(page,skills[0].id);
    await ui.widget(page,'panel:tooltip');
    await shot(page,'phone-skills-rust.png');
    await page.touchscreen.tap(832,195);
    await action(page,'close');await settle(page);
    await ui.noWidget(page,'panel:close');
    await walkToFomentius(page);await settle(page);
    await shot(page,'phone-shop-rust.png');
    const dye=await ui.byLabel(page,'Buy Dye for 300 gold',{prefix:'panel:',role:'button'});
    await ui.tap(page,dye.id);await ui.tap(page,dye.id);await settle(page);
    await action(page,'close');await settle(page);
    await ui.tap(page,'hud:inventory');await settle(page);
    await action(page,'view-backpack');
    const ownedDye=await item(page,'Dye, quantity 1');
    await ui.tap(page,ownedDye.id);await ui.tap(page,ownedDye.id);
    await ui.widget(page,'panel:dye');
    const swatches=(await ui.readUi(page)).widgets.filter(widget=>widget.id.startsWith('panel:dye-swatch:'));
    assert.equal(swatches.length,18);
    await action(page,'dye-swatch:0');await action(page,'dye-swatch:7');
    assert.ok(ui.bounds(await ui.widget(page,'panel:dye-swatch:7')).width>=44);
    await shot(page,'phone-dye-rust.png');
    await action(page,'dye-cancel');
    await ui.noWidget(page,'panel:dye');
    await page.setViewportSize({width:390,height:844});
    await ui.widget(page,'landscape-guidance');
    await shot(page,'phone-inventory-portrait-guidance.png');
    await page.setViewportSize({width:844,height:390});await settle(page);
    await ui.noWidget(page,'landscape-guidance');
    await action(page,'view-equipment');
    assert.ok(ui.bounds(await ui.widget(page,'panel:equipment:hat')).width>=46);
    await shot(page,'landscape-inventory-rust.png');
    await page.setViewportSize({width:1600,height:900});await settle(page);
    assert.equal(ui.findWidget(await ui.readUi(page),'panel:navigation'),undefined,
      'native desktop controls replace the phone navigation toolbar');
    await ui.assertGpuOnly(page);
    await shot(page,'responsive-desktop-inventory-rust.png');
    await ui.settleErrors(page);
    assert.deepEqual(observed.errors,[]);
    fs.writeFileSync(path.join(output,'responsive-browser-receipt.json'),JSON.stringify({passed:true,
      errors:observed.errors,viewports:[[390,844],[844,390],[1600,900]],
      portraitNativeStage:'source landscape guidance',touchWorldEntry:'Fomentius',touchDrag:true,
      submissions:await ui.gpuDraws(page)},null,2));
    console.log(JSON.stringify({passed:true,errors:observed.errors}));
  } catch(error) {
    const failure={error:error.stack};
    if(page){
      await ui.settleErrors(page);
      failure.errors=observed.errors;
      failure.ui=await ui.readUi(page);
      await shot(page,'responsive-failure.png');
    }
    fs.writeFileSync(path.join(output,'responsive-failure.json'),JSON.stringify(failure,null,2));
    throw error;
  } finally {
    try {fs.writeFileSync(path.join(output,'browser-cleanup.json'),JSON.stringify(await owned.close(),null,2))}
    catch(error){
      fs.writeFileSync(path.join(output,'browser-cleanup-failure.json'),JSON.stringify(error.cleanup||{error:error.stack},null,2));
      throw error;
    }
  }
})().catch(error=>{console.error(error);process.exitCode=1});
