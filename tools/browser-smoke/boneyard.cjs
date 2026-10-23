// Ordinary College/Create/Map admission, actual seeded scenery, movement and Leave/Continue.
// Retained generator fixtures own fixed-seed geometry; this journey never mutates authority.
const assert=require('node:assert/strict');
const fs=require('node:fs/promises');
const path=require('node:path');
const {chromium}=require(path.resolve(process.env.PW));
const {startNewGame}=require('./menu-journey.cjs');
const {readSave,waitForSave,localParticipant}=require('./saved-game.cjs');
const ui=require('./gpu-ui.cjs');
const output=process.env.OUT||path.resolve('boneyard-smoke');
const url=process.env.URL||'http://127.0.0.1:4181/';
const state=page=>page.evaluate(()=>Array.from(window.wasmBindings.wizard_state()));

async function admitted(page) {
  const deadline=Date.now()+120000;
  const loading=[];
  while(Date.now()<deadline) {
    const snapshot=await ui.readUi(page);
    if(snapshot.error)throw new Error(snapshot.error);
    const stage=ui.findWidget(snapshot,'match-loading:label');
    if(stage&&loading.at(-1)?.label!==stage.label)loading.push({label:stage.label,draw:snapshot.draw});
    const saved=await readSave(page);
    if(saved?.checkpoint.boneyard) {
      await ui.noWidget(page,'match-loading:label');
      await ui.surface(page,'hidden');
      return {saved,loading};
    }
    await page.waitForTimeout(100);
  }
  throw new Error('ordinary Map admission never committed a Boneyard');
}

async function scenery(page,filename) {
  const bytes=await page.screenshot({path:filename});
  const pixels=await page.evaluate(async encoded=>{
    const image=await createImageBitmap(new Blob([
      Uint8Array.from(atob(encoded),character=>character.charCodeAt(0)),
    ],{type:'image/png'}));
    const context=new OffscreenCanvas(image.width,image.height).getContext('2d');
    context.drawImage(image,0,0);
    // The central world excludes the top meters, bottom belt and side shortcuts.
    const region={x:400,y:250,width:800,height:400};
    const data=context.getImageData(region.x,region.y,region.width,region.height).data;
    const colors=new Set();
    let minimum=255,maximum=0;
    for(let index=0;index<data.length;index+=16) {
      colors.add(`${data[index]},${data[index+1]},${data[index+2]}`);
      const value=Math.max(data[index],data[index+1],data[index+2]);
      minimum=Math.min(minimum,value);maximum=Math.max(maximum,value);
    }
    return {region,colors:colors.size,minimum,maximum};
  },bytes.toString('base64'));
  assert.ok(pixels.colors>32&&pixels.maximum-pixels.minimum>20,
    `the actual world contains textured scenery rather than a blank frame: ${JSON.stringify(pixels)}`);
  return pixels;
}

