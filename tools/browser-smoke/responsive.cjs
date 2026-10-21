// Real touch gestures use the same owned inventory actions as desktop input.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {chromium}=require(path.resolve(process.env.PW));
const output=process.env.OUT||'/tmp';
const url=process.env.URL||'http://127.0.0.1:4185/';
const action=(page,name)=>page.locator(`[data-panel-action="${name}"]`).first();
const settle=page=>page.waitForTimeout(450);
const shot=(page,name)=>page.screenshot({path:path.join(output,name)});
async function dragTouch(page,from,to) {
  const source=await from.boundingBox(),target=await to.boundingBox();
  const session=await page.context().newCDPSession(page);
  const start={x:source.x+source.width/2,y:source.y+source.height/2};
  const end={x:target.x+target.width/2,y:target.y+target.height/2};
  await session.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[start]});
  for(let step=1;step<=10;step++) {
    await session.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{x:start.x+(end.x-start.x)*step/10,y:start.y+(end.y-start.y)*step/10}]});
  }
  await session.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});
  await session.detach();
}
// The pinned courtyard path helper generated these clear corners. The browser drives
// the real touch joystick and observes authoritative position; no state is teleported.
async function walkToFomentius(page) {
  const session=await page.context().newCDPSession(page);
  const box=await page.locator('[data-joystick="movement"]').boundingBox();
  const center={x:box.x+box.width/2,y:box.y+box.height/2};
  const route=[[1050,200],[1090,190],[1120,210],[1160,370],[1170,520],[1360,730]];
  await session.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[center]});
  try {
    for(const [targetX,targetY] of route) {
      const deadline=Date.now()+30000;
      let arrived=false;
      while(Date.now()<deadline) {
        const [x,y]=await page.evaluate(()=>Array.from(wasmBindings.wizard_state()));
        const dx=targetX-x,dy=targetY-y,distance=Math.hypot(dx,dy);
        if(distance<5) {arrived=true;break;}
        const length=Math.min(70,Math.max(9,distance/2));
        await session.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{x:center.x+dx/distance*length,y:center.y+dy/distance*length}]});
        await page.waitForTimeout(40);
      }
      assert.ok(arrived,`touch movement reached ${targetX},${targetY}`);
    }
  } finally {
    await session.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});
    await session.detach();
  }
  const [x,y]=await page.evaluate(()=>Array.from(wasmBindings.wizard_state()));
  const originX=Math.max(0,Math.min(2000-1600/1.2,x-800/1.2));
  const originY=Math.max(0,Math.min(1024-900/1.2,y-450/1.2));
  const viewport=page.viewportSize();
  const scale=Math.min(viewport.width/1600,viewport.height/900);
  const target={x:(viewport.width-1600*scale)/2+(1397-originX)*1.2*scale,
    y:(viewport.height-900*scale)/2+(664-originY)*1.2*scale};
  await page.touchscreen.tap(target.x,target.y);
  try {
    await page.getByRole('dialog',{name:'Fomentius',exact:true}).waitFor({timeout:3000});
  } catch(error) {
    const after=await page.evaluate(()=>({wizard:Array.from(wasmBindings.wizard_state()),input:Array.from(wasmBindings.gameplay_input_state())}));
    fs.writeFileSync(path.join(output,'phone-world-entry-failure.json'),JSON.stringify({before:[x,y],target,after},null,2));
    await shot(page,'phone-world-entry-failure.png');
    throw error;
  }
  await action(page,'dialogue-next').tap();
  await page.getByRole('button',{name:'Buy',exact:true}).tap();
}

