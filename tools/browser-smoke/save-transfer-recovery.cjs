// Actual selected files, with a bounded read hold/failure at the browser capability.
// The saved wizard and archive come from the retained ordinary export journey.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {chromium}=require(process.env.PW);
const {launchOwnedBrowser}=require('./browser-lifecycle.cjs');
const ui=require('./gpu-ui.cjs');
const output=path.resolve(process.env.OUT);
const receipt={passed:false,artifact:process.env.ARTIFACT,checks:[],errors:[]};
let page,observed;
async function reveal(id){
  if(ui.findWidget(await ui.readUi(page),id))return;
  await ui.wheel(page,'settings-content',0,-4000);
  for(let i=0;i<16;i++){
    if(ui.findWidget(await ui.readUi(page),id))return;
    await ui.wheel(page,'settings-content',0,200);await ui.nextFrame(page);
  }
  throw new Error(`Save Transfer did not reveal ${id}`);
}
async function choose(files){
  await reveal('save-import');const chooser=page.waitForEvent('filechooser');
  await ui.click(page,'save-import');await(await chooser).setFiles(files);
}
const heldFile={name:'owned-delayed.json',mimeType:'application/json',buffer:Buffer.from('{}')};
(async()=>{
  fs.mkdirSync(output,{recursive:true});
  const owner=await launchOwnedBrowser(chromium,{headless:true,executablePath:process.env.CHROME,
    args:['--use-angle=metal','--ignore-gpu-blocklist']});
  let failure;
  try{
    page=await owner.browser.newPage({viewport:{width:1280,height:900},ignoreHTTPSErrors:true});
    observed=ui.watchErrors(page);await observed.ready;
    await page.addInitScript(()=>{
      const original=File.prototype.arrayBuffer;
      window.saveReadWitness={pending:[],released:0,failed:0};
      File.prototype.arrayBuffer=async function(){
        if(this.name==='owned-unreadable.json'){
          window.saveReadWitness.failed++;
          throw new DOMException('Declared read failure for this selected test file','NotReadableError');
        }
        const bytes=await original.call(this);
        if(this.name==='owned-delayed.json'){
          await new Promise(resolve=>window.saveReadWitness.pending.push(resolve));
          window.saveReadWitness.released++;
        }
        return bytes;
      };
    });
    await page.goto(process.env.URL);await ui.ready(page);
    if(ui.findWidget(await ui.readUi(page),'prompt-secondary'))await ui.click(page,'prompt-secondary');
    await ui.click(page,'settings');await ui.click(page,'settings-page:cloud');
    await ui.click(page,'settings-page:save-transfer');
    await choose(heldFile);await page.waitForFunction(()=>window.saveReadWitness.pending.length===1);
    await ui.waitForUi(page,s=>ui.findWidget(s,'save-import')?.enabled===false,'busy import button');
    assert.equal((await ui.widget(page,'save-export')).enabled,false);
    await page.screenshot({path:path.join(output,'actual-file-read-busy.png')});
    await page.evaluate(()=>window.saveReadWitness.pending.shift()());
    await reveal('save-error');await ui.widget(page,'save-error');
    await reveal('save-import');assert.equal((await ui.widget(page,'save-import')).enabled,true);
    receipt.checks.push('holding the real selected file read disables both transfer actions and completion restores import');

    await choose({...heldFile,name:'owned-unreadable.json'});await reveal('save-error');
    await ui.waitForUi(page,s=>ui.findWidget(s,'save-error')?.label==='The save file owned-unreadable.json could not be read.',
      'visible file capability failure');
    await page.screenshot({path:path.join(output,'actual-file-read-error.png')});
    await choose(process.env.ARCHIVE);await reveal('save-preview');
    assert.match((await ui.widget(page,'save-preview')).value,/Game save: inventory, equipment, and the saved run will be restored/);
    await ui.noWidget(page,'save-error');await reveal('save-cancel');await ui.click(page,'save-cancel');
    receipt.checks.push('the declared file read failure is visible and a genuine exported ZIP retries successfully');

    await choose(heldFile);await page.waitForFunction(()=>window.saveReadWitness.pending.length===1);
    await ui.click(page,'settings-back');await ui.click(page,'settings-page:cloud');
    await ui.click(page,'settings-page:save-transfer');
    await ui.widget(page,'save-import',{enabled:true});
    await page.evaluate(()=>window.saveReadWitness.pending.shift()());
    await page.waitForFunction(()=>window.saveReadWitness.released===2);await ui.nextFrame(page);await ui.nextFrame(page);
    await ui.noWidget(page,'save-error');await ui.noWidget(page,'save-preview');
    await ui.widget(page,'save-import',{enabled:true});
    receipt.checks.push('leaving and reopening Save Transfer retires the held inspection and ignores its late result');

    const files=process.env.ARCHIVE_FILES;
    for(const [selection,source] of [
      [[path.join(files,'darkdata.cfg'),path.join(files,'gamestate.sav')],'Stock progression only:'],
      [path.join(files,'browser-game-save.json'),'Game save:'],
    ]){
      await choose(selection);await reveal('save-preview');
      assert.ok((await ui.widget(page,'save-preview')).value.includes(source));
      await reveal('save-cancel');await ui.click(page,'save-cancel');await ui.noWidget(page,'save-preview');
    }
    receipt.checks.push('the actual exported CFG/SAV pair and browser JSON inspect through the multiple-file chooser without replacing a slot');
    await ui.assertGpuOnly(page);await ui.settleErrors(page);assert.deepEqual(observed.errors,[]);
    receipt.reads=await page.evaluate(()=>({released:window.saveReadWitness.released,failed:window.saveReadWitness.failed,
      pending:window.saveReadWitness.pending.length}));
    receipt.passed=true;
  }catch(error){failure=error;receipt.failure=error.stack;
    if(page){receipt.failedUi=await ui.readUi(page).catch(()=>null);await page.screenshot({path:path.join(output,'failure.png')}).catch(()=>{});}}
  finally{
    if(page)try{await ui.settleErrors(page);assert.deepEqual(observed.errors,[]);}catch(error){failure||=error;receipt.passed=false;}
    receipt.errors=observed?.errors||[];
    try{receipt.cleanup=await owner.close();}catch(error){failure||=error;receipt.passed=false;receipt.cleanup=error.cleanup;}
    fs.writeFileSync(path.join(output,'receipt.json'),JSON.stringify(receipt,null,2));
  }
  console.log(JSON.stringify({passed:receipt.passed,checks:receipt.checks,cleanup:receipt.cleanup}));
  if(failure)throw failure;
})().catch(error=>{console.error(error);process.exitCode=1;});