(async()=>{
  await fs.mkdir(output,{recursive:true});
  const catalog=JSON.parse(await fs.readFile(path.resolve(__dirname,'../../web/assets/boneyards.json'),'utf8'));
  const templates=new Set(catalog.templates.map(template=>`${template.sourceSha256}:${template.geometrySha256}`));
  const browser=await chromium.launch({headless:true,
    executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
  const receipt={url,maps:[],errors:[]};
  let current;
  let currentErrors=[];
  try {
    for(const name of ['Felix','Vitus']) {
      // Each actual new wizard receives its own ordinary random seed and fresh local save.
      const context=await browser.newContext({viewport:{width:1600,height:900},deviceScaleFactor:1});
      current=await context.newPage();
      const observed=ui.watchErrors(current);
      await observed.ready;
      currentErrors=observed.errors;
      await ui.observeGpu(current);
      await current.goto(url,{waitUntil:'domcontentloaded'});
      await ui.ready(current);
      await startNewGame(current,{name});
      await ui.click(current,'hud:map');
      const {saved:entrySave,loading}=await admitted(current);
      const world=entrySave.checkpoint.boneyard;
      assert.match(world.seed,/^[0-9a-f]{32}$/,'the accepted world retains its actual generated seed');
      assert.ok(templates.has(`${world.source_sha256}:${world.geometry_sha256}`),
        'the actual world uses a retained native source and geometry pair');
      assert.equal(entrySave.checkpoint.run.run_id,world.run_id,'one admitted run owns the loaded world');
      assert.equal(localParticipant(entrySave).player.selection.name,name);
      const entry=await state(current);
      const entryPixels=await scenery(current,path.join(output,`${name}-entry.png`));
      const side=world.arena_transition?.entry_side;
      assert.ok(side==='North'||side==='South','the native entrance side is retained');
      const key=side==='North'?'KeyS':'KeyW';
      const direction=side==='North'?1:-1;
      await current.keyboard.down(key);
      try {await current.waitForTimeout(3000)}
      finally {await current.keyboard.up(key)}
      const walked=await state(current);
      assert.ok((walked[1]-entry[1])*direction>50,'ordinary input walks inward through the real entrance');
      assert.ok(walked[5]>entry[5],'the Boneyard advances its actual clock');
      const walkedPixels=await scenery(current,path.join(output,`${name}-walk.png`));
      await current.keyboard.press('Escape');
      await ui.surface(current,'pause');
      const paused=await state(current);
      await current.waitForTimeout(300);
      assert.deepEqual(await state(current),paused,'Boneyard Pause freezes the admitted world');
      await ui.click(current,'leave');
      await ui.surface(current,'title');
      const left=await waitForSave(current,record=>record.checkpoint.boneyard?.run_id===world.run_id
        &&record.checkpoint.ticks===paused[5]);
      await current.screenshot({path:path.join(output,`${name}-left-title.png`)});
      await ui.click(current,'play');
      assert.equal((await ui.widget(current,'continue')).enabled,true);
      await ui.click(current,'continue');
      await ui.surface(current,'hidden');
      await current.waitForFunction(tick=>window.wasmBindings.wizard_state()[5]>tick,paused[5]);
      const resumed=await state(current);
      assert.ok(Math.hypot(resumed[0]-paused[0],resumed[1]-paused[1])<0.1,
        'Continue returns to the exact paused entrance position');
      await current.keyboard.press('Escape');
      await ui.surface(current,'pause');
      const continuedPause=await state(current);
      const continued=await waitForSave(current,record=>record.revision>left.revision
        &&record.checkpoint.ticks===continuedPause[5]);
      assert.equal(continued.checkpoint.boneyard.run_id,world.run_id,'the live continued world keeps its run');
      assert.equal(continued.checkpoint.boneyard.seed,world.seed,'Continue retains the same real seed');
      assert.equal(localParticipant(continued).player.selection.name,name,'Continue retains the admitted wizard');
      await ui.click(current,'resume');
      await ui.surface(current,'hidden');
      await ui.assertGpuOnly(current);
      const continuedPixels=await scenery(current,path.join(output,`${name}-continued.png`));
      const submissions=await ui.gpuDraws(current);
      assert.ok(submissions.canvasCalls.game>0,'the actual game canvas submitted the world and HUD');
      await ui.settleErrors(current);
      receipt.maps.push({name,seed:world.seed,runId:world.run_id,
        sourceSha256:world.source_sha256,geometrySha256:world.geometry_sha256,
        entrySide:side,entry,walked,paused,resumed,loading,
        saveRevision:left.revision,entryPixels,walkedPixels,continuedPixels,submissions,
        errors:observed.errors,warnings:observed.warnings});
      assert.deepEqual(observed.errors,[]);
      await context.close();
      current=null;
    }
    assert.notEqual(receipt.maps[0].seed,receipt.maps[1].seed,'independent admissions receive independent seeds');
    receipt.status='passed';
  } catch(error) {
    receipt.status='failed';receipt.failure=error.stack;
    if(current)await ui.settleErrors(current);
    receipt.errors.push(...currentErrors);
    if(current) {
      receipt.failedUi=await ui.readUi(current);
      receipt.failedSave=await readSave(current);
      receipt.failedPlayer=await state(current);
      await current.screenshot({path:path.join(output,'failure.png')});
    }
    process.exitCode=1;
  } finally {
    await fs.writeFile(path.join(output,'receipt.json'),`${JSON.stringify(receipt,null,2)}\n`);
    await browser.close();
  }
  console.log(JSON.stringify({status:receipt.status,maps:receipt.maps.length,errors:receipt.errors,failure:receipt.failure}));
})().catch(error=>{console.error(error);process.exitCode=1});
