// Controlled private UI prerequisites, then real spell reservation and lethal
// enemy damage. No save, event, socket message or game-state injection.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {pathToFileURL}=require('node:url');
const {createHash}=require('node:crypto');
const {chromium,request}=require(path.resolve(process.env.PW));
const ui=require('./gpu-ui.cjs');
const {launchOwnedBrowser}=require('./browser-lifecycle.cjs');
const {createNativeMessageReader}=require('./native-wire.cjs');
const {startNewGame}=require('./menu-journey.cjs');
const {walkSolomonEntrance}=require('./solomon-entrance.cjs');
const {assertWorldNotification}=require('./world-notifications.cjs');
const output=path.resolve(process.env.OUT||'/tmp/world-notification-events');
const url=new URL(process.env.URL||'https://127.0.0.1:9449/game-stg/');
assert.ok(['127.0.0.1','localhost'].includes(url.hostname),'use the isolated owned Website');
const account=JSON.parse(fs.readFileSync(process.env.ACCOUNTS,'utf8'))[0];
assert.equal(account.user.id,3,'the existing QA3 developer capability; roles are not changed');
const mount=url.pathname.replace(/\/$/,''),tokenKey=mount?`sdr.token:${mount}`:'sdr.token';
const sourceRoot=path.resolve(process.env.SOLOMON_WEBSITE_DIR||path.join(__dirname,'../../../Website'));
const source=file=>import(pathToFileURL(path.join(sourceRoot,'frontend/src/game',file)).href);
const receipt={passed:false,scope:'controlled private UI prerequisites; not natural earned Gold',
  url:url.href,checks:[],commands:[],requests:[],welcomes:[],errors:[],consoleUsed:false};
const accepted={frame:null,id:null,loaded:null,lua:[],checkpoints:new Map(),leaveRequests:[],leaveReceipts:[]};
let page,observed,owner,admitted=false,left=false;
const player=()=>accepted.frame?.snapshot.participants[accepted.id]?.player;
const world=()=>accepted.frame?.snapshot.boneyard;
const secondary=()=>accepted.frame?.snapshot.secondary.players[accepted.id];

