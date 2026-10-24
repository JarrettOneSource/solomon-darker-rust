// Ordinary Title YES through the real Tutorial combat, books, potion and return.
// Native frames are observations. Source camera/navigation only choose real input;
// no planner values, commands or checkpoints are installed into the game.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {pathToFileURL}=require('node:url');
const {createHash}=require('node:crypto');
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
const receipt={passed:false,url:url.href,artifact:process.env.ARTIFACT||null,
  scope:process.env.NATIVE_SAVE?'standalone-native-authority':'hosted-native-authority',
  checks:[],stages:[],lessons:[],commands:[],welcomes:[],mediaNetwork:[],errors:[]};
const accepted={id:null,frame:null,loaded:null,checkpoint:null,checkpointEvidence:null};
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
async function actualCheckpoint() {
  if(process.env.NATIVE_SAVE){
    let host,document;
    await until(()=>{
      if(!fs.existsSync(process.env.NATIVE_SAVE))return false;
      host=JSON.parse(fs.readFileSync(process.env.NATIVE_SAVE,'utf8'));
      document=JSON.parse(host.game_document);
      return document.checkpoint.boneyard?.run_id===world().run_id
        &&document.checkpoint.participants[accepted.id];
    },'the native authority durably checkpoints the actual Tutorial run');
    assert.equal(host.version,1);assert.equal(document.schema_version,2);
    const checkpoint=document.checkpoint;
    assert.equal(checkpoint.participants[accepted.id].player.selection.name,'Sirmin');
    assert.equal(checkpoint.boneyard.source_sha256,world().source_sha256);
    receipt.checkpoint={kind:'standalone-durable-file',hostVersion:host.version,
      schemaVersion:document.schema_version,revision:host.host.revision,
      tick:checkpoint.ticks,playerId:accepted.id,runId:checkpoint.boneyard.run_id,
      bytes:Buffer.byteLength(host.game_document),
      sha256:createHash('sha256').update(host.game_document).digest('hex')};
  }else{
    await until(()=>accepted.checkpoint,'real paused Tutorial owner checkpoint');
    receipt.checkpoint=accepted.checkpointEvidence;
  }
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
  const [tutorial,cameras,viewports,terminal,settings]=await Promise.all([
    source('core-kernels/native-tutorial.ts'),source('renderer/boneyard-render-contract.ts'),
    source('renderer/game-viewport.ts'),source('game-over-presentation.ts'),source('game-settings.ts'),
  ]);
  const controls=settings.DEFAULT_GAME_CONTROL_BINDINGS;
  const owned=await launchOwnedBrowser(chromium,{headless:true,
    executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=metal','--ignore-gpu-blocklist']});
  const {browser}=owned;
  const context=await browser.newContext({viewport:{width:1600,height:900},
    ignoreHTTPSErrors:process.env.LOCAL_TLS==='1'});
  page=await context.newPage();
  observed=ui.watchErrors(page);
  const network=await context.newCDPSession(page),mediaRequests=new Map();
  const header=(headers,name)=>Object.entries(headers)
    .find(([key])=>key.toLowerCase()===name)?.[1]??null;
  network.on('Network.requestWillBeSent',event=>{
    if(event.type!=='Media')return;
    const row={requestId:event.requestId,url:event.request.url,method:event.request.method,
      range:header(event.request.headers,'range'),started:event.timestamp,
      wallTime:event.wallTime,dataBytes:0,encodedBytes:0};
    mediaRequests.set(event.requestId,row);receipt.mediaNetwork.push(row);
  });
  network.on('Network.responseReceived',event=>{
    const row=mediaRequests.get(event.requestId);if(!row)return;
    const response=event.response;
    row.response={time:event.timestamp,status:response.status,connectionId:response.connectionId,
      contentLength:header(response.headers,'content-length'),
      contentRange:header(response.headers,'content-range'),
      contentType:header(response.headers,'content-type'),fromDiskCache:response.fromDiskCache};
  });
  network.on('Network.dataReceived',event=>{
    const row=mediaRequests.get(event.requestId);if(!row)return;
    row.dataBytes+=event.dataLength;row.encodedBytes+=event.encodedDataLength;
    row.lastData=event.timestamp;
  });
  network.on('Network.loadingFinished',event=>{
    const row=mediaRequests.get(event.requestId);
    if(row)row.finished={time:event.timestamp,encodedBytes:event.encodedDataLength};
  });
  network.on('Network.loadingFailed',event=>{
    const row=mediaRequests.get(event.requestId);
    if(row)row.failed={time:event.timestamp,error:event.errorText,canceled:event.canceled};
  });
  await network.send('Network.enable');
  page.on('websocket',socket=>{
    const read=createNativeMessageReader();
    socket.on('framereceived',({payload})=>{
      try {
        const message=read(JSON.parse(payload.toString()));
        if(!message)return;
        if(message.type==='welcome'){
          accepted.id=message.credentials.player_id;accepted.frame=message.frame;
          receipt.welcomes.push({playerId:accepted.id,tick:message.frame.snapshot.ticks,
            runId:message.frame.snapshot.boneyard?.run_id??null});
        }else if(message.type==='snapshot')accepted.frame=message.frame;
        else if(message.type==='boneyard_loaded')accepted.loaded=message.boneyard;
        else if(message.type==='hosted'&&message.message.kind==='save_checkpoint'){
          accepted.checkpoint=message.message.save;
          accepted.checkpointEvidence={kind:'hosted-owner-message',sequence:message.message.sequence,
            reason:message.message.reason,bytes:Buffer.byteLength(message.message.save),
            sha256:createHash('sha256').update(message.message.save).digest('hex')};
        }
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
    receipt.admission={playerId:accepted.id,tick:accepted.frame.snapshot.ticks,
      runId:world().run_id,sourceSha256:world().source_sha256,
      geometrySha256:world().geometry_sha256,selection:player().player.selection,stage:stage()};
    assert.equal(accepted.loaded.source_sha256,world().source_sha256);
    await lesson('USE YOUR KEYBOARD\nTO MOVE THE WIZARD');
    const beforeEscape=await ui.readUi(page);
    await page.keyboard.press('Escape');
    const afterEscape=await ui.waitForUi(page,snapshot=>snapshot.frame>beforeEscape.frame+1,
      'the actual early Tutorial Escape frame');
    assert.equal(afterEscape.surface,'hidden','Source seals Pause before combat access');
    assert.ok(!accepted.frame.host.pause,'early Escape does not pause the native Tutorial');
    receipt.earlyEscape={beforeFrame:beforeEscape.frame,afterFrame:afterEscape.frame,
      surface:afterEscape.surface,stage:stage()};
    await actualCheckpoint();
    await walkSolomonEntrance(page,{snapshot:()=>accepted.frame.snapshot,playerId:()=>accepted.id,output});
    checked('real Title YES, Source-gated early Escape, durable checkpoint and ordinary Solomon confrontation');
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
        await release();await ui.click(page,'picker:choice:0');
        await ui.noWidget(page,'picker:choice:0');continue;
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
        assert.equal(controls.belt1,'Mouse2');
        const hint=await ui.widget(page,'tutorial:text:subheading');
        assert.ok(hint.label.includes(settings.gameBindingLabel(controls.belt1)));
        receipt.secondaryInput={binding:controls.belt1,hint:hint.label};
        await page.mouse.move(800,450);
        await page.mouse.down({button:'right'});
        try{await until(()=>stage()!=='CastAcidRain','accepted Tutorial Acid Rain');}
        finally{await page.mouse.up({button:'right'});}
        checked('real Source secondary binding acknowledges Acid Rain and starts the item wave');continue;
      }
      if(current==='DrinkPotion'){
        await release();
        const potion=player().player.economy.backpack.find(item=>item.native_type_id===7001
          &&item.native_subtype===0);
        assert.ok(potion,'the authored potion was genuinely collected');
        const pickup=world().loot_events.findLast(event=>event.event_type==='LootPickup'
          &&event.player_id===accepted.id&&event.text===potion.name);
        assert.ok(pickup,'the actual local pickup, rather than an imported item, owns the notice');
        const id=`world-notification:loot:${pickup.event_id}`;
        const visible=await ui.waitForUi(page,snapshot=>{
          const notice=ui.findWidget(snapshot,id);
          return notice&&JSON.parse(notice.value).offset===0;
        },'the genuinely earned potion notice completes its native entrance');
        const notice=ui.findWidget(visible,id),presentation=JSON.parse(notice.value);
        assert.equal(notice.label,potion.name);
        assert.equal(presentation.baseline,67);assert.equal(presentation.clipTop,50);
        assert.equal(presentation.scale,1);
        await page.waitForFunction(name=>window.__solomonAnnouncements.some(event=>
          event.text.split('\n').includes(name)),potion.name);
        const speech=(await ui.announcements(page)).filter(event=>event.text.split('\n').includes(potion.name));
        assert.equal(speech.length,1,'one accepted earned loot event announces its actual item name once');
        receipt.earnedLoot={qualification:'genuine scripted Tutorial drop after ordinary combat and pickup; not a random Gold/book drop',
          pickup,item:{id:potion.id,name:potion.name,quantity:potion.quantity},notice,presentation,speech};
        await page.screenshot({path:path.join(output,'earned-tutorial-potion-notice.png')});
        await lesson('DRINK POTION');
        assert.equal(controls.belt4,'Digit3');
        const hint=await ui.widget(page,'tutorial:text:subheading');
        assert.ok(hint.label.includes(settings.gameBindingLabel(controls.belt4)));
        receipt.potionInput={binding:controls.belt4,hint:hint.label};
        await page.keyboard.down(controls.belt4);
        try{await until(()=>stage()!=='DrinkPotion','accepted Tutorial Health Potion drink');}
        finally{await page.keyboard.up(controls.belt4);}
        checked('authored potion pickup, exact lesson and real Source potion binding acknowledgement');continue;
      }
      if(current==='Survive'||player().player.combat.life_state!=='Alive'){
        await release();
        const gameOver=ui.findWidget(snapshot,'game-over:continue');
        const run=accepted.frame.snapshot.run;
        const presentation=terminal.gameOverPresentation(run.game_over_ticks,
          run.game_over_exit_ticks,run.game_over_exit_kind?.toLowerCase()??null);
        if(gameOver?.enabled&&presentation.titleAlpha===1&&presentation.promptAlpha===1
          &&!receipt.gameOver){
          receipt.gameOver={tick:accepted.frame.snapshot.ticks,widget:gameOver,
            phase:run.phase,presentation};
          await page.screenshot({path:path.join(output,'game-over.png')});
        }
      }else if(['CollectAmulet','CollectPotion','ClubEnemies'].includes(current)){
        if(primary){await page.mouse.up();primary=false;}
        const targets=current==='ClubEnemies'?living():world().loot.actors.filter(actor=>actor.kind==='Sack');
        const target=targets.toSorted((a,b)=>distance(a.position,player().wizard.position)
          -distance(b.position,player().wizard.position))[0];
        if(target)await moveTo(target.position);else await keys([]);
      }else if(['ThrowMissiles','ClearSkeletons','ClearItemWave','ClearArcherWave',
        'AwaitArcherWave','WaitForPotionDrop'].includes(current)){
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
