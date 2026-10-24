// Ordinary saved-layout publication and loading through the real Website service.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {chromium}=require(process.env.PW);
const {launchOwnedBrowser}=require('./browser-lifecycle.cjs');
const {revealSettingsControl}=require('./settings-journey.cjs');
const ui=require('./gpu-ui.cjs');
const out=path.resolve(process.env.OUT),url=process.env.URL,website=new URL(url);
const mount=website.pathname.replace(/\/$/,'');
const account=JSON.parse(fs.readFileSync(path.join(process.env.PRIVATE_DIR,'private-accounts.json')))[0];
const receipt={passed:false,checks:[],requests:[]};
const observations=[];
const pendingCatalogs=new WeakMap();
let page,step='ordinary sign-in',releaseLoad;
const stored=page=>page.evaluate(()=>Object.fromEntries(Object.entries(localStorage)
  .filter(([key])=>key.includes('mobile-ui-layout')).map(([key,value])=>[key,JSON.parse(value)])));
async function title(page){await ui.ready(page);if(ui.findWidget(await ui.readUi(page),'prompt-secondary'))
  await ui.click(page,'prompt-secondary');}
async function layouts(page){await ui.click(page,'explore');await ui.surface(page,'dark-cloud');
  await ui.waitForUi(page,s=>ui.findWidget(s,'cloud-list-status')&&!ui.findWidget(s,'cloud-list-status').value?.includes('CONSULTING')
    &&pendingCatalogs.get(page).size===0,'initial real catalog requests finish');
  await ui.click(page,'cloud-tab:layouts');await ui.widget(page,'dark-cloud-layout-code');}
async function observe(page){const observed=ui.watchErrors(page);observations.push({page,observed});await observed.ready;
  const pending=new Set();pendingCatalogs.set(page,pending);
  page.on('request',request=>{if(new URL(request.url()).pathname.includes('/api/mods'))pending.add(request)});
  page.on('requestfinished',request=>pending.delete(request));page.on('requestfailed',request=>pending.delete(request));
  page.on('response',response=>{const pathname=new URL(response.url()).pathname;
    if(pathname.includes('/api/game/layouts'))receipt.requests.push({path:pathname,status:response.status(),method:response.request().method()});});}
async function shot(name){await page.screenshot({path:path.join(out,`${name}.png`)});}

