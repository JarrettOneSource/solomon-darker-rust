// Buy an ordinary Sack, edit through the real GPU field, and inspect its actual
// shaped-font draw and durable accepted inventory. No state or commands are injected.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {chromium}=require(path.resolve(process.env.PW));
const ui=require('./gpu-ui.cjs');
const {startNewGame}=require('./menu-journey.cjs');
const {waitForSave,localParticipant}=require('./saved-game.cjs');
const output=process.env.OUT||'/tmp/sack-rename';
const url=process.env.URL||'http://127.0.0.1:4181/';
fs.mkdirSync(output,{recursive:true});
const receipt={passed:false,generatedName:process.env.GENERATED_NAME==='1',checks:[],errors:[],pageErrors:[]};
const names=['AV fi 🧙','تراث fi 🧙','A🧑‍💻é🧙🏽B'];
async function pagePath(page,expected){
  await ui.waitForUi(page,snapshot=>{
    const value=ui.findWidget(snapshot,'panel:sack-page')?.value;
    return value&&JSON.stringify(JSON.parse(value).path)===JSON.stringify(expected);
  },'settled native Sack page');
}
async function expectedFont(page,text){
  return page.evaluate(text=>{
    const canvas=document.createElement('canvas'),font=canvas.getContext('2d');
    font.font='normal normal normal 24px "Arial",sans-serif';
    const probe=font.measureText('|ÉqÅM'),line=font.measureText(text);
    return {width:Math.max(line.width,line.actualBoundingBoxLeft+line.actualBoundingBoxRight),
      height:Math.max(24,probe.actualBoundingBoxAscent+probe.actualBoundingBoxDescent),
      baselineOffset:probe.actualBoundingBoxAscent};
  },text);
}
(async()=>{
  const browser=await chromium.launch({headless:true,
    executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=metal','--ignore-gpu-blocklist']});
  const page=await browser.newPage({viewport:{width:1600,height:900}});
  const observed=ui.watchErrors(page);
  page.on('pageerror',error=>receipt.pageErrors.push(error.stack||error.message));
  try{
    await page.goto(url);await ui.ready(page);
    await startNewGame(page,{name:receipt.generatedName?null:'Felix'});
    const fomentius=await ui.byLabel(page,'Fomentius',{role:'button',prefix:'hud:shortcut:'});
    await ui.click(page,fomentius.id);await ui.widget(page,'panel:service');
    const offer=await ui.byLabel(page,/^Buy Sack for \d+ gold$/,{role:'button',prefix:'panel:shop:'});
    const price=Number(offer.label.match(/(\d+) gold$/)[1]);
    await ui.click(page,offer.id);await ui.click(page,offer.id);
    const sack=await ui.byLabel(page,/^Sack, quantity 1$/,{role:'button',prefix:'panel:item:'});
    const id=Number(sack.id.split(':').at(-1));
    await ui.value(page,'panel:gold',String(500-price));
    receipt.checks.push('the ordinary shop commits one paid Sack purchase');
    await ui.click(page,'panel:close');await ui.noWidget(page,'panel:service');
    await page.keyboard.press('KeyI');await ui.widget(page,'panel:surface:inventory');
    await ui.click(page,sack.id);await ui.click(page,'panel:rename-sack');
    await ui.widget(page,'panel:sack-rename');
    await ui.fill(page,'panel:sack-rename:name','a'.repeat(31)+'🧙');
    await ui.value(page,'panel:sack-rename:name','a'.repeat(31));
    await page.keyboard.press('Escape');await ui.noWidget(page,'panel:sack-rename');
    assert.equal((await ui.widget(page,sack.id)).label,'Sack, quantity 1','Cancel preserves the accepted owned name');
    receipt.checks.push('the real field preserves UTF16 maxlength without splitting a supplementary character; Cancel is local');
    for(const [index,name] of names.entries()){
      await ui.click(page,'panel:rename-sack');
      await ui.fill(page,'panel:sack-rename:name',`  ${name}  `);
      await ui.click(page,'panel:sack-rename:save');await ui.noWidget(page,'panel:sack-rename');
      await ui.waitForUi(page,snapshot=>ui.findWidget(snapshot,sack.id)?.label===`${name}, quantity 1`,'accepted normalized Sack name');
      await ui.click(page,sack.id,{clickCount:2});await pagePath(page,[id]);
      const painted=await ui.widget(page,'panel:sack-name'),metrics=JSON.parse(painted.value);
      const expected=await expectedFont(page,name);
      for(const field of ['width','height','baselineOffset'])assert.ok(Math.abs(metrics[field]-expected[field])<1e-6,`${name}: Source measured ${field}`);
      assert.deepEqual(metrics.lines,[name]);
      assert.ok(ui.bounds(painted).width>0&&ui.bounds(painted).height>0,'the measured shaped name is actually drawn');
      await page.screenshot({path:path.join(output,`sack-name-${index}.png`)});
      await page.keyboard.press('Escape');await pagePath(page,[]);
      await ui.click(page,sack.id);
      receipt.checks.push({name,metrics,actualQuad:painted.paintRect});
    }
    const last=names.at(-1);
    const saved=await waitForSave(page,record=>localParticipant(record).player.economy.backpack.some(item=>item.id===id&&item.name===last));
    fs.writeFileSync(path.join(output,'accepted-save.json'),JSON.stringify(saved,null,2));
    await page.keyboard.press('Escape');await ui.noWidget(page,'panel:surface:inventory');
    await page.context().storageState({path:path.join(output,'browser-storage.json'),indexedDB:true});
    receipt.reproduction={url,save:'accepted-save.json',storage:'browser-storage.json'};
    await page.reload();await ui.ready(page);await ui.click(page,'play');await ui.click(page,'continue');
    await ui.widget(page,'hud:inventory',{enabled:true});
    await page.keyboard.press('KeyI');await ui.widget(page,'panel:surface:inventory');
    assert.equal((await ui.widget(page,sack.id)).label,`${last}, quantity 1`,'full accepted Unicode name survives ordinary reload/Continue');
    await ui.click(page,sack.id,{clickCount:2});await pagePath(page,[id]);
    await ui.widget(page,'panel:sack-name');
    await page.screenshot({path:path.join(output,'sack-name-restored.png')});
    await ui.assertGpuOnly(page);assert.deepEqual(observed.errors,[]);
    receipt.passed=true;
  }catch(error){
    receipt.failure=error.stack;receipt.ui=await ui.readUi(page).catch(failure=>({readError:failure.message}));
    await page.screenshot({path:path.join(output,'failure.png')});throw error;
  }finally{
    receipt.errors=observed.errors;fs.writeFileSync(path.join(output,'receipt.json'),JSON.stringify(receipt,null,2));await browser.close();
  }
  console.log(JSON.stringify(receipt));
})().catch(error=>{console.error(error);process.exitCode=1;});
