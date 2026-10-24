// Real Rust economy commands through painted GPU controls and trusted browser input.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const {startNewGame} = require('./menu-journey.cjs');
const ui = require('./gpu-ui.cjs');
const {launchOwnedBrowser}=require('./browser-lifecycle.cjs');
const output = process.env.OUT || '/tmp';
const url = process.env.URL || 'http://127.0.0.1:4185/';
fs.mkdirSync(output,{recursive:true});
const settle = page => page.waitForTimeout(500);
const screenshot = (page,name) => page.screenshot({path:path.join(output,name)});
const close = async page => {await ui.click(page,'panel:close');await ui.noWidget(page,'panel:service');};
const item = (page,label) => ui.byLabel(page,label,{role:'button',prefix:'panel:item:'});
const shortcut = async (page,label) => {const widget=await ui.byLabel(page,label,{role:'button',prefix:'hud:shortcut:'});await ui.click(page,widget.id);};
const gold = (page,expected) => ui.value(page,'panel:gold',expected);
const rows = snapshot => snapshot.widgets.filter(widget=>widget.id.startsWith('panel:dialogue-selector:'));

(async()=>{
  const owned=await launchOwnedBrowser(chromium,{headless:true,
    executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=metal','--ignore-gpu-blocklist']});
  const browser=owned.browser;
  const checks=[];
  const receipt={passed:false,artifact:process.env.ARTIFACT,generatedName:process.env.GENERATED_NAME==='1',checks,screenshots:output};
  let page,observed;
  try {
    page=await browser.newPage({viewport:{width:1600,height:900}});
    observed=ui.watchErrors(page);await observed.ready;
    await page.goto(url);await ui.ready(page);
    await startNewGame(page,{name:process.env.GENERATED_NAME==='1'?null:'Felix'});
    await shortcut(page,'Fomentius');await ui.widget(page,'panel:service');await settle(page);
    await screenshot(page,'service-fomentius-rust.png');
    const healthOffer=(await ui.readUi(page)).widgets.find(widget=>widget.id.startsWith('panel:shop:')&&widget.label==='Buy Health Potion for 150 gold');
    assert.ok(healthOffer,'the actual Health Potion shop offer is visible');
    await ui.click(page,healthOffer.id);await settle(page);
    await item(page,'Health Potion, quantity 1');await gold(page,'500');
    await ui.click(page,healthOffer.id);await settle(page);
    const health=await item(page,'Health Potion, quantity 2');await gold(page,'350');
    checks.push('first purchase click selects; the second commits one real 150-gold purchase');
    await close(page);await shortcut(page,'Luthacus');await ui.widget(page,'panel:service');await settle(page);
    assert.equal((await ui.readUi(page)).widgets.filter(widget=>widget.role==='button'&&/sell/i.test(widget.label)).length,0,'native storage has no invented sale');
    const cell=(await ui.readUi(page)).widgets.find(widget=>widget.id.startsWith('panel:shop-cell:'));
    assert.ok(cell,'an actual addressed storage cell is visible');
    await ui.drag(page,health.id,cell.id);await settle(page);
    const stored=await ui.byLabel(page,/Health Potion/,{role:'button',prefix:'panel:storage:'});
    await screenshot(page,'service-luthacus-rust.png');
    await ui.click(page,stored.id,{clickCount:2});await settle(page);
    assert.equal((await ui.readUi(page)).widgets.filter(widget=>widget.id.startsWith('panel:storage:')).length,0,'double activation returns the real storage item');
    await item(page,'Health Potion, quantity 2');
    checks.push('addressed drag stores the owned stack; double activation returns it without selling');
    await close(page);await shortcut(page,'Hagatha');await ui.widget(page,'panel:service');await settle(page);
    await screenshot(page,'service-hagatha-rust.png');
    await ui.click(page,'panel:hagatha:0');await ui.click(page,'panel:hagatha:0');await settle(page);
    assert.equal(ui.findWidget(await ui.readUi(page),'panel:notice-title'),undefined,'ordinary insufficient-gold shop rejection has no invented modal');
    await gold(page,'350');
    assert.equal((await ui.readUi(page)).widgets.filter(widget=>widget.id.startsWith('panel:hagatha:')).length,27,'all27 canonical Hagatha offers retain the row-first grid');
    checks.push('Hagatha exposes all27 offers and insufficient gold preserves the economy');
    await close(page);await shortcut(page,'Shlorio');await ui.widget(page,'panel:service');await settle(page);
    await screenshot(page,'service-shlorio-rust.png');await ui.click(page,'panel:dowse');
    await ui.value(page,'panel:notice-title','NOT ENOUGH GOLD!');await gold(page,'350');
    await screenshot(page,'service-dowsing-insufficient-rust.png');
    await ui.click(page,'panel:notice-okay');await close(page);
    checks.push('rejected Dowsing shows the native notice and keeps accepted gold');
    await shortcut(page,'Provokatus');await ui.widget(page,'panel:dialogue');await settle(page);
    await screenshot(page,'dialogue-annalist-rust.png');await ui.click(page,'panel:dialogue-next');await settle(page);
    await screenshot(page,'dialogue-annalist-choices-rust.png');
    const boast=await ui.byLabel(page,/boast/i,{role:'button',prefix:'panel:dialogue-choice:'});await ui.click(page,boast.id);await settle(page);
    await screenshot(page,'selector-boasts-rust.png');
    const row=ui.bounds(await ui.widget(page,'panel:dialogue-selector:0'));
    const frame=ui.bounds(await ui.widget(page,'panel:selector-frame:0'));
    assert.ok(Math.abs(frame.x-row.x)<1&&Math.abs(frame.y-row.y)<1,'native selector chrome starts at the actual row origin');
    const title=ui.bounds(await ui.widget(page,'panel:selector-label:0'));
    const detail=ui.bounds(await ui.widget(page,'panel:selector-detail:0'));
    assert.ok(title.y+title.height<=detail.y,'actual painted boast title fits above its description');
    const seen=new Set(rows(await ui.readUi(page)).map(widget=>widget.id));
    for(let index=0;index<12&&!ui.findWidget(await ui.readUi(page),'panel:selector-done')?.focused;index++){
      await page.keyboard.press('Tab');await ui.nextFrame(page);await page.waitForTimeout(30);
      for(const widget of rows(await ui.readUi(page)))seen.add(widget.id);
    }
    assert.equal(seen.size,5,'real keyboard traversal exposes every native boast');
    await ui.wheel(page,'panel:selector-scroll',0,-10000);
    await ui.widget(page,'panel:dialogue-selector:0');
    await ui.click(page,'panel:dialogue-selector:0');
    await ui.waitForUi(page,snapshot=>ui.findWidget(snapshot,'panel:dialogue-key')?.value.startsWith('ANNAL_'),'accepted annalist response');
    await ui.click(page,'panel:dialogue-next');await ui.noWidget(page,'panel:dialogue');
    assert.equal((await ui.readUi(page)).surface,'hidden','closing the conversation returns directly to play');
    await ui.assertGpuOnly(page);await ui.settleErrors(page);assert.deepEqual(observed.errors,[]);
    checks.push('all5 native boasts preserve frame/text geometry and accepted dialogue lifecycle');
    receipt.passed=true;
  }catch(error){
    receipt.failure=error.stack;
    if(page){await ui.settleErrors(page);await screenshot(page,'hub-services-failure.png');}
    fs.writeFileSync(path.join(output,'hub-services-failure.json'),JSON.stringify({error:error.stack,checks,errors:observed?.errors,ui:page?await ui.readUi(page):null},null,2));
    throw error;
  }finally{
    try {if(page)await ui.settleErrors(page)}
    catch(error){receipt.passed=false;receipt.failure??=error.stack;throw error}
    finally {
      try{receipt.cleanup=await owned.close()}
      catch(error){receipt.passed=false;receipt.cleanup=error.cleanup;throw error}
      finally {
        receipt.errors=observed?.errors||[];
        if(receipt.errors.length)receipt.passed=false;
        fs.writeFileSync(path.join(output,'receipt.json'),JSON.stringify(receipt,null,2));
      }
    }
  }
  assert.deepEqual(receipt.errors,[]);
  console.log(JSON.stringify(receipt));
})().catch(error=>{console.error(error);process.exitCode=1;});
