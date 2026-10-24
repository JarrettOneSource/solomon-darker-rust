// Ordinary Title YES through the real Tutorial combat, books, potion and return.
// Native frames are observations. Source camera/navigation only choose real input;
// no planner values, commands or checkpoints are installed into the game.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {pathToFileURL}=require('node:url');
const {chromium}=require(path.resolve(process.env.PW));
const ui=require('./gpu-ui.cjs');
const {launchOwnedBrowser}=require('./browser-lifecycle.cjs');
const {createNativeMessageReader}=require('./native-wire.cjs');
const {walkSolomonEntrance}=require('./solomon-entrance.cjs');
const {reachCollegeLoadout}=require('./menu-journey.cjs');
const {assertTutorialTextBounds}=require('./tutorial-text-bounds.cjs');

const output=path.resolve(process.env.OUT||'/tmp/tutorial-progression');
const url=new URL(process.env.URL||'https://127.0.0.1:9449/game-stg/');
assert.ok(['127.0.0.1','localhost'].includes(url.hostname),'use the owned native admission host');
const website=path.resolve(process.env.SOLOMON_WEBSITE_DIR||path.join(__dirname,'../../../Website'));
const source=file=>import(pathToFileURL(path.join(website,'frontend/src/game',file)).href);
const receipt={passed:false,url:url.href,checks:[],stages:[],lessons:[],commands:[],errors:[]};
const accepted={id:null,frame:null,loaded:null,checkpoint:null};
const held=new Set();
let primary=false;
let page,observed;
const world=()=>accepted.frame?.snapshot.boneyard;
const player=()=>accepted.frame?.snapshot.participants[accepted.id];
const stage=()=>world()?.tutorial?.stage;
const living=()=>world()?.enemies.actors.filter(actor=>actor.life_state==='Alive')||[];
const distance=(a,b)=>Math.hypot(a.x-b.x,a.y-b.y);
const point=value=>({x:value.x,y:value.y});

async function until(predicate,label,timeout=30000) {
  const deadline=Date.now()+timeout;
  do {
    if(await predicate())return;
    if(observed.errors.length){
      await ui.settleErrors(page);
      assert.deepEqual(observed.errors,[],label);
    }
    await page.waitForTimeout(40);
  }while(Date.now()<deadline);
  throw new Error(`Timed out waiting for ${label}`);
}
function checked(label) {
  receipt.checks.push(label);
  console.log(label);
}
async function keys(desired) {
  for(const key of held)if(!desired.includes(key)){
    await page.keyboard.up(key);held.delete(key);
  }
  for(const key of desired)if(!held.has(key)){
    await page.keyboard.down(key);held.add(key);
  }
}
async function release() {
  await keys([]);
  if(primary){await page.mouse.up();primary=false;}
}
async function moveTo(target) {
  const position=player().wizard.position;
  const x=target.x-position.x,y=target.y-position.y;
  if(Math.hypot(x,y)<14){await keys([]);return;}
  const desired=[];
  const diagonal=Math.tan(Math.PI/8);
  if(Math.abs(x)>Math.abs(y)*diagonal)desired.push(x<0?'KeyA':'KeyD');
  if(Math.abs(y)>Math.abs(x)*diagonal)desired.push(y<0?'KeyW':'KeyS');
  await keys(desired);
}
async function lesson(expected) {
  await ui.waitForUi(page,snapshot=>ui.findWidget(snapshot,'tutorial:text:heading')?.label===expected,
    `actual Tutorial lesson ${expected}`);
  const bounds=await assertTutorialTextBounds(page);
  await ui.announced(page,expected.replaceAll('\n',' '));
  receipt.lessons.push({stage:stage(),heading:expected,bounds});
  await page.screenshot({path:path.join(output,`lesson-${stage()}.png`)});
}
async function modal(kind) {
  const inventory=kind==='Inventory';
  const key=inventory?'KeyI':'KeyK';
  await release();
  await page.keyboard.press(key);
  await ui.widget(page,`panel:surface:${inventory?'inventory':'skills'}`);
  const texts=inventory?{
    resume:"Click here or press 'I'\nagain to resume playing",
    'quick-use':'Put items here\nfor quick use',
    equipment:'Put equippable items\nhere to wear them.',
    backpack:'Found items go in your backpack.  Click and\ndrag to move items, double-click to use them.',
  }:{
    resume:"Click here or press 'K'\nagain to resume playing",
    'quick-use':'Drag skills here\nfor quick use',
    concentration:'You are CONCENTRATING on\nyour new skill automatically',
    'concentration-limit':'This confers a bonus, but is\nlimited to one skill at a time.',
    hover:'Hover your mouse over a\nskill icon for more information.',
  };
  for(const [id,text]of Object.entries(texts)){
    const callout=await ui.widget(page,`tutorial:callout:${id}`);
    assert.equal(callout.label,text);
    assert.ok(callout.paintRect.every(Number.isFinite),'actual native callout paint geometry');
  }
  const before={position:point(player().wizard.position),ticks:accepted.frame.snapshot.ticks};
  await page.keyboard.down('KeyD');
  try {
    const current=await ui.readUi(page);
    await ui.waitForUi(page,snapshot=>snapshot.frame>current.frame+2,
      'actual modal frames with held movement');
    assert.deepEqual(point(player().wizard.position),before.position,'Tutorial book seals movement');
  }finally{await page.keyboard.up('KeyD');}
  await page.screenshot({path:path.join(output,`${kind.toLowerCase()}-callouts.png`)});
  await ui.assertGpuOnly(page);
  await page.keyboard.press(key);
  await ui.noWidget(page,`panel:surface:${inventory?'inventory':'skills'}`);
  await until(()=>!accepted.frame.host.pause&&!accepted.frame.host.resume_grace,
    `accepted ${kind} close and native resume grace`);
  checked(`real ${kind} open/close, Source callouts and modal movement ownership`);
}

