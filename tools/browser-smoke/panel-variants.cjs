// Real paid clothing, native dye transactions, ranked/welded pages and all belt
// banks. Private Console prerequisites are explicit; snapshots only observe.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {pathToFileURL} = require('node:url');
const {chromium} = require(process.env.PW);
const {createNativeMessageReader} = require('./native-wire.cjs');
const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
const {startNewGame} = require('./menu-journey.cjs');
const {revealSettingsControl} = require('./settings-journey.cjs');
const {checkNativeBeltPullOff} = require('./belt-pull-off-check.cjs');
const {walkSolomonEntrance} = require('./solomon-entrance.cjs');
const ui = require('./gpu-ui.cjs');

const output=path.resolve(process.env.OUT),url=process.env.URL;
const account=JSON.parse(fs.readFileSync(process.env.ACCOUNT));
const receipt={passed:false,artifact:process.env.ARTIFACT,checks:[],errors:[],commands:[],results:[],
  qualification:'controlled private SET GOLD and Console skills; ordinary purchases, drag, dye and save'};
let page,observed,frame,playerId,step='private admission';
const player=()=>frame?.snapshot.participants[playerId]?.player;
const item=id=>player().economy.backpack.find(row=>row.id===id);
async function until(predicate,label,timeout=30000) {
  const end=Date.now()+timeout;
  while(Date.now()<end){assert.deepEqual(observed.errors,[]);assert.deepEqual(receipt.errors,[]);
    if(await predicate())return;await page.waitForTimeout(25);}
  throw new Error(`Timed out: ${label}`);
}
async function shot(name){await page.screenshot({path:path.join(output,`${name}.png`)});}
async function ready(){await until(()=>player()&&!frame.host.loading_run_id&&!frame.host.resume_grace&&!frame.host.pause,'host gameplay admission',120000);
  await ui.waitForUi(page,s=>s.surface==='hidden'&&!s.inputScope&&!ui.findWidget(s,'cheats/dialog'),'ordinary gameplay input');}
async function consoleRun(code){await page.keyboard.press('Backquote');await ui.click(page,'cheats/tab:console');
  await ui.fill(page,'cheats/lua-editor',code);const before=receipt.commands.length;await ui.click(page,'cheats/run');
  await until(()=>receipt.commands.slice(before).some(row=>row.code===code),'Console request');
  const request=receipt.commands.slice(before).find(row=>row.code===code);
  await until(()=>receipt.results.some(row=>row.request_id===request.request_id),'Console result');
  const result=receipt.results.find(row=>row.request_id===request.request_id);assert.equal(result.ok,true,result.error);
  await page.keyboard.press('Backquote');await ready();return {request,result};}
function observe(){page.on('websocket',socket=>{const read=createNativeMessageReader();
  socket.on('framesent',({payload})=>{const row=JSON.parse(payload.toString());
    if(row.type==='command')receipt.commands.push(row.command);
    else if(row.type==='hosted'&&row.command.kind==='lua_execute')receipt.commands.push(row.command);});
  socket.on('framereceived',({payload})=>{const row=read(JSON.parse(payload.toString()));if(!row)return;
    if(row.type==='welcome'){playerId=row.credentials.player_id;frame=row.frame;
      receipt.welcome={playerId,sessionKind:row.hosted?.session_kind,developerAccess:row.hosted?.developer_access};}
    else if(row.type==='snapshot')frame=row.frame;
    else if(row.type==='error')receipt.errors.push({code:row.code,reason:row.reason});
    else if(row.type==='hosted'&&row.message.kind==='lua_result')receipt.results.push(row.message);});});}
async function buy(stock){const gold=player().economy.gold,before=player().economy.backpack.map(row=>row.id);
  await ui.click(page,`panel:shop:${stock.item.id}`);await ui.click(page,`panel:shop:${stock.item.id}`);
  await until(()=>player().economy.gold===gold-stock.price,'actual shop payment');
  const purchased=player().economy.backpack.find(row=>!before.includes(row.id)&&row.native_type_id===stock.item.native_type_id);
  assert.ok(purchased,'the accepted purchase creates the owned item');return structuredClone(purchased);}
async function buyClothing(){
  await ui.click(page,(await ui.byLabel(page,'Shlorio',{role:'button',prefix:'hud:shortcut:'})).id);
  const panel=ui.bounds(await ui.widget(page,'panel:service'));
  const from=ui.center(ui.bounds(await ui.widget(page,'panel:equipment:hat')));
  // The native mirror center is the same real reference drop used by private-dowsing.cjs.
  const to={x:panel.x+panel.width*302/604,y:panel.y+panel.height*175.5/420};
  await page.mouse.move(from.x,from.y);await page.mouse.down();await page.mouse.move(to.x,to.y,{steps:15});await page.mouse.up();
  await ui.nextFrame(page);await ui.click(page,'panel:dowse');
  await until(()=>player().economy.dowsing_rolled,'actual owned-Hat reference roll');
  const offer=player().economy.dowsing_offers[0],gold=player().economy.gold;
  const before=new Set(player().economy.backpack.map(row=>row.id));assert.ok(offer);
  await ui.click(page,`panel:shop:${offer.id}`);await ui.click(page,`panel:shop:${offer.id}`);
  await until(()=>player().economy.gold===gold-offer.price,'actual clothing payment');
  const clothing=player().economy.backpack.find(row=>!before.has(row.id)&&row.recipe_index===offer.recipe_index);
  assert.equal(clothing?.equipment_type,'Hat');
  await ui.click(page,'panel:close');await ready();return structuredClone(clothing);
}

