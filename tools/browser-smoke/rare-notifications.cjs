// Private developer-UI prerequisites, actual belt toggles, a paid Hagatha charm
// and lethal enemy damage. Observers never fabricate state, saves or notices.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(process.env.PW);
const {createNativeMessageReader} = require('./native-wire.cjs');
const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
const {startNewGame} = require('./menu-journey.cjs');
const {walkSolomonEntrance} = require('./solomon-entrance.cjs');
const {revealSettingsControl} = require('./settings-journey.cjs');
const {assertWorldNotification} = require('./world-notifications.cjs');
const ui = require('./gpu-ui.cjs');

const output = path.resolve(process.env.OUT);
const url = process.env.URL || 'https://127.0.0.1:9449/game-stg/';
const account = JSON.parse(fs.readFileSync(process.env.ACCOUNT));
const receipt = {passed:false, artifact:process.env.ARTIFACT, checks:[], errors:[],
  qualification:'controlled private developer Console skill/funding prerequisites; actual purchases, belt input and enemy damage',
  commands:[], results:[], secondaryEvents:[], combatEvents:[], health:[]};
let page, observed, frame, playerId, step = 'private developer admission';
const actor = () => frame?.snapshot.participants[playerId];
const player = () => actor()?.player;
const secondary = () => frame?.snapshot.secondary.players[playerId];
const seenSecondary = new Set(), seenCombat = new Set();
async function until(predicate, label, timeout=30000) {
  const end = Date.now()+timeout;
  while(Date.now()<end) {
    assert.deepEqual(observed.errors,[]); assert.deepEqual(receipt.errors,[]);
    if(await predicate())return;
    await page.waitForTimeout(25);
  }
  throw new Error(`Timed out: ${label}`);
}
async function shot(name) { await page.screenshot({path:path.join(output,`${name}.png`)}); }
async function ready() {
  await until(()=>player()&&!frame.host.loading_run_id&&!frame.host.resume_grace&&!frame.host.pause,
    'the real host admits gameplay',120000);
  await ui.waitForUi(page,snapshot=>snapshot.surface==='hidden'&&!snapshot.inputScope
    &&!ui.findWidget(snapshot,'cheats/dialog')&&!snapshot.widgets.some(row=>row.id.startsWith('panel:')),
  'ordinary gameplay owns input');
}
function observe() {
  page.on('websocket',socket=>{
    const read=createNativeMessageReader();
    socket.on('framesent',({payload})=>{
      const row=JSON.parse(payload.toString());
      if(row.type==='hosted'&&row.command.kind==='lua_execute')receipt.commands.push(row.command);
      else if(row.type==='command')receipt.commands.push(row.command);
    });
    socket.on('framereceived',({payload})=>{
      const row=read(JSON.parse(payload.toString())); if(!row)return;
      if(row.type==='welcome') {
        playerId=row.credentials.player_id; frame=row.frame;
        receipt.welcome={playerId,sessionKind:row.hosted?.session_kind,
          developerAccess:row.hosted?.developer_access,cheatsEnabled:row.hosted?.cheats_enabled};
      } else if(row.type==='snapshot')frame=row.frame;
      else if(row.type==='error')receipt.errors.push({code:row.code,reason:row.reason});
      else if(row.type==='hosted'&&row.message.kind==='lua_result')receipt.results.push(row.message);
      if(row.type!=='snapshot'&&row.type!=='welcome')return;
      for(const event of frame.snapshot.secondary.events) {
        const key=`${event.world_key}:${event.event_id}`;
        if(event.owner_id===playerId&&!seenSecondary.has(key)) {
          seenSecondary.add(key);receipt.secondaryEvents.push(event);
        }
      }
      for(const event of frame.snapshot.boneyard?.enemy_events||[]) {
        const key=`${frame.snapshot.boneyard.run_id}:${event.event_id}`;
        if(event.target_player_id===playerId&&!seenCombat.has(key)) {
          seenCombat.add(key);receipt.combatEvents.push(event);
        }
      }
      const health={tick:frame.snapshot.ticks,health:player().combat.current_health,
        maximum:player().combat.maximum_health,life:player().combat.life_state,
        charge:player().progression.hagatha_runtime.cheat_death_charges};
      const previous=receipt.health.at(-1);
      if(!previous||health.health!==previous.health||health.life!==previous.life||health.charge!==previous.charge)
        receipt.health.push(health);
    });
  });
}
async function consoleRun(code) {
  await page.keyboard.press('Backquote'); await ui.click(page,'cheats/tab:console');
  await ui.fill(page,'cheats/lua-editor',code);
  const before=receipt.commands.length;
  await ui.click(page,'cheats/run');
  await until(()=>receipt.commands.slice(before).some(row=>row.code===code),'the real Console request');
  const request=receipt.commands.slice(before).find(row=>row.code===code);
  await until(()=>receipt.results.some(row=>row.request_id===request.request_id),'the real Console result');
  const result=receipt.results.find(row=>row.request_id===request.request_id);
  assert.equal(result.ok,true,result.error||'Console accepted');
  await page.keyboard.press('Backquote'); await ready();
  return {request,result};
}
async function pauseNotice(key,text,afterAnnouncement,name,{resume=true}={}) {
  const notice=await assertWorldNotification(page,{key,text,afterAnnouncement});
  await page.keyboard.press('Escape');await ui.surface(page,'pause');
  await until(()=>frame.host.pause,'actual native pause preserves the notification frame');
  await shot(name);
  if(resume){await ui.click(page,'resume');await ready();}
  return notice;
}

