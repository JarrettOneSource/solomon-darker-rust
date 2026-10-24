// Select the unchanged Source Apothecary boast and fail it by using an owned potion.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {chromium}=require(process.env.PW);
const {launchOwnedBrowser}=require('./browser-lifecycle.cjs');
const {startNewGame}=require('./menu-journey.cjs');
const {walkSolomonEntrance}=require('./solomon-entrance.cjs');
const {createNativeMessageReader}=require('./native-wire.cjs');
const ui=require('./gpu-ui.cjs');
const out=path.resolve(process.env.OUT),url=new URL(process.env.URL);
const mount=url.pathname.replace(/\/$/,'');
const account=JSON.parse(fs.readFileSync(path.join(process.env.CONTENT_DIR,'private-account.json')));
const content=JSON.parse(fs.readFileSync(path.join(process.env.CONTENT_DIR,'package.json')));
const boastName='EMPTY HANDS, FULL GLORY!';
const instruction='Survive through Wave 25 without breaking your boast.';
const receipt={passed:false,checks:[],commands:[],errors:[]};
let page,observed,frame,playerId,step='ordinary account sign-in';
const player=()=>frame?.snapshot.participants[playerId]?.player;
async function until(predicate,label,timeout=45000){const end=Date.now()+timeout;
  while(Date.now()<end){if(predicate())return;assert.deepEqual(receipt.errors,[]);await page.waitForTimeout(40);}
  throw new Error(`Timed out waiting for ${label}`);}
async function ready(){await until(()=>player()&&!frame.host.loading_run_id&&!frame.host.resume_grace,'native participant');
  await ui.waitForUi(page,s=>s.surface==='hidden'&&!s.inputScope,'actual gameplay input');}
async function shortcut(name){await ui.click(page,(await ui.byLabel(page,name,{role:'button',prefix:'hud:shortcut:'})).id);}
async function shot(name){await page.screenshot({path:path.join(out,`${name}.png`)});}

