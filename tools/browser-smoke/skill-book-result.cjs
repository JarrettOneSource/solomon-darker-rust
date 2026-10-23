// Source-produced saves enter through the real File chooser. All book reads,
// confirmations and reloads use ordinary UI input; snapshots are read-only.
// This covers imported-inventory use, not an ordinary earned loot drop.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const zlib = require('node:zlib');
const {chromium} = require(path.resolve(process.env.PW));
const ui = require('./gpu-ui.cjs');
const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
const {createNativeMessageReader} = require('./native-wire.cjs');
const root = path.resolve(__dirname,'../..');
const fixture = JSON.parse(zlib.inflateSync(fs.readFileSync(path.join(root,
  'crates/sim/tests/fixtures/website/skill-book-browser-saves.json.zlib'))));
const layouts = JSON.parse(fs.readFileSync(path.join(root,
  'crates/sim/tests/fixtures/website/skill-book-feedback-goldens.json'))).cases;
const output = path.resolve(process.env.OUT || '/tmp/skill-book-result');
const url = process.env.URL || 'http://127.0.0.1:4181/';
fs.mkdirSync(output,{recursive:true});
const receipt = {passed:false,qualification:fixture.qualification,worlds:[],errors:[]};

async function until(predicate,label,timeout=30000) {
  const end=Date.now()+timeout;
  while(Date.now()<end){if(await predicate())return;await new Promise(resolve=>setTimeout(resolve,30));}
  throw new Error(`Timed out waiting for ${label}`);
}
async function visibleAfterScroll(page,id) {
  for(let attempt=0;attempt<20;attempt++){
    const target=ui.findWidget(await ui.readUi(page),id);
    if(target?.enabled)return target;
    await ui.wheel(page,'settings-content',0,260);
  }
  throw new Error(`Settings never revealed ${id}`);
}
async function admitted(page,value,accepted) {
  const expected=value.document.continuation.simulation.playerEntities.configs[0];
  const uiState=await ui.waitForUi(page,snapshot=>{
    const selection=snapshot.acceptedSelection,frame=accepted.frame,host=frame?.host;
    return snapshot.ready&&snapshot.surface==='hidden'&&snapshot.inputScope===null
      &&selection?.name===expected.displayName
      &&selection.element.toLowerCase()===expected.element
      &&selection.discipline.toLowerCase()===expected.discipline
      &&frame?.snapshot.run.phase===(value.world==='hub'?'Hub':'Active')
      &&host?.connected_player_ids.includes(accepted.id)
      &&!host.materializing_player_ids.includes(accepted.id)
      &&host.loading_run_id===null&&host.resume_grace===null
      &&(host.pause===null||host.pause.owner_player_id===accepted.id);
  },`actual admitted ${expected.displayName}/${value.world} and ready authority`,{timeout:120000});
  assert.deepEqual(uiState.widgets.filter(widget=>widget.id.startsWith('world-notification:book:'))
    .map(widget=>widget.id),[],'admission never replays a retained accepted rank notification');
  return{selection:uiState.acceptedSelection,frame:uiState.frame,inputScope:uiState.inputScope,
    authority:{playerId:accepted.id,ticks:accepted.frame.snapshot.ticks,
      phase:accepted.frame.snapshot.run.phase,host:accepted.frame.host}};
}
async function importSave(page,value,accepted) {
  await page.goto(url);await ui.ready(page);await ui.surface(page,'title');
  if(ui.findWidget(await ui.readUi(page),'prompt-secondary'))await ui.click(page,'prompt-secondary');
  await ui.click(page,'settings');await ui.click(page,'settings-page:cloud');
  await visibleAfterScroll(page,'settings-page:save-transfer');
  await ui.click(page,'settings-page:save-transfer');
  const chooser=page.waitForEvent('filechooser');await ui.click(page,'save-import');
  await(await chooser).setFiles({name:`source-book-${value.world}.json`,mimeType:'application/json',
    buffer:Buffer.from(JSON.stringify(value.document))});
  await visibleAfterScroll(page,'save-replace');await ui.click(page,'save-replace');
  await ui.noWidget(page,'save-replace');
  await ui.widget(page,'save-export',{enabled:true});
  await ui.observeDocumentReplacement(page,()=>page.reload());await ui.ready(page);await ui.surface(page,'title');
  if(ui.findWidget(await ui.readUi(page),'prompt-secondary'))await ui.click(page,'prompt-secondary');
  await ui.click(page,'play');await ui.click(page,'continue');
  const admission=await admitted(page,value,accepted);
  await ui.noWidget(page,'panel:skill-book-result');
  return admission;
}
async function position(page) {
  return page.evaluate(()=>Array.from(window.wasmBindings.wizard_state()).slice(0,2));
}
async function tick(page) {
  return page.evaluate(()=>window.wasmBindings.wizard_state()[5]);
}