(async()=>{
  fs.mkdirSync(output,{recursive:true});
  const [tutorial,cameras,viewports]=await Promise.all([
    source('core-kernels/native-tutorial.ts'),source('renderer/boneyard-render-contract.ts'),
    source('renderer/game-viewport.ts'),
  ]);
  const owned=await launchOwnedBrowser(chromium,{headless:true,
    executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=metal','--ignore-gpu-blocklist']});
  const {browser}=owned;
  const context=await browser.newContext({viewport:{width:1600,height:900},
    ignoreHTTPSErrors:process.env.LOCAL_TLS==='1'});
  page=await context.newPage();
  observed=ui.watchErrors(page);
  page.on('websocket',socket=>{
    const read=createNativeMessageReader();
    socket.on('framereceived',({payload})=>{
      try {
        const message=read(JSON.parse(payload.toString()));
        if(!message)return;
        if(message.type==='welcome'){
          accepted.id=message.credentials.player_id;accepted.frame=message.frame;
        }else if(message.type==='snapshot')accepted.frame=message.frame;
        else if(message.type==='boneyard_loaded')accepted.loaded=message.boneyard;
        else if(message.type==='hosted'&&message.message.kind==='save_checkpoint')
          accepted.checkpoint=message.message.save;
        else if(message.type==='error')observed.errors.push({type:'native',message:message.reason});
      }catch(error){observed.errors.push({type:'native-observation',message:error.message});}
    });
    socket.on('framesent',({payload})=>{
      const message=JSON.parse(payload.toString());
      if(message.type==='command')receipt.commands.push({tick:accepted.frame?.snapshot.ticks,
        command:message.command});
    });
  });
  const seen=new Set();
  const project=target=>{
    const scene=accepted.loaded.scene.bounds;
    const logical=viewports.boundedGameViewportLayout(1600,900,
      {width:scene.width,height:scene.height},cameras.BONEYARD_CAMERA_ZOOM);
    const bounds=tutorial.nativeTutorialCameraBounds({
      cameraLockTriggered:world().tutorial.camera_lock_triggered,
      cameraLockAgeTicks:world().tutorial.camera_lock_age_ticks,
    })||{x:scene.x,y:scene.y,w:scene.width,h:scene.height};
    const camera=cameras.boneyardCamera(player().wizard.position,bounds,logical,
      cameras.BONEYARD_CAMERA_ZOOM*logical.worldZoom);
    return {x:800+(target.x-camera.x)*camera.zoom,y:450+(target.y-camera.y)*camera.zoom};
  };
  try {
    await observed.ready;
    await ui.observeGpu(page);await ui.observeAnnouncements(page);
    await page.goto(url.href,{waitUntil:'domcontentloaded'});await ui.ready(page);
    await ui.click(page,'prompt-primary');
    await until(()=>world()?.tutorial&&accepted.loaded,'real solo Tutorial native admission',120000);
    assert.equal(player().player.selection.name,'Sirmin');
    receipt.runId=world().run_id;receipt.seed=world().seed;
    assert.equal(accepted.loaded.source_sha256,world().source_sha256);
    await lesson('USE YOUR KEYBOARD\nTO MOVE THE WIZARD');
    await page.keyboard.press('Escape');await ui.surface(page,'pause');
    await until(()=>accepted.checkpoint,'real paused Tutorial checkpoint');
    await ui.click(page,'resume');
    await until(()=>!accepted.frame.host.pause&&!accepted.frame.host.resume_grace,
      'ordinary Tutorial resume');
    await walkSolomonEntrance(page,{document:accepted.checkpoint,
      snapshot:()=>accepted.frame.snapshot,playerId:()=>accepted.id,output});
    checked('real Title YES, retained Tutorial geometry and ordinary Solomon confrontation');
    const deadline=Date.now()+480000;
    while(world()?.tutorial&&Date.now()<deadline){
      const current=stage();
      if(!seen.has(current)){
        seen.add(current);
        receipt.stages.push({stage:current,tick:accepted.frame.snapshot.ticks,
          position:point(player().wizard.position),enemies:living().length});
      }
      const snapshot=await ui.readUi(page);
      if(snapshot.error)throw new Error(snapshot.error);
      if(ui.findWidget(snapshot,'picker:choice:0')){
        await release();await ui.click(page,'picker:choice:0');continue;
      }
      if(accepted.frame.host.pause||accepted.frame.host.resume_grace||accepted.frame.host.loading_run_id){
        await release();await page.waitForTimeout(40);continue;
      }
      if(current==='OpenInventory'){
        await lesson('ACCESS YOUR INVENTORY');await modal('Inventory');continue;
      }
      if(current==='OpenSkills'){
        await lesson('ACCESS YOUR SKILLS');await modal('Skills');continue;
      }
      if(current==='ClubEnemies'&&!receipt.clubLesson){
        await release();await lesson('WALK INTO ENEMIES TO CLUB THEM');receipt.clubLesson=true;
      }
      if(current==='ClearArcherWave'&&!receipt.selectorLesson){
        await release();
        assert.equal((await ui.widget(page,'tutorial:text:selected-first')).label,
          'click these icons to change your');
        assert.equal((await ui.widget(page,'tutorial:text:selected-second')).label,
          'primary attack or concentration');
        await page.screenshot({path:path.join(output,'selected-skills-callout.png')});
        await ui.click(page,'hud:skill-binding:12');await ui.widget(page,'panel:skill-selector');
        await ui.click(page,`panel:hud-select-skill:${player().player.book.primary_skill_id}`);
        await ui.noWidget(page,'panel:skill-selector');
        await ui.noWidget(page,'tutorial:text:selected-first');
        await until(()=>!accepted.frame.host.pause&&!accepted.frame.host.resume_grace,
          'actual primary selector acknowledgement');
        receipt.selectorLesson=true;
        checked('actual selected-skill callout and primary-selector acknowledgement');
      }
      if(current==='CastAcidRain'){
        await release();await lesson('A SECONDARY SPELL IS READY');
        await page.keyboard.press('Space');
        await until(()=>stage()!=='CastAcidRain','accepted Tutorial Acid Rain');
        checked('real secondary input acknowledges Acid Rain and starts the item wave');continue;
      }
      if(current==='DrinkPotion'){
        await release();await lesson('DRINK POTION');
        assert.ok(player().player.economy.backpack.some(item=>item.native_type_id===7001
          &&item.native_subtype===0),'the authored potion was genuinely collected');
        await page.keyboard.press('KeyQ');
        await until(()=>stage()!=='DrinkPotion','accepted Tutorial Health Potion drink');
        checked('authored potion pickup, exact lesson and real Q acknowledgement');continue;
      }
      if(current==='Survive'||player().player.combat.life_state!=='Alive'){
        await release();
        const gameOver=ui.findWidget(snapshot,'game-over:continue');
        if(gameOver&&!receipt.gameOver){
          receipt.gameOver={tick:accepted.frame.snapshot.ticks,widget:gameOver,
            phase:accepted.frame.snapshot.run.phase};
          await page.screenshot({path:path.join(output,'game-over.png')});
        }
      }else if(['CollectAmulet','CollectPotion','ClubEnemies'].includes(current)){
        if(primary){await page.mouse.up();primary=false;}
        const targets=current==='ClubEnemies'?living():world().loot.actors.filter(actor=>actor.kind==='Sack');
        const target=targets.toSorted((a,b)=>distance(a.position,player().wizard.position)
          -distance(b.position,player().wizard.position))[0];
        if(target)await moveTo(target.position);else await keys([]);
      }else if(['ThrowMissiles','ClearSkeletons','ClearItemWave','ClearArcherWave'].includes(current)){
        if(current==='ThrowMissiles'&&!receipt.primaryLesson){
          await lesson('POINT AND CLICK YOUR MOUSE\nTO THROW MAGIC MISSILES');receipt.primaryLesson=true;
        }
        const target=living().toSorted((a,b)=>distance(a.position,player().wizard.position)
          -distance(b.position,player().wizard.position))[0];
        if(target){
          const aim=project(target.position);
          if(aim.x<10||aim.x>1590||aim.y<10||aim.y>890)await moveTo(target.position);
          else{
            await keys([]);
            if(!primary){await page.mouse.move(800,450);await page.mouse.down();primary=true;}
            await page.mouse.move(Math.max(10,Math.min(1590,aim.x)),Math.max(10,Math.min(890,aim.y)));
          }
        }else await release();
      }else await release();
      if(observed.errors.length){await ui.settleErrors(page);assert.deepEqual(observed.errors,[]);}
      await page.waitForTimeout(40);
    }
    await release();
    assert.equal(world(),null,'ordinary survival/death automatically returns to the College');
    assert.ok(receipt.gameOver,'the genuine terminal run painted its Game Over surface');
    for(const expected of ['ThrowMissiles','CastAcidRain','CollectAmulet','OpenInventory',
      'ClubEnemies','OpenSkills','ClearArcherWave','CollectPotion','DrinkPotion','Survive'])
      assert.ok(seen.has(expected),`the actual Tutorial passed ${expected}`);
    await reachCollegeLoadout(page);
    await ui.surface(page,'create');
    await page.screenshot({path:path.join(output,'post-tutorial-create.png')});
    checked('real survival/death, automatic College story Office and ordinary Create admission');
    receipt.announcements=await ui.announcements(page);
    await ui.assertGpuOnly(page);await ui.settleErrors(page);
    assert.deepEqual(observed.errors,[]);receipt.passed=true;
  }catch(error){
    receipt.failure=error.stack;receipt.ui=await ui.readUi(page);
    receipt.actual={stage:stage(),tick:accepted.frame?.snapshot.ticks,
      position:player()?.wizard.position,life:player()?.player.combat.life_state,
      phase:accepted.frame?.snapshot.run.phase,enemies:living().length};
    await page.screenshot({path:path.join(output,'failure.png')});
    process.exitCode=1;
  }finally{
    try{await release();await ui.settleErrors(page);}
    catch(error){receipt.passed=false;receipt.failure??=error.stack;process.exitCode=1;}
    receipt.errors=observed.errors;
    try{receipt.cleanup=await owned.close();}
    catch(error){receipt.passed=false;receipt.cleanup=error.cleanup||{error:error.stack};process.exitCode=1;}
    fs.writeFileSync(path.join(output,'receipt.json'),JSON.stringify(receipt,null,2));
  }
  console.log(JSON.stringify({passed:receipt.passed,checks:receipt.checks,errors:receipt.errors,
    failure:receipt.failure}));
})().catch(error=>{console.error(error);process.exitCode=1;});
