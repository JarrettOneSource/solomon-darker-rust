// Actual browser listeners and the public sampled input seam, using a fresh local wizard.
const assert=require('node:assert/strict');
const path=require('node:path');
const {chromium}=require(path.resolve(process.env.PW));
const {startNewGame}=require('./menu-journey.cjs');
const url=process.env.URL||'http://127.0.0.1:4185/';
const input=page=>page.evaluate(()=>Array.from(window.wasmBindings.gameplay_input_state()));
async function frames(page,count=3){await page.evaluate(async count=>{for(let i=0;i<count;i++)await new Promise(requestAnimationFrame)},count);}
async function pad(page,buttons=[],axes=[0,0,0,0],analog={}){
  await page.evaluate(({buttons,axes,analog})=>{window.inputTestPad={buttons,axes,analog}}, {buttons,axes,analog});
  await frames(page);
}
(async()=>{
  const browser=await chromium.launch({headless:true,
    executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
  const errors=[];
  try{
    const page=await browser.newPage({viewport:{width:1600,height:900}});
    page.on('pageerror',error=>errors.push(error.message));
    page.on('console',message=>{if(message.type()==='error')errors.push(message.text())});
    page.on('response',response=>{if(response.status()>=400)errors.push(`${response.status()} ${response.url()}`)});
    await page.goto(url);
    await page.waitForFunction(()=>window.wasmBindings?.wizard_state().length>0&&!document.querySelector('#status').textContent,null,{timeout:60000});
    await startNewGame(page);
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
    assert.equal(await page.locator('.hud-belt-slot[data-slot="3"]').isDisabled(),false,'the consumed potion alias remains removable through the source belt gesture');
    await page.keyboard.up('Digit3');await frames(page);
    const manaButton=page.locator('.hud-belt-slot[data-slot="4"]');
    const manaBounds=await manaButton.boundingBox();
    const beforeTap=(await input(page))[8];
    await page.mouse.move(manaBounds.x+manaBounds.width/2,manaBounds.y+manaBounds.height/2);
    await page.mouse.down();await frames(page);
    assert.equal((await input(page))[8],beforeTap,'a belt GUI press waits for its release');
    assert.equal(await manaButton.isDisabled(),false);
    await page.mouse.up();await frames(page);
    assert.equal((await input(page))[8],beforeTap+1);
    assert.equal(await manaButton.isDisabled(),false,'the consumed mana alias remains removable');
    await page.keyboard.down('Digit1');await frames(page);
    await page.mouse.move(1160,270);
    await page.mouse.down({button:'right'});await frames(page);
    assert.equal((await input(page))[3],0,'the latest input source owns the held belt');
    await page.mouse.up({button:'right'});await frames(page);
    assert.equal((await input(page))[3],1,'releasing one source restores the still-held keyboard source');
    await page.evaluate(()=>window.dispatchEvent(new Event('blur')));await frames(page);
    sample=await input(page);assert.equal(sample[3],-1);assert.equal(sample[4],0);
    await page.keyboard.up('Digit1');
    console.log('mouse, keyboard, potion edge and blur passed');
    await page.evaluate(()=>{
      window.inputTestPad={buttons:[],axes:[0,0,0,0],analog:{}};
      window.inputTestPadReads=0;
      const gamepad=Object.create(Gamepad.prototype,{
        index:{value:0},connected:{value:true},mapping:{value:'standard'},
        axes:{get:()=>window.inputTestPad.axes},
        buttons:{get:()=>Array.from({length:16},(_,i)=>Object.create(GamepadButton.prototype,{
          pressed:{value:window.inputTestPad.buttons.includes(i)},
          value:{value:window.inputTestPad.analog[i]??(window.inputTestPad.buttons.includes(i)?1:0)},
        }))},
      });
      Object.defineProperty(navigator,'getGamepads',{value:()=>{window.inputTestPadReads++;return[gamepad]}});
    });
    await frames(page);
    const reads=await page.evaluate(async()=>{
      const before=window.inputTestPadReads;
      for(let i=0;i<8;i++)await new Promise(requestAnimationFrame);
      return window.inputTestPadReads-before;
    });
    assert.equal(reads,8,'gameplay reads gamepads once per animation frame');
    await pad(page,[],[0,0,.8,-.6],{7:.7});
    sample=await input(page);assert.equal(sample[4],2);assert.equal(sample[5],.8);assert.equal(sample[6],-.6);assert.equal(sample[2],0);
    await pad(page,[5]);assert.equal((await input(page))[7],1);
    assert.equal(await page.locator('.hud-belt-slot[data-slot="1"][data-controller-selected="true"]').count(),1,'an empty controller selection stays visible');
    await pad(page,[2]);assert.equal((await input(page))[3],1);
    await pad(page,[2,5]);assert.equal((await input(page))[7],1,'the selected belt slot stays fixed during a held cast');
    await pad(page,[]);
    await pad(page,[8]);
    await page.locator('.player-panels:not([hidden]) [data-player-panel="inventory"]').waitFor();
    await pad(page,[2,7],[1,0,1,0]);
    sample=await input(page);assert.equal(sample[2],0);assert.equal(sample[3],-1);assert.equal(sample[0],0);
    await page.keyboard.press('Escape');await page.locator('.player-panels').waitFor({state:'hidden'});await frames(page);
    sample=await input(page);assert.equal(sample[3],-1);assert.equal(sample[0],0,'held controls require a neutral handoff after the panel');
    await pad(page,[]);await pad(page,[],[0,0,1,0]);
    assert.equal((await input(page))[4],2);
    await pad(page,[]);await pad(page,[9]);
    await page.locator('.game-menus[data-menu-screen="pause"]').waitFor();
    assert.deepEqual(errors,[]);
    if(process.env.OUT)await page.screenshot({path:path.join(process.env.OUT,'gameplay-input-pause.png')});
    console.log(JSON.stringify({passed:true,errors,gamepadReadsPerFrame:1,primaryContext:'College-disabled; live Boneyard acceptance follows coordinator integration'}));
  }finally{await browser.close()}
})().catch(error=>{console.error(error);process.exitCode=1});