(async()=>{
  const owned=await launchOwnedBrowser(chromium,{headless:true,
    executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=metal','--ignore-gpu-blocklist']});
  const {browser}=owned;
  try {
    for(const value of fixture.cases){
      const context=await browser.newContext({viewport:{width:1600,height:900},
        ignoreHTTPSErrors:process.env.LOCAL_TLS==='1'});
      const page=await context.newPage();
      const observed=ui.watchErrors(page);
      const accepted={frame:null,id:null};
      const requests=[];
      page.on('websocket',socket=>{
        const read=createNativeMessageReader();
        socket.on('framereceived',({payload})=>{
          const message=read(JSON.parse(payload.toString()));
          if(!message)return;
          if(message.type==='welcome'){accepted.id=message.credentials.player_id;accepted.frame=message.frame;}
          else if(message.type==='snapshot')accepted.frame=message.frame;
        });
        socket.on('framesent',({payload})=>{
          const message=JSON.parse(payload.toString());
          if(message.type==='command'&&JSON.stringify(message.command).includes('ReadSkillBook'))requests.push(message.command);
        });
      });
      const result={world:value.world,retainedSequence:value.retainedSequence,reads:[]};
      try {
        await observed.ready;
        result.admission=await importSave(page,value,accepted);
        assert.equal(ui.findWidget(await ui.readUi(page),'panel:skill-book-result'),undefined,
          'admission seeds the retained accepted rank receipt without replaying it');
        for(const [index,bookId] of value.bookIds.slice(0,2).entries()){
          await page.keyboard.press('KeyI');await ui.widget(page,'panel:surface:inventory');
          const book=`panel:item:${bookId}`;await ui.widget(page,book,{enabled:true});
          if(value.world==='boneyard'&&accepted.frame)await until(()=>accepted.frame?.host.pause?.source==='inventory','authoritative Inventory pause');
          await ui.click(page,book,{clickCount:2});
          const modal=await ui.widget(page,'panel:skill-book-result');
          const outcome=JSON.parse(modal.value);
          assert.ok(outcome.sequence>value.retainedSequence);
          const expected=layouts.find(row=>row.skillId===outcome.skillId);
          assert.ok(expected,'accepted result names an authored native rank row');
          await ui.value(page,'panel:skill-book-result-text',expected.layout.lines[1].text);
          await ui.noWidget(page,'panel:surface:inventory');
          const before=await position(page),pausedTick=await tick(page);
          await page.keyboard.down('KeyS');await page.waitForTimeout(220);await page.keyboard.up('KeyS');
          assert.deepEqual(await position(page),before,'the independent result seals movement after Inventory closes');
          if(value.world==='boneyard'){
            if(accepted.frame)assert.equal(accepted.frame.host.pause.source,'inventory','the result retains its prior native pause source');
            assert.equal(await tick(page),pausedTick,'the authoritative world stays paused until confirmation');
          }
          await page.screenshot({path:path.join(output,`${value.world}-rank-${index}-result.png`)});
          if(index===0)await ui.click(page,'panel:skill-book-result-okay');
          else await page.keyboard.press('Escape');
          await ui.noWidget(page,'panel:skill-book-result');
          assert.equal((await ui.readUi(page)).surface,'hidden','dismissing the result does not open Pause');
          if(value.world==='boneyard')await until(async()=>await tick(page)>pausedTick,'authoritative play resumes after confirmation');
          const notificationId=`world-notification:book:-${outcome.sequence}`;
          const entered=await ui.waitForUi(page,snapshot=>{
            const message=ui.findWidget(snapshot,notificationId);
            return message&&JSON.parse(message.value).offset===0;
          },'the actual rank notification completes its native entrance');
          const notification=ui.findWidget(entered,notificationId);
          assert.equal(notification.label,expected.text,'the actual accepted rank also paints its native world +1 message');
          const presentation=JSON.parse(notification.value);
          assert.equal(presentation.baseline,67,'Source places the settled Menu text at50+17');
          assert.equal(presentation.clipTop,50,'Source world-notification ink clips atnativeY50');
          assert.equal(presentation.scale,1);
          await page.screenshot({path:path.join(output,`${value.world}-rank-${index}-notification.png`)});
          result.reads.push({bookId,skillId:outcome.skillId,sequence:outcome.sequence,
            dismiss:index===0?'OKAY':'Escape',notification});
        }
        await ui.observeDocumentReplacement(page,()=>page.reload());await ui.ready(page);await ui.click(page,'play');await ui.click(page,'continue');
        result.restoredAdmission=await admitted(page,value,accepted);
        await ui.noWidget(page,'panel:skill-book-result');
        await page.keyboard.press('KeyI');await ui.widget(page,'panel:surface:inventory');
        for(const id of value.bookIds.slice(0,2))assert.equal(ui.findWidget(await ui.readUi(page),`panel:item:${id}`),undefined,'accepted consumed books stay absent after reload');
        await ui.click(page,`panel:item:${value.bookIds[2]}`,{clickCount:2});
        await ui.widget(page,'picker:choice:0');
        assert.equal(ui.findWidget(await ui.readUi(page),'panel:skill-book-result'),undefined,'choice books open the actual offer instead of a rank result');
        await ui.click(page,'picker:choice:0');await ui.noWidget(page,'picker:choice:0');
        await ui.assertGpuOnly(page);await ui.settleErrors(page);assert.deepEqual(observed.errors,[]);
        result.requests=requests.length;
        result.authority=accepted.frame?'native host':'local Rust session';
        receipt.worlds.push(result);
      }catch(error){
        await page.screenshot({path:path.join(output,`${value.world}-failure.png`)});
        receipt.failure=error.stack;
        receipt.ui=await ui.readUi(page).catch(failure=>({readError:failure.message}));
        throw error;
      }finally{await ui.settleErrors(page);receipt.errors.push(...observed.errors);await context.close();}
    }
    receipt.passed=true;
  }finally{
    try {receipt.cleanup=await owned.close();}
    catch(error){receipt.passed=false;receipt.cleanup=error.cleanup||{error:error.stack};throw error;}
    finally{fs.writeFileSync(path.join(output,'receipt.json'),JSON.stringify(receipt,null,2));}
  }
  console.log(JSON.stringify(receipt));
})().catch(error=>{console.error(error);process.exitCode=1;});
