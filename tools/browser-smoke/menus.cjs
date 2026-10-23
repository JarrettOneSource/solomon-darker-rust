// Offline title/admission/Create/pause/settings/Continue with actual GPU widgets.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {chromium}=require(path.resolve(process.env.PW));
const {reachCollegeLoadout}=require('./menu-journey.cjs');
const ui=require('./gpu-ui.cjs');
const url=process.env.URL||'http://127.0.0.1:4185/';
const output=process.env.OUT||'/tmp';
fs.mkdirSync(output,{recursive:true});
const state=page=>page.evaluate(()=>Array.from(window.wasmBindings.wizard_state()));
const screen=ui.surface;
const shot=(page,name)=>page.screenshot({path:path.join(output,name)});

async function ticks(page,count) {
  const start=(await state(page))[5];
  await page.waitForFunction(target=>window.wasmBindings.wizard_state()[5]>=target,
    start+count,{timeout:20000});
}

(async()=>{
  const browser=await chromium.launch({headless:true,
    executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
  try {
    const page=await browser.newPage({viewport:{width:1600,height:900}});
    const observed=ui.watchErrors(page);
    await observed.ready;
    await ui.observeGpu(page);
    await ui.observeAnnouncements(page);
    await page.goto(url,{waitUntil:'domcontentloaded'});
    await ui.ready(page);
    await screen(page,'title');
    if(ui.findWidget(await ui.readUi(page),'prompt-secondary'))await ui.click(page,'prompt-secondary');
    await ui.assertGpuOnly(page);
    await shot(page,'menu-title.png');
    assert.equal((await state(page))[5],0,'the title does not advance the world');
    await ui.click(page,'play');
    assert.equal((await ui.widget(page,'continue')).enabled,false);
    await ui.click(page,'new-game');
    await screen(page,'hidden');
    await reachCollegeLoadout(page);
    assert.equal((await ui.widget(page,'create-back')).enabled,false,
      'the sole character-selection screen belongs to College admission');
    await ui.fill(page,'create-name','AAAAAAAAAAA');
    await ui.value(page,'create-name','AAAAAAAAAAA');
    await ui.fill(page,'create-name','AAAAAAAAAAAA');
    await ui.value(page,'create-name','AAAAAAAAAAA');
    await ui.value(page,'create-validation','Wizard name is too wide.');
    await ui.fill(page,'create-name','Solon-Solus');
    await ui.value(page,'create-name','AAAAAAAAAAA');
    await ui.fill(page,'create-name','');
    await ui.value(page,'create-name','');
    await ui.noWidget(page,'create-validation');
    await ui.waitForUi(page,snapshot=>ui.findWidget(snapshot,'randomize-loadout')?.enabled,
      'completed native Create entry');
    await ui.fill(page,'create-name','Felix');
    await ui.value(page,'create-name','Felix');
    await ui.widget(page,'element:ether');
    await page.waitForTimeout(1100);
    await shot(page,'menu-create-elements.png');
    await ui.click(page,'element:ether');
    await ui.widget(page,'discipline:arcane');
    await page.waitForTimeout(1100);
    await shot(page,'menu-create-ether.png');
    await ui.click(page,'discipline:arcane');
    await screen(page,'hidden');
    await page.waitForFunction(()=>window.wasmBindings.wizard_state()[6]===0
      &&window.wasmBindings.wizard_state()[7]===-1,null,{timeout:30000});
    await ticks(page,5);
    assert.equal((await ui.widget(page,'hud:health')).value,'50');
    assert.equal((await ui.widget(page,'hud:mana')).value,'100');
    await shot(page,'menu-new-game.png');
    await page.keyboard.down('KeyS');
    await ticks(page,70);
    await page.keyboard.up('KeyS');
    await ticks(page,35);
    await page.keyboard.press('Escape');
    await screen(page,'pause');
    await page.waitForTimeout(400);
    const paused=await state(page);
    await page.keyboard.down('KeyS');
    await ticks(page,30);
    await page.keyboard.up('KeyS');
    const afterPause=await state(page);
    assert.ok(afterPause[5]>paused[5],'College actors continue while its menu is open');
    assert.deepEqual(afterPause.slice(0,2),paused.slice(0,2),'College pause seals player movement');
    await shot(page,'menu-pause.png');
    await ui.click(page,'pause-settings');
    await screen(page,'settings');
    const announcementsBefore=(await ui.announcements(page)).length;
    await ui.setSlider(page,'settings-range:ui-scale',135);
    await ui.announced(page,'UI SCALE',{afterIndex:announcementsBefore});
    await ui.announced(page,'135',{afterIndex:announcementsBefore});
    await shot(page,'menu-settings-root.png');
    await ui.click(page,'settings-page:controls');
    await ui.widget(page,'bind:move-up');
    await ui.click(page,'bind:move-up');
    await page.keyboard.press('Escape');
    await ui.value(page,'bind:move-up','Escape');
    assert.equal((await ui.widget(page,'bind:menu')).value,'KeyW',
      'Escape rebind swaps the previous Menu control');
    await screen(page,'settings');
    await ui.click(page,'bind:move-up');
    await page.keyboard.press('Control+q');
    await ui.nextFrame(page);
    assert.equal((await ui.widget(page,'bind:move-up')).selected,true,
      'modified keys do not finish capture');
    const binding=ui.bounds(await ui.widget(page,'bind:move-up'));
    await page.mouse.click(binding.x+15,binding.y+15,{button:'right'});
    await ui.nextFrame(page);
    assert.equal((await ui.widget(page,'bind:move-up')).selected,true,
      'movement cannot bind a mouse button');
    await page.keyboard.press('KeyJ');
    await ui.value(page,'bind:move-up','KeyJ');
    await ui.click(page,'bind:menu');
    await page.keyboard.press('Escape');
    await ui.value(page,'bind:menu','Escape');
    await ui.click(page,'bind:move-down');
    await page.keyboard.press('ArrowUp');
    await ui.value(page,'bind:move-down','ArrowUp');
    await shot(page,'menu-settings-keyboard.png');
    await ui.click(page,'settings-back');
    await ui.widget(page,'settings-range:ui-scale');
    await ui.click(page,'settings-back');
    await screen(page,'pause');
    await ui.click(page,'resume');
    await screen(page,'hidden');
    await ticks(page,5);
    const meter=ui.bounds(await ui.widget(page,'hud:health'));
    assert.ok(Math.abs(meter.width-148.5)<0.01,'UI scale reaches the actual HUD');
    const beforeRebound=await state(page);
    await page.keyboard.down('KeyJ');
    await ticks(page,60);
    await page.keyboard.up('KeyJ');
    await ticks(page,40);
    const afterRebound=await state(page);
    assert.ok(afterRebound[1]<beforeRebound[1]-20,'the rebound movement key reaches simulation');
    await page.keyboard.down('KeyW');
    await ticks(page,40);
    await page.keyboard.up('KeyW');
    const afterOldKey=await state(page);
    assert.ok(Math.hypot(afterOldKey[0]-afterRebound[0],afterOldKey[1]-afterRebound[1])<0.1,
      'the replaced key is released');
    await page.keyboard.down('ArrowUp');
    await ticks(page,60);
    await page.keyboard.up('ArrowUp');
    await ticks(page,40);
    assert.ok((await state(page))[1]>afterOldKey[1]+20,'an assigned arrow overrides its legacy direction');
    await page.keyboard.press('Escape');
    await screen(page,'pause');
    await ui.click(page,'leave');
    await screen(page,'title');
    const retained=await state(page);
    await ui.click(page,'play');
    assert.equal((await ui.widget(page,'continue')).enabled,true);
    await ui.click(page,'continue');
    await screen(page,'hidden');
    assert.ok(Math.hypot((await state(page))[0]-retained[0],(await state(page))[1]-retained[1])<0.1,
      'Continue returns to the same local world');
    const gpu=await ui.assertGpuOnly(page);
    await shot(page,'menu-continued.png');
    await ui.settleErrors(page);
    assert.deepEqual(observed.errors,[]);
    console.log(JSON.stringify({checks:['title clock','College admission before the sole loadout',
      'accepted/rejected stock Create names','stock Create effects','new-game HUD',
      'College pause seals input while actors advance','real settings slider',
      'key rebinding and capture rejection','HUD scale','screen-reader focus/value announcements',
      'Continue','actual GPU UI'],gpu:gpu.gpu.draw,announcements:await ui.announcements(page),errors:observed.errors}));
  } finally {await browser.close()}
})().catch(error=>{console.error(error);process.exitCode=1});