(async()=>{
  const browser=await chromium.launch({headless:true,executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
  const errors=[];
  try {
    const page=await browser.newPage({viewport:{width:390,height:844},hasTouch:true,isMobile:true,deviceScaleFactor:1});
    page.on('pageerror',error=>errors.push(error.message));
    page.on('console',message=>{if(message.type()==='error')errors.push(message.text())});
    page.on('response',response=>{if(response.status()>=400)errors.push(`${response.status()} ${response.url()}`)});
    await page.goto(url);
    await page.waitForFunction(()=>window.wasmBindings?.wizard_state().length>0&&!document.querySelector('#status').textContent,null,{timeout:60000});
    await page.locator('[data-menu-action="play"]').tap();
    await page.locator('[data-menu-action="new-game"]').tap();
    await page.getByRole('textbox',{name:'Wizard name',exact:true}).fill('Felix');
    await page.locator('[data-menu-action="element:ether"]').tap();
    await page.locator('[data-menu-action="discipline:arcane"]').tap();
    await page.waitForFunction(()=>document.querySelector('.game-menus').dataset.menuScreen==='hidden');
    await page.getByRole('button',{name:/^Inventory, /}).tap();await settle(page);
    await action(page,'view-backpack').tap();
    const health=page.getByRole('button',{name:'Health Potion, quantity 1',exact:true});
    assert.ok((await health.boundingBox()).width>=46,'phone slots keep a usable physical size');
    await shot(page,'phone-inventory-rust.png');
    await health.tap();
    await page.locator('.player-panels .player-panel-tooltip').waitFor({state:'visible'});
    const tooltip=await page.locator('.player-panels .player-panel-tooltip').boundingBox();
    assert.ok(tooltip.x>=0&&tooltip.x+tooltip.width<=390,'touch inspection stays on screen');
    await shot(page,'phone-item-tooltip-rust.png');
    await page.touchscreen.tap(378,180);
    await dragTouch(page,health,action(page,'cell:5'));
    await page.waitForFunction(()=>document.querySelector('[aria-label="Health Potion, quantity 1"]')?.dataset.inventorySlot==='5');
    await action(page,'view-equipment').tap();
    const hat=action(page,'equipment:hat');await hat.tap();await hat.tap();
    await page.getByRole('heading',{name:'A WIZARD WOULD NEVER REMOVE HIS HAT!'}).waitFor();
    const notice=await page.locator('.player-panel-notice').boundingBox();
    assert.ok(notice.x>=0&&notice.x+notice.width<=390,'authored notices fit the phone');
    await shot(page,'phone-notice-rust.png');
    await action(page,'notice-okay').tap();
    await action(page,'skills').tap();await settle(page);await settle(page);
    await page.locator('[data-panel-action^="skill:"]').first().tap();
    await page.locator('.player-panels .player-panel-tooltip .panel-tooltip-title').waitFor();
    await shot(page,'phone-skills-rust.png');
    await page.touchscreen.tap(378,180);
    await page.locator('.player-panel-navigation [data-panel-action="close"]').tap();await settle(page);
    await page.locator('.player-panels').waitFor({state:'hidden'});
    await walkToFomentius(page);await settle(page);
    await shot(page,'phone-shop-rust.png');
    const dye=page.getByRole('button',{name:'Buy Dye for 300 gold',exact:true}).first();
    await dye.tap();await dye.tap();await settle(page);
    await page.locator('.player-panel-navigation [data-panel-action="close"]').tap();await settle(page);
    await page.getByRole('button',{name:/^Inventory, /}).tap();await settle(page);
    await action(page,'view-backpack').tap();
    const ownedDye=page.getByRole('button',{name:'Dye, quantity 1',exact:true});
    await ownedDye.tap();await ownedDye.tap();
    await page.locator('.panel-compact-dye').waitFor();
    assert.equal(await page.locator('[data-panel-action^="dye-swatch:"]').count(),18);
    await action(page,'dye-swatch:0').tap();await action(page,'dye-swatch:7').tap();
    assert.ok((await action(page,'dye-swatch:7').boundingBox()).width>=44);
    await shot(page,'phone-dye-rust.png');
    await action(page,'dye-cancel').tap();
    await page.locator('.panel-compact-dye').waitFor({state:'detached'});
    await page.setViewportSize({width:844,height:390});await settle(page);
    await action(page,'view-equipment').tap();
    assert.ok((await action(page,'equipment:hat').boundingBox()).width>=46);
    await shot(page,'landscape-inventory-rust.png');
    await page.setViewportSize({width:1600,height:900});await settle(page);
    assert.equal(await page.locator('.player-panel-navigation').isVisible(),false);
    await shot(page,'responsive-desktop-inventory-rust.png');
    assert.deepEqual(errors,[]);
    fs.writeFileSync(path.join(output,'responsive-browser-receipt.json'),JSON.stringify({passed:true,errors,viewports:[[390,844],[844,390],[1600,900]],touchWorldEntry:'Fomentius',touchDrag:true},null,2));
    console.log(JSON.stringify({passed:true,errors}));
  } finally {await browser.close()}
})().catch(error=>{console.error(error);process.exitCode=1});