(async()=>{
  fs.mkdirSync(output,{recursive:true});
  const owner=await launchOwnedBrowser(chromium,{headless:true,executablePath:process.env.CHROME,
    args:['--use-angle=metal','--ignore-gpu-blocklist']});
  let failure;
  try {
    const context=await owner.browser.newContext({viewport:{width:1600,height:900},ignoreHTTPSErrors:true});
    await context.addInitScript(token=>localStorage.setItem('sdr.token:/game-stg',token),account.token);
    page=await context.newPage();observed=ui.watchErrors(page);await observed.ready;
    await ui.observeAnnouncements(page);observe();
    await page.goto(url);await ui.ready(page);
    const me=await context.request.get(new URL('api/auth/me',url).href,{headers:{Authorization:`Bearer ${account.token}`}});
    assert.equal(me.status(),200);assert.equal((await me.json()).user.developerAccess,true);
    if(ui.findWidget(await ui.readUi(page),'prompt-secondary'))await ui.click(page,'prompt-secondary');
    await ui.click(page,'settings');await ui.click(page,'settings-page:cloud');
    for(const key of ['shared-hub','cheats']) {
      if(key==='cheats')await revealSettingsControl(page,`settings-toggle:${key}`);
      if((await ui.widget(page,`settings-toggle:${key}`)).selected)await ui.click(page,`settings-toggle:${key}`);
      await ui.value(page,`settings-toggle:${key}`,'false');
    }
    await ui.click(page,'settings-back');await ui.click(page,'settings-back');await ui.surface(page,'title');
    await startNewGame(page,{name:'Antonia',element:'fire',discipline:'arcane',continueLocal:true});await ready();
    assert.equal(receipt.welcome.sessionKind,'PrivateCollege');assert.equal(receipt.welcome.developerAccess,true);

    step='paid Hagatha prerequisite';
    const beforeGold=player().economy.gold;
    await page.keyboard.press('Backquote');await ui.widget(page,'cheats/dialog');
    await ui.fill(page,'cheats/gold','10000');await ui.click(page,'cheats/submit:gold');
    await until(()=>player().economy.gold===10000,'real SET GOLD accepts the declared prerequisite');
    await ui.widget(page,'cheats/submit:gold',{enabled:true});
    receipt.funding={before:beforeGold,after:player().economy.gold,qualification:receipt.qualification,
      feedback:await ui.widget(page,'cheats/action-receipt')};
    await page.keyboard.press('Backquote');await ready();
    await ui.click(page,(await ui.byLabel(page,'Hagatha',{role:'button',prefix:'hud:shortcut:'})).id);
    assert.equal(player().economy.owned_perk_selectors.includes(7),false,'this wizard must buy the charge');
    await ui.click(page,'panel:hagatha:7');await ui.click(page,'panel:hagatha:7');
    await until(()=>player().economy.owned_perk_selectors.includes(7),'the real Hagatha purchase is accepted');
    assert.equal(player().economy.gold,5000);assert.equal(player().progression.hagatha_runtime.cheat_death_charges,1);
    receipt.charm={selector:7,price:5000,gold:player().economy.gold,runtime:structuredClone(player().progression.hagatha_runtime)};
    await shot('paid-cheat-death-charm');await ui.click(page,'panel:close');await ready();
    receipt.checks.push('actual SET GOLD prerequisite, paid Hagatha charm and one native charge');

    step='real secondary belt bindings';
    await ui.click(page,(await ui.byLabel(page,'Enter the Boneyard',{role:'button'})).id);
    await until(()=>frame?.snapshot.boneyard,'actual Boneyard admission');await ready();
    receipt.runId=frame.snapshot.boneyard.run_id;
    receipt.skills=await consoleRun('sd.dev.grant_skill(23,1); sd.dev.grant_skill(78,1); sd.dev.grant_skill(79,1); return true');
    for(const id of [23,78,79])assert.equal(player().book.permanent_ranks[id],1);
    await page.keyboard.press('KeyK');await ui.widget(page,'panel:surface:skills');
    for(const [slot,id] of [23,78,79].entries()) {
      await ui.drag(page,`panel:skill:${id}`,`panel:belt:${slot}`);
      await until(()=>player().belt.entries[slot]?.Skill?.skill_id===id,'actual Skillbook drag binds the skill');
    }
    await shot('actual-secondary-skillbook-bindings');
    receipt.belt=structuredClone(player().belt);
    await page.keyboard.press('Escape');await ui.noWidget(page,'panel:surface:skills');await ready();
    receipt.checks.push('three actual Console-granted skills bind through the visible Skillbook and belt');

    step='actual mana overload';
    const announcementStart=(await ui.announcements(page)).length;
    receipt.toggles=[];
    for(const [slot,name] of ['firewalker','mindstar','regenerate'].entries()) {
      await until(()=>!secondary()||secondary().global_cooldown_ticks<=0,'actual secondary cooldown completes');
      await ui.click(page,`hud:belt:${slot}`);
      if(slot<2)await until(()=>secondary()?.[name]===true,`real ${name} activates`);
      else await until(()=>receipt.secondaryEvents.some(event=>event.kind==='Overload'),'native mana reservation emits overload');
      receipt.toggles.push({slot,name,tick:frame.snapshot.ticks,maximumMana:player().combat.maximum_mana,
        state:structuredClone(secondary())});
    }
    const overload=receipt.secondaryEvents.find(event=>event.kind==='Overload');
    assert.equal(overload.world_key,`boneyard:${receipt.runId}`);assert.equal(overload.owner_id,playerId);
    assert.ok(['firewalker','mindstar','regenerate'].every(name=>secondary()[name]===false));
    receipt.overload={event:overload,notice:await pauseNotice(`secondary:${overload.event_id}`,
      'Overloaded Mana!',announcementStart,'actual-overloaded-mana')};
    receipt.checks.push('real belt reservations overload, clear toggles and paint/announce the accepted notice once');

    step='actual lethal enemy damage';
    await walkSolomonEntrance(page,{snapshot:()=>frame.snapshot,playerId:()=>playerId,output});await ready();
    assert.ok(frame.snapshot.boneyard.encounter.run_event_id>0);
    assert.equal(player().progression.hagatha_runtime.cheat_death_charges,1);
    const deathAnnouncementStart=(await ui.announcements(page)).length;
    const at=actor().wizard.position;
    receipt.enemies=await consoleRun(`sd.enemies.spawn("skeleton",{x=${at.x}+20,y=${at.y}}); return true`);
    await until(()=>receipt.combatEvents.some(event=>event.kind==='PlayerCheatDeath'),'actual enemy damage consumes the purchased charge');
    const death=receipt.combatEvents.find(event=>event.kind==='PlayerCheatDeath');
    assert.equal(player().progression.hagatha_runtime.cheat_death_charges,0);
    assert.equal(player().combat.life_state,'Alive');
    receipt.cheatDeath={event:death,notice:await pauseNotice(`combat:${death.event_id}`,
      'CHEAT DEATH!',deathAnnouncementStart,'actual-cheat-death',{resume:false})};
    receipt.checks.push('lethal enemy damage consumes the paid charge and paints/announces CHEAT DEATH once');
    await ui.click(page,'leave');await ui.surface(page,'title');
    await ui.assertGpuOnly(page);await ui.settleErrors(page);assert.deepEqual(observed.errors,[]);assert.deepEqual(receipt.errors,[]);
    receipt.passed=true;
  } catch(error) {
    failure=error;receipt.failure=error.stack;receipt.step=step;
    receipt.failedPlayer=player();receipt.failedSecondary=secondary();
    if(page){receipt.failedUi=await ui.readUi(page).catch(error=>({error:error.message}));await shot('failure').catch(()=>{});}
  } finally {
    if(page)try{await ui.settleErrors(page);assert.deepEqual(observed.errors,[]);assert.deepEqual(receipt.errors,[]);}
    catch(error){failure||=error;receipt.passed=false;receipt.observerFailure=error.stack;}
    receipt.browserErrors=observed?.errors||[];
    try{receipt.cleanup=await owner.close();}catch(error){failure||=error;receipt.passed=false;receipt.cleanup=error.cleanup;}
    fs.writeFileSync(path.join(output,'receipt.json'),JSON.stringify(receipt,null,2));
  }
  console.log(JSON.stringify({passed:receipt.passed,step,checks:receipt.checks,cleanup:receipt.cleanup}));
  if(failure)throw failure;
})().catch(error=>{console.error(error);process.exitCode=1;});