async function until(predicate,label,timeout=30000){
  const deadline=Date.now()+timeout;
  do{
    if(observed?.errors.length){await ui.settleErrors(page);assert.deepEqual(observed.errors,[],label);}
    if(await predicate())return;
    await page.waitForTimeout(40);
  }while(Date.now()<deadline);
  throw new Error(`Timed out waiting for ${label}`);
}
function checked(label){receipt.checks.push(label);console.log(label);}
async function inputReady(){
  await until(()=>player()&&!accepted.frame.host.loading_run_id&&!accepted.frame.host.resume_grace,
    'accepted local participant and renderer admission',120000);
  return ui.waitForUi(page,state=>state.surface==='hidden'&&!state.inputScope&&!state.capture.text
    &&ui.findWidget(state,'hud:health'),'actual unowned gameplay input');
}
async function reveal(id){
  await ui.wheel(page,'cheats/dialog',0,-10000);await ui.nextFrame(page);
  for(let index=0;index<12;index++){
    const row=ui.findWidget(await ui.readUi(page),id);
    if(row&&Math.abs(row.rect[3]-row.paintRect[3])<.5)return row;
    await ui.wheel(page,'cheats/dialog',0,250);await ui.nextFrame(page);
  }
  throw new Error(`real Cheats wheel scrolling did not reveal ${id}`);
}
async function chooseSkill(id){
  await reveal('cheats/skill');await ui.click(page,'cheats/skill');
  const target=`cheats/skill/option:${id}`;
  for(let index=0;index<14;index++){
    const state=await ui.readUi(page),row=ui.findWidget(state,target);
    if(row&&Math.abs(row.rect[3]-row.paintRect[3])<.5){
      await ui.click(page,target);await ui.value(page,'cheats/skill',String(id));return;
    }
    const visible=state.widgets.filter(row=>row.id.startsWith('cheats/skill/option:'));
    assert.ok(visible.length,'the actual native skill list is open');
    const first=visible[0],firstId=Number(first.id.split(':').at(-1));
    await ui.wheel(page,first.id,0,id<firstId?-220:220);await ui.nextFrame(page);
  }
  throw new Error(`real skill-list scrolling did not reveal ${id}`);
}
async function grantSkill(id){
  const before=player().book.permanent_ranks[id];
  if(before>0){receipt.checks.push(`prerequisite skill${id} already learned by the actual new wizard`);return;}
  await chooseSkill(id);await reveal('cheats/ranks');await ui.fill(page,'cheats/ranks','1');
  await reveal('cheats/grant-skill');const beforeResult=accepted.lua.length;
  await ui.click(page,'cheats/grant-skill');
  await until(()=>player().book.permanent_ranks[id]>before&&accepted.lua.length>beforeResult,
    `actual developer grant control accepts skill${id}`);
  assert.equal(accepted.lua.at(-1).ok,true);
  (receipt.prerequisiteSkills??=[]).push({id,before,after:player().book.permanent_ranks[id]});
}
async function closeCheats(){await page.keyboard.press('Escape');await ui.noWidget(page,'cheats/dialog');await inputReady();}
async function saveAndLeave(){
  const before=accepted.leaveReceipts.length,beforeRequests=accepted.leaveRequests.length;
  await page.keyboard.press('Escape');await ui.surface(page,'pause');await ui.click(page,'leave');
  await ui.surface(page,'title');
  await until(()=>accepted.leaveReceipts.length>before,'real signed owner receipt before Leave');
  const leave=accepted.leaveReceipts.at(-1);
  assert.ok(accepted.leaveRequests.slice(beforeRequests).includes(leave.request_id));
  assert.ok(leave.checkpoint_sequence>0);
  const checkpoint=accepted.checkpoints.get(leave.checkpoint_sequence);
  assert.ok(checkpoint,'the actual leave receipt names a retained signed checkpoint');
  left=true;receipt.savedLeave={receipt:leave,checkpoint,
    scope:'actual coordinator saved Leave; an already committed checkpoint may satisfy persistence'};
}
function observe(){
  page.on('response',response=>{
    const pathname=new URL(response.url()).pathname;
    if(pathname.includes('/api/'))receipt.requests.push({path:pathname,method:response.request().method(),status:response.status()});
  });
  page.on('websocket',socket=>{
    const read=createNativeMessageReader();
    socket.on('framereceived',({payload})=>{
      try{
        const message=read(JSON.parse(payload.toString()));if(!message)return;
        if(message.type==='welcome'){
          accepted.id=message.credentials.player_id;accepted.frame=message.frame;
          const row={playerId:accepted.id,sessionKind:message.hosted?.session_kind,
            cheats:message.hosted?.cheats_enabled,developer:message.hosted?.developer_access};
          receipt.welcomes.push(row);
        }else if(message.type==='snapshot')accepted.frame=message.frame;
        else if(message.type==='boneyard_loaded')accepted.loaded=message.boneyard;
        else if(message.type==='error')observed.errors.push({type:'native',message:message.reason});
        else if(message.type==='hosted'&&message.message.kind==='lua_result')
          accepted.lua.push({requestId:message.message.request_id,ok:message.message.ok,error:message.message.error});
        else if(message.type==='hosted'&&message.message.kind==='save_checkpoint'){
          const event=message.message;
          accepted.checkpoints.set(event.sequence,{sequence:event.sequence,reason:event.reason,
            bytes:Buffer.byteLength(event.save),sha256:createHash('sha256').update(event.save).digest('hex')});
        }else if(message.type==='hosted'&&message.message.kind==='save_before_leave')
          accepted.leaveReceipts.push(message.message);
      }catch(error){observed.errors.push({type:'native-observation',message:error.message});}
    });
    socket.on('framesent',({payload})=>{
      try{
        const message=JSON.parse(payload.toString());
        if(message.type==='hosted'&&message.command.kind==='lua_execute')
          receipt.commands.push({kind:'lua_execute',code:message.command.code});
        else if(message.type==='command')receipt.commands.push({kind:message.command.kind});
        else if(message.type==='save_before_leave')accepted.leaveRequests.push(message.request_id);
      }catch(error){observed.errors.push({type:'native-observation',message:error.message});}
    });
  });
}
(async()=>{
  fs.mkdirSync(output,{recursive:true});
  const api=await request.newContext({ignoreHTTPSErrors:process.env.LOCAL_TLS==='1',
    extraHTTPHeaders:{Authorization:`Bearer ${account.token}`}});
  try{
    const response=await api.get(new URL('api/auth/me',url).href);assert.equal(response.status(),200);
    const data=await response.json(),user=data.user||data;
    assert.equal(user.id,3);assert.equal(user.developerAccess,true);
    receipt.account={userId:user.id,developer:true};
  }finally{await api.dispose();}
  const {repeatedFloatDecay}=await source('renderer/native-secondary-draws.ts');
  owner=await launchOwnedBrowser(chromium,{headless:true,executablePath:process.env.CHROME,
    args:['--use-angle=metal','--ignore-gpu-blocklist']});
  const context=await owner.browser.newContext({viewport:{width:1600,height:900},ignoreHTTPSErrors:process.env.LOCAL_TLS==='1'});
  await context.addInitScript(({key,token})=>localStorage.setItem(key,token),{key:tokenKey,token:account.token});
  page=await context.newPage();observed=ui.watchErrors(page);observe();
  try{
    await observed.ready;await ui.observeGpu(page);await ui.observeAnnouncements(page);
    await page.goto(url.href,{waitUntil:'domcontentloaded'});await ui.ready(page);
    if(ui.findWidget(await ui.readUi(page),'prompt-secondary'))await ui.click(page,'prompt-secondary');
    await ui.click(page,'settings');await ui.click(page,'settings-page:cloud');
    if(!(await ui.widget(page,'settings-toggle:cheats')).selected)await ui.click(page,'settings-toggle:cheats');
    if((await ui.widget(page,'settings-toggle:shared-hub')).selected)await ui.click(page,'settings-toggle:shared-hub');
    await ui.click(page,'settings-back');await ui.click(page,'settings-back');
    await startNewGame(page,{name:'NoticeQA',element:'fire',discipline:'body'});admitted=true;await inputReady();
    const welcome=receipt.welcomes.at(-1);
    assert.equal(welcome.sessionKind,'PrivateHub');assert.equal(welcome.cheats,true);assert.equal(welcome.developer,true);
    await page.keyboard.press('Backquote');await ui.widget(page,'cheats/dialog');
    await ui.fill(page,'cheats/gold','7000');await ui.click(page,'cheats/submit:gold');
    await until(()=>player().economy.gold===7000,'actual private UI prerequisite funds');
    receipt.prerequisiteGold={source:'actual private SET GOLD control; not earned',accepted:player().economy.gold};
    for(const id of [23,78])await grantSkill(id);
    await closeCheats();checked('real private developer admission and visible prerequisite controls');
    const hagatha=await ui.byLabel(page,'Hagatha',{prefix:'hud:shortcut:',role:'button'});
    await ui.click(page,hagatha.id);await ui.widget(page,'panel:service');
    assert.equal(player().progression.hagatha_runtime.cheat_death_charges,0);
    await ui.click(page,'panel:hagatha:7');await ui.click(page,'panel:hagatha:7');
    await until(()=>player().progression.hagatha_runtime.cheat_death_charges===1,'real purchased Cheat Death charge');
    assert.equal(player().economy.gold,2000);
    receipt.purchase={selector:7,price:5000,gold:player().economy.gold,charges:player().progression.hagatha_runtime.cheat_death_charges};
    await page.screenshot({path:path.join(output,'actual-cheat-death-purchase.png')});
    await ui.click(page,'panel:close');await ui.noWidget(page,'panel:service');await inputReady();
    await page.keyboard.press('KeyK');await ui.widget(page,'panel:surface:skills');
    for(const [id,slot]of [[23,0],[78,1]]){
      await ui.drag(page,`panel:skill:${id}`,`panel:belt:${slot}`);
      await until(()=>player().belt.entries[slot]?.Skill?.skill_id===id,`actual skill${id} drag binding`);
    }
    await page.keyboard.press('KeyK');await ui.noWidget(page,'panel:surface:skills');await inputReady();
    checked('real Hagatha purchase and actual skill-book belt binding');
    await ui.click(page,'hud:map');
    await until(()=>world()&&accepted.loaded&&world().run_id===accepted.loaded.run_id,
      'real ordinary Map Boneyard admission',120000);await inputReady();
    receipt.world={runId:world().run_id,sourceSha256:world().source_sha256,geometrySha256:world().geometry_sha256};
    const combatBefore=Math.max(0,...world().enemy_events.map(event=>event.event_id));
    const rescueSpeechBefore=(await ui.announcements(page)).length;
    let confronted=false;
    if(!JSON.parse((await ui.widget(page,'hud:belt:0')).value).castEligible){
      await walkSolomonEntrance(page,{snapshot:()=>accepted.frame.snapshot,playerId:()=>accepted.id,output});confronted=true;
    }
    const firstSecondary=accepted.frame.snapshot.secondary.next_event_id;
    const overload=()=>accepted.frame.snapshot.secondary.events.find(event=>event.event_id>=firstSecondary
      &&event.kind==='Overload'&&event.owner_id===accepted.id&&event.world_key===`boneyard:${world().run_id}`);
    const overloadSpeechBefore=(await ui.announcements(page)).length;
    await page.mouse.move(800,450);await page.mouse.down({button:'right'});
    try{await until(()=>secondary()?.firewalker||overload(),'actual Firewalker toggle');}
    finally{await page.mouse.up({button:'right'});}
    if(!overload()){
      receipt.beforeOverload={maximumMana:player().combat.maximum_mana,reservedMana:secondary().reserved_mana,
        firewalker:secondary().firewalker};
      assert.ok(50+.6*player().combat.maximum_mana>player().combat.maximum_mana,'Source rank-one reservations exceed actual maximum');
      await page.keyboard.down('Digit1');
      try{await until(()=>overload(),'actual Mindstar over-reservation');}
      finally{await page.keyboard.up('Digit1');}
    }
    const overloaded=overload();
    assert.equal(secondary().firewalker,false);assert.equal(secondary().mindstar,false);assert.equal(secondary().reserved_mana,0);
    receipt.overload={event:overloaded,visual:await assertWorldNotification(page,{key:`secondary:${overloaded.event_id}`,
      text:'Overloaded Mana!',afterAnnouncement:overloadSpeechBefore})};
    await page.screenshot({path:path.join(output,'actual-overloaded-mana-notice.png')});
    checked('actual secondary over-reservation produces local GPU notice and one speech');
    if(!confronted)await walkSolomonEntrance(page,{snapshot:()=>accepted.frame.snapshot,playerId:()=>accepted.id,output});
    const rescue=()=>world().enemy_events.find(event=>event.event_id>combatBefore
      &&event.kind==='PlayerCheatDeath'&&event.target_player_id===accepted.id);
    await until(()=>rescue(),'actual lethal enemy hit consumes the purchased charm',120000);
    const event=rescue();assert.equal(player().progression.hagatha_runtime.cheat_death_charges,0);
    assert.equal(player().combat.life_state,'Alive');
    const visual=await assertWorldNotification(page,{key:`combat:${event.event_id}`,text:'CHEAT DEATH!',afterAnnouncement:rescueSpeechBefore});
    const flash=accepted.frame.snapshot.screen_flashes.writes.find(write=>write.tick===event.tick
      &&write.world_key===`boneyard:${world().run_id}`&&!write.only_if_clear&&write.flash.red===1
      &&write.flash.green===1&&write.flash.blue===1);
    assert.ok(flash,'the actual rescue owns its white native flash');
    await until(()=>repeatedFloatDecay(flash.flash.alpha,flash.flash.decay_per_tick,
      accepted.frame.snapshot.ticks-flash.tick)<=0,'the actual Source white flash finishes');
    await ui.nextFrame(page);
    receipt.cheatDeath={event,visual,chargesAfter:0,healthAfter:player().combat.current_health,
      maximumHealth:player().combat.maximum_health,flash};
    await page.screenshot({path:path.join(output,'actual-cheat-death-notice.png')});
    checked('actual lethal rescue consumes one real charm and produces local GPU notice and one speech');
    await ui.assertGpuOnly(page);await saveAndLeave();await ui.settleErrors(page);
    assert.deepEqual(observed.errors,[]);receipt.passed=true;
  }catch(error){
    receipt.failure=error.stack;receipt.ui=await ui.readUi(page);
    receipt.actual={tick:accepted.frame?.snapshot.ticks,life:player()?.combat.life_state,
      gold:player()?.economy.gold,world:world()?.run_id};
    await page.screenshot({path:path.join(output,'failure.png')});process.exitCode=1;
  }finally{
    try{
      if(admitted&&!left){
        const state=await ui.readUi(page);
        if(ui.findWidget(state,'cheats/dialog'))await closeCheats();
        if(ui.findWidget(await ui.readUi(page),'panel:close'))await ui.click(page,'panel:close');
        if((await ui.readUi(page)).surface==='hidden'&&player()?.combat.life_state==='Alive')await saveAndLeave();
      }
      await ui.settleErrors(page);
    }catch(error){receipt.passed=false;receipt.cleanupFailure=error.stack;process.exitCode=1;}
    receipt.errors=observed.errors;
    try{receipt.cleanup=await owner.close();}
    catch(error){receipt.passed=false;receipt.cleanup=error.cleanup||{error:error.stack};process.exitCode=1;}
    if(receipt.errors.length)receipt.passed=false;
    fs.writeFileSync(path.join(output,'receipt.json'),JSON.stringify(receipt,null,2));
  }
  console.log(JSON.stringify({passed:receipt.passed,checks:receipt.checks,errors:receipt.errors,failure:receipt.failure}));
})().catch(error=>{console.error(error);process.exitCode=1;});