(async()=>{
  fs.mkdirSync(output,{recursive:true});
  const source=await import(pathToFileURL(path.join(process.env.SOLOMON_WEBSITE_DIR,'frontend/src/game/core-kernels/hub-economy.ts')).href);
  const owner=await launchOwnedBrowser(chromium,{headless:true,executablePath:process.env.CHROME,args:['--use-angle=metal','--ignore-gpu-blocklist']});
  let failure;
  try{
    const context=await owner.browser.newContext({viewport:{width:1600,height:900},ignoreHTTPSErrors:true});
    await context.addInitScript(token=>localStorage.setItem('sdr.token:/game-stg',token),account.token);
    page=await context.newPage();observed=ui.watchErrors(page);await observed.ready;observe();
    await page.goto(url);await ui.ready(page);
    if(ui.findWidget(await ui.readUi(page),'prompt-secondary'))await ui.click(page,'prompt-secondary');
    await ui.click(page,'settings');await ui.click(page,'settings-page:cloud');
    for(const key of ['shared-hub','cheats']){if(key==='cheats')await revealSettingsControl(page,`settings-toggle:${key}`);
      if((await ui.widget(page,`settings-toggle:${key}`)).selected)await ui.click(page,`settings-toggle:${key}`);
      await ui.value(page,`settings-toggle:${key}`,'false');}
    await ui.click(page,'settings-back');await ui.click(page,'settings-back');await ui.surface(page,'title');
    await startNewGame(page,{name:'Lucian',element:'air',discipline:'arcane',continueLocal:true});await ready();
    assert.equal(receipt.welcome.sessionKind,'PrivateCollege');assert.equal(receipt.welcome.developerAccess,true);
    await page.keyboard.press('Backquote');await ui.fill(page,'cheats/gold','20000');await ui.click(page,'cheats/submit:gold');
    await until(()=>player().economy.gold===20000,'declared SET GOLD prerequisite');
    await page.keyboard.press('Backquote');await ready();

    step='paid clothing and actual cloth/trim dye';
    const clothing=await buyClothing();
    await ui.click(page,(await ui.byLabel(page,'Fomentius',{role:'button',prefix:'hud:shortcut:'})).id);
    const stock=player().economy.fomentius_stock;
    const kit=stock.find(row=>row.item.kind==='Dye');
    assert.ok(kit,'Fomentius stocks the actual dye kit');
    receipt.purchases={clothing,dye:await buy(kit)};
    await ui.click(page,'panel:close');await ready();await page.keyboard.press('KeyI');
    const clothingId=receipt.purchases.clothing.id,dyeId=receipt.purchases.dye.id;
    await ui.click(page,`panel:item:${dyeId}`,{clickCount:2});await ui.widget(page,'panel:dye');
    assert.ok(item(dyeId),'opening dye does not consume the kit before a committed paint');
    const swatches=[0,7];for(const row of swatches)await ui.click(page,`panel:dye-swatch:${row}`);
    await ui.click(page,`panel:item:${clothingId}`);await ui.click(page,'panel:dye-cloth');
    const cloth=source.nativeDyeCommittedTint(swatches);
    await until(()=>item(clothingId)?.icon_tints?.[0]===cloth,'accepted cloth color equals unmodified Source');
    assert.equal(item(dyeId),undefined,'first actual paint consumes exactly one kit');
    const afterCloth=structuredClone(item(clothingId));await shot('actual-dyed-cloth');
    await ui.click(page,'panel:dye-swatch:12');swatches.push(12);
    await ui.click(page,`panel:item:${clothingId}`);await ui.click(page,'panel:dye-trim');
    const trim=source.nativeDyeCommittedTint(swatches);
    await until(()=>item(clothingId)?.icon_tints?.[1]===trim,'accepted trim color equals unmodified Source');
    assert.equal(item(clothingId).icon_tints[0],cloth,'trim preserves committed cloth');
    receipt.dye={swatches,cloth,trim,afterCloth,afterTrim:structuredClone(item(clothingId))};
    await shot('actual-dyed-trim');await ui.click(page,'panel:dye-cancel');await ui.noWidget(page,'panel:dye');
    await page.keyboard.press('Escape');await ready();
    receipt.checks.push('real clothing/kit purchases, Source-exact cloth and trim, one consumed kit and normal cancel');

    step='ranked and welded Skillbook, all three banks';
    receipt.skills=await consoleRun('sd.dev.grant_skill(23,2); sd.dev.grant_weld(1003); return true');
    assert.equal(player().book.permanent_ranks[23],2);assert.equal(player().book.weld_build_id,1003);
    await page.keyboard.press('KeyK');await ui.widget(page,'panel:skill:52');
    assert.match((await ui.widget(page,'panel:skill:23')).label,/rank 2/);
    const weld=ui.center(ui.bounds(await ui.widget(page,'panel:skill:52')));await page.mouse.move(weld.x,weld.y);
    await ui.waitForUi(page,s=>ui.findWidget(s,'panel:tooltip')?.label.includes('FLAME LASH'),'actual welded hover text');
    await shot('ranked-and-welded-skillbook');receipt.bankBindings=[];
    for(const bank of [0,1,2]){await ui.drag(page,'panel:skill:23',`panel:belt:${bank*8}`);
      await until(()=>player().belt.entries[bank*8]?.Skill?.skill_id===23,'actual drag addresses the selected bank');
      receipt.bankBindings.push({bank,slot:bank*8,entry:structuredClone(player().belt.entries[bank*8])});
      await ui.click(page,'panel:hotbar:next');await ui.nextFrame(page);}
    await page.keyboard.press('Escape');await ready();
    receipt.pullOff=await checkNativeBeltPullOff(page,0);
    await until(()=>player().belt.entries[0]===null,'actual native pull-off unbinds the first bank');
    assert.equal(player().belt.entries[8].Skill.skill_id,23);assert.equal(player().belt.entries[16].Skill.skill_id,23);
    receipt.checks.push('ranked/welded pages and hover, all three bank bindings, exact pull-off boundary and other-bank retention');

    step='owned item drag and saved continuation';
    await page.keyboard.press('KeyI');await ui.widget(page,'panel:surface:inventory');
    await ui.click(page,'panel:hotbar:next');await ui.nextFrame(page);
    const mana=player().economy.backpack.find(row=>row.name==='Mana Potion');assert.ok(mana);
    await ui.drag(page,`panel:item:${mana.id}`,'panel:belt:9');
    await until(()=>player().belt.entries[9]?.ManaPotion!==undefined||player().belt.entries[9]==='ManaPotion','real item drag binds bank two');
    receipt.saved={belt:structuredClone(player().belt),clothing:structuredClone(item(clothingId))};
    await page.keyboard.press('Escape');await ready();await page.keyboard.press('Escape');await ui.surface(page,'pause');
    await ui.click(page,'leave');await ui.surface(page,'title');
    await page.reload();await ui.ready(page);await ui.click(page,'continue');await ready();
    assert.deepEqual(player().belt,receipt.saved.belt);assert.deepEqual(item(clothingId).icon_tints,receipt.saved.clothing.icon_tints);
    receipt.checks.push('owned potion drag into bank two and ordinary saved Leave/reload/Continue preserve all bindings and both colors');
    step='native skill activation from the second and third banks';
    await ui.click(page,'hud:hotbar:next');await ui.widget(page,'hud:belt:8');
    await ui.click(page,(await ui.byLabel(page,'Enter the Boneyard',{role:'button'})).id);
    await until(()=>frame?.snapshot.boneyard,'actual Boneyard admission');await ready();
    await walkSolomonEntrance(page,{snapshot:()=>frame.snapshot,playerId:()=>playerId,output});await ready();
    for(const [slot,expected] of [[8,true],[16,false]]){
      await until(()=>frame.snapshot.secondary.players[playerId]?.global_cooldown_ticks<=0,'native secondary cooldown');
      await ui.click(page,`hud:belt:${slot}`);
      await until(()=>frame.snapshot.secondary.players[playerId]?.firewalker===expected,'actual banked skill changes the native toggle');
      if(slot===8){await ui.click(page,'hud:hotbar:next');await ui.widget(page,'hud:belt:16');}
    }
    await shot('native-second-and-third-bank-casts');
    await page.keyboard.press('Escape');await ui.surface(page,'pause');await ui.click(page,'leave');await ui.surface(page,'title');
    await ui.assertGpuOnly(page);await ui.settleErrors(page);assert.deepEqual(observed.errors,[]);
    receipt.checks.push('second and third bank pointer activations toggle the real native Firewalker and save normally');
    receipt.passed=true;
  }catch(error){failure=error;receipt.failure=error.stack;receipt.step=step;receipt.failedPlayer=player();
    if(page){receipt.failedUi=await ui.readUi(page).catch(error=>({error:error.message}));await shot('failure').catch(()=>{});}}
  finally{if(page)try{await ui.settleErrors(page);assert.deepEqual(observed.errors,[]);assert.deepEqual(receipt.errors,[]);}
    catch(error){failure||=error;receipt.passed=false;receipt.observerFailure=error.stack;}
    receipt.browserErrors=observed?.errors||[];
    try{receipt.cleanup=await owner.close();}catch(error){failure||=error;receipt.passed=false;receipt.cleanup=error.cleanup;}
    fs.writeFileSync(path.join(output,'receipt.json'),JSON.stringify(receipt,null,2));}
  console.log(JSON.stringify({passed:receipt.passed,step,checks:receipt.checks,cleanup:receipt.cleanup}));
  if(failure)throw failure;
})().catch(error=>{console.error(error);process.exitCode=1;});