(async()=>{
  fs.mkdirSync(out,{recursive:true});
  const owner=await launchOwnedBrowser(chromium,{headless:true,executablePath:process.env.CHROME,
    args:['--use-angle=metal','--ignore-gpu-blocklist']});
  let failure;
  try{
    page=await owner.browser.newPage({ignoreHTTPSErrors:true,viewport:{width:1600,height:900}});
    observed=ui.watchErrors(page);await observed.ready;
    page.on('websocket',socket=>{const read=createNativeMessageReader();
      socket.on('framesent',({payload})=>{const message=JSON.parse(payload.toString());
        if(message.type==='command')receipt.commands.push({sequence:message.sequence,command:message.command});});
      socket.on('framereceived',({payload})=>{try{const message=read(JSON.parse(payload.toString()));
        if(message?.type==='welcome'){frame=message.frame;playerId=message.credentials.player_id;
          receipt.welcome={kind:message.hosted?.session_kind,cheats:message.hosted?.cheats_enabled};}
        else if(message?.type==='snapshot')frame=message.frame;
        else if(message?.type==='error')receipt.errors.push({code:message.code,reason:message.reason});
      }catch(error){receipt.errors.push(error.stack);}});});
    await page.goto(new URL(`${mount}/login`,url).href);
    await page.locator('input[autocomplete="username"]').fill(account.username);
    await page.locator('input[autocomplete="current-password"]').fill(account.password);
    const reads=Promise.all([
      page.waitForResponse(r=>new URL(r.url()).pathname===`${mount}/api/game/saves/0`&&r.ok()),
      page.waitForResponse(r=>{const target=new URL(r.url());return target.pathname===`${mount}/api/mods`
        &&target.searchParams.get('mine')==='true'&&r.ok();}),
    ]);
    await page.getByRole('button',{name:'Sign in',exact:true}).click();
    await page.waitForURL(target=>target.pathname===`${mount}/account`);
    for(const response of await reads)assert.equal(await response.finished(),null);
    await ui.observeDocumentReplacement(page,()=>page.goto(url.href));await ui.ready(page);
    await startNewGame(page,{name:'Felix',element:'fire',discipline:'arcane',continueLocal:true});
    await ready();assert.equal(receipt.welcome.kind,'PrivateCollege');assert.equal(receipt.welcome.cheats,false);
    const potion=player().economy.backpack.find(item=>item.name==='Health Potion');
    assert.ok(potion?.quantity>0,'the admitted wizard owns a real Health Potion');
    receipt.potion={id:potion.id,quantity:potion.quantity,origin:'native backpack at ordinary admission'};

    step='actual authored selector and response';
    await shortcut('Provokatus');await ui.widget(page,'panel:dialogue');
    await ui.click(page,'panel:dialogue-next');
    await ui.click(page,(await ui.byLabel(page,/boast/i,{role:'button',prefix:'panel:dialogue-choice:'})).id);
    await ui.wheel(page,'panel:selector-scroll',0,1000);
    const row=await ui.byLabel(page,boastName,{role:'button',prefix:`panel:npc:${content.packageId}:`});
    const choice=row.id,contentId=choice.split(':').at(-1);
    assert.match(contentId,/^[1-9][0-9]{0,18}$/);
    await shot('authored-boast-choice');
    const commandsBefore=receipt.commands.length;
    await ui.click(page,choice);
    await until(()=>player().economy.npc.boast.selected?.contentId===contentId,'accepted namespaced boast');
    const selected=player().economy.npc.boast.selected;
    assert.equal(selected.modId,content.packageId);
    await ui.waitForUi(page,s=>ui.findWidget(s,'panel:dialogue-key')?.value===`MOD_BOAST_${contentId}`,'authored response after native highlight');
    await shot('authored-boast-response');
    receipt.selection={selected,commands:receipt.commands.slice(commandsBefore)};
    await ui.click(page,'panel:dialogue-next');await ui.noWidget(page,'panel:dialogue');
    const note=await ui.widget(page,'npc:notebox');assert.equal(note.role,'status');assert.equal(note.label,instruction);
    await shot('authored-boast-instruction');
    await ui.click(page,'npc:notebox');await ui.noWidget(page,'npc:notebox');
    await ready();
    receipt.checks.push('actual authored row identity, accepted native highlight, response and dismissible instruction preserve the Source package');

    step='real combat and potion-use failure';
    await ui.click(page,(await ui.byLabel(page,'Enter the Boneyard',{role:'button'})).id);
    await until(()=>frame.snapshot.boneyard,'actual Boneyard');await ready();
    await walkSolomonEntrance(page,{snapshot:()=>frame.snapshot,playerId:()=>playerId,output:out});await ready();
    await until(()=>player().combat.current_health<player().combat.maximum_health
      &&player().combat.life_state==='Alive','real enemy damage before potion use',120000);
    await page.keyboard.press('KeyI');await ui.widget(page,'panel:surface:inventory');
    assert.equal(player().economy.npc.boast.failed,false);
    const beforeQuantity=player().economy.backpack.find(item=>item.id===potion.id)?.quantity;
    await ui.click(page,`panel:item:${potion.id}`);await ui.click(page,`panel:item:${potion.id}`);
    await until(()=>player().economy.npc.boast.failed,'actual potion use breaks the authored boast');
    const afterQuantity=player().economy.backpack.find(item=>item.id===potion.id)?.quantity||0;
    assert.equal(afterQuantity,beforeQuantity-1);
    const broken=await ui.widget(page,'npc:notebox');assert.equal(broken.role,'alert');
    assert.equal(broken.label,`FAILED "${boastName}"`);
    receipt.failureNotice={label:broken.label,beforeQuantity,afterQuantity,boast:player().economy.npc.boast};
    await shot('authored-boast-failed');await ui.click(page,'npc:notebox');await ui.noWidget(page,'npc:notebox');
    await page.keyboard.press('Escape');await ui.noWidget(page,'panel:surface:inventory');
    receipt.checks.push('actual enemy damage and one owned potion produce the namespaced red failure notice and normal dismissal');
    await page.keyboard.press('Escape');await ui.surface(page,'pause');await ui.click(page,'leave');await ui.surface(page,'title');
    await ui.assertGpuOnly(page);await ui.settleErrors(page);assert.deepEqual(observed.errors,[]);
    assert.deepEqual(receipt.errors,[]);receipt.checks.push('authored failure returns through the ordinary saved Leave');receipt.passed=true;
  }catch(error){failure=error;receipt.step=step;receipt.failure=error.stack;receipt.failedBoast=player()?.economy.npc.boast;
    if(page){receipt.failedUi=await ui.readUi(page).catch(error=>({error:error.message}));await shot('failure').catch(()=>{});}}
  finally{if(page)try{await ui.settleErrors(page);receipt.browserErrors=observed.errors;
      if(observed.errors.length){receipt.passed=false;failure||=new Error('Unexpected browser errors');}}
    catch(error){receipt.passed=false;failure||=error;}
    try{receipt.cleanup=await owner.close();}catch(error){failure||=error;receipt.passed=false;receipt.cleanup=error.cleanup;}
    fs.writeFileSync(path.join(out,'receipt.json'),JSON.stringify(receipt,null,2));}
  console.log(JSON.stringify({passed:receipt.passed,checks:receipt.checks,step,cleanup:receipt.cleanup}));
  if(failure)throw failure;
})().catch(error=>{console.error(error);process.exitCode=1});