(async()=>{
  fs.mkdirSync(out,{recursive:true});
  const owner=await launchOwnedBrowser(chromium,{headless:true,executablePath:process.env.CHROME,
    args:['--use-angle=metal','--ignore-gpu-blocklist']});
  let failure;
  try{
    const context=await owner.browser.newContext({viewport:{width:1600,height:900},ignoreHTTPSErrors:true});
    await context.grantPermissions(['clipboard-read','clipboard-write'],{origin:website.origin});
    page=await context.newPage();await observe(page);
    await page.goto(new URL(`${mount}/login`,website).href);
    await page.locator('input[autocomplete="username"]').fill(account.username);
    await page.locator('input[autocomplete="current-password"]').fill(account.password);
    await page.getByRole('button',{name:'Sign in',exact:true}).click();
    await page.waitForURL(target=>target.pathname===`${mount}/account`);
    await ui.observeDocumentReplacement(page,()=>page.goto(url));await title(page);
    step='real editor customization and Save';
    await ui.click(page,'settings');await revealSettingsControl(page,'settings-page:mobile-ui');
    await ui.click(page,'settings-page:mobile-ui');await ui.widget(page,'mobile/editor');
    const old=(await ui.widget(page,'mobile/control:pause')).value;
    const from=ui.center(ui.bounds(await ui.widget(page,'mobile/control:pause')));
    await page.mouse.move(from.x,from.y);await page.mouse.down();
    await page.mouse.move(from.x+90,from.y+60,{steps:8});await page.mouse.up();
    await ui.waitForUi(page,s=>ui.findWidget(s,'mobile/control:pause')?.value!==old,'actual layout draft changes');
    await ui.click(page,'settings-back');await ui.noWidget(page,'mobile/editor');
    receipt.saved=await stored(page);assert.equal(Object.keys(receipt.saved).length,1);
    await ui.click(page,'settings-back');await ui.surface(page,'title');await layouts(page);
    step='real layout publication and trusted clipboard';
    const publishing=page.waitForResponse(response=>new URL(response.url()).pathname.endsWith('/api/game/layouts')
      &&response.request().method()==='POST'&&response.ok());
    await ui.click(page,'cloud-layout-publish');receipt.published=await(await publishing).json();
    const code=receipt.published.code;assert.match(code,/^[A-HJ-NP-Z2-9]{4}-[A-HJ-NP-Z2-9]{4}$/);
    assert.equal(receipt.published.author.username,account.username);
    assert.deepEqual(receipt.published.layout,Object.values(receipt.saved)[0]);
    await ui.value(page,'dark-cloud-layout-code',code);await ui.click(page,'cloud-copy-layout');
    await ui.waitForUi(page,s=>ui.findWidget(s,'cloud-copy-layout')?.label==='COPIED','trusted code copy completion');
    assert.equal(await page.evaluate(()=>navigator.clipboard.readText()),code);
    await shot('layout-published-desktop');
    receipt.checks.push('real editor Save, authenticated layout publication and trusted clipboard preserve the complete document');
    await page.setViewportSize({width:390,height:844});await ui.nextFrame(page);
    await page.mouse.move(195,500);await page.mouse.wheel(0,600);await ui.nextFrame(page);
    await shot('layout-published-phone');
    const phoneReceipt=await ui.readUi(page);
    let bottom=0;
    for(const id of ['cloud-layout-status','cloud-layout-receipt-code','cloud-layout-author','cloud-copy-layout']){
      const rect=ui.bounds(await ui.widget(page,id));assert.ok(rect.y>=bottom,`${id} follows the prior phone receipt row`);
      assert.ok(rect.x>=0&&rect.x+rect.width<=390,`${id} stays inside the phone`);bottom=rect.y+rect.height;
    }
    receipt.phoneReceipt=phoneReceipt.widgets.filter(row=>row.id.startsWith('cloud-layout-')||row.id==='cloud-copy-layout');
    receipt.checks.push('phone publication status, code, author and Copy use separate visible rows');

    step='guest real load, busy action admission and editable source field';
    const guest=await owner.browser.newContext({viewport:{width:1600,height:900},ignoreHTTPSErrors:true});
    page=await guest.newPage();await observe(page);await page.goto(url);await title(page);await layouts(page);
    assert.equal((await ui.widget(page,'cloud-layout-publish')).enabled,false);
    assert.deepEqual(await stored(page),{});
    await ui.fill(page,'dark-cloud-layout-code',`0I-${code.toLowerCase()}`);await ui.value(page,'dark-cloud-layout-code',code);
    await page.keyboard.press('Meta+A');
    const selection=ui.bounds(await ui.widget(page,'dark-cloud-layout-code/selection:0'));
    const field=ui.bounds(await ui.widget(page,'dark-cloud-layout-code'));
    assert.ok(Math.abs(ui.center(selection).x-ui.center(field).x)<1,'native selection shares the centered monospace text geometry');
    let captured;
    const gate=new Promise(resolve=>{releaseLoad=resolve});
    await guest.route('**/api/game/layouts/*',async route=>{captured=route.request();await gate;await route.continue();});
    await ui.click(page,'cloud-layout-load');
    await ui.waitForUi(page,s=>captured&&ui.findWidget(s,'cloud-layout-load')?.label==='LOADING...','actual pending layout request');
    assert.equal((await ui.widget(page,'cloud-layout-load')).enabled,false);
    assert.equal((await ui.widget(page,'cloud-layout-publish')).enabled,false);
    assert.equal((await ui.widget(page,'dark-cloud-layout-code')).enabled,true,'Source keeps the code field editable while its request is pending');
    await ui.fill(page,'dark-cloud-layout-code','ABCD-EFGH');
    assert.equal(new URL(captured.url()).pathname,`${mount}/api/game/layouts/${code}`);
    assert.deepEqual(await stored(page),{},'no layout is installed before the real response');
    await shot('layout-request-pending');releaseLoad();
    await ui.widget(page,'cloud-copy-layout');await ui.value(page,'dark-cloud-layout-code',code);
    assert.deepEqual(await stored(page),receipt.saved);
    receipt.checks.push('guest code normalization and held real request preserve editable draft, disable actions and install the returned complete document');
    await shot('layout-loaded-guest');
    await ui.click(page,'cloud-menu');await ui.surface(page,'pause');await ui.click(page,'pause-settings');
    await revealSettingsControl(page,'settings-page:mobile-ui');await ui.click(page,'settings-page:mobile-ui');
    await ui.waitForUi(page,s=>ui.findWidget(s,'mobile/editor')?.selected===true,'shared layout reaches the actual editor');
    receipt.loadedPause=(await ui.widget(page,'mobile/control:pause')).value;
    assert.deepEqual(await stored(page),receipt.saved);await shot('layout-loaded-editor');
    await ui.assertGpuOnly(page);receipt.checks.push('loaded shared layout opens as customized through the ordinary Settings editor');
    receipt.passed=true;
  }catch(error){failure=error;receipt.failure=error.stack;receipt.step=step;
    if(page){receipt.failedUi=await ui.readUi(page).catch(error=>({error:error.message}));await shot('failure').catch(()=>{});}}
  finally{releaseLoad?.();receipt.errors=[];
    for(const row of observations){try{await ui.settleErrors(row.page);receipt.errors.push(...row.observed.errors);}
      catch(error){failure||=error;receipt.errors.push(error.message);}}
    if(receipt.errors.length){receipt.passed=false;failure||=new Error('Unexpected browser errors');}
    try{receipt.cleanup=await owner.close();}catch(error){failure||=error;receipt.passed=false;receipt.cleanup=error.cleanup;}
    fs.writeFileSync(path.join(out,'receipt.json'),JSON.stringify(receipt,null,2));}
  console.log(JSON.stringify({passed:receipt.passed,checks:receipt.checks,step,cleanup:receipt.cleanup}));
  if(failure)throw failure;
})().catch(error=>{console.error(error);process.exitCode=1});
