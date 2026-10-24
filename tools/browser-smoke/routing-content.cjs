// Real local packages and saved games exercise the Source routing dialogs.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const {execFileSync} = require('node:child_process');
const {chromium} = require(process.env.PW);
const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
const {createNativeMessageReader} = require('./native-wire.cjs');
const {reachCollegeLoadout} = require('./menu-journey.cjs');
const ui = require('./gpu-ui.cjs');
const output = path.resolve(process.env.OUT), url = new URL(process.env.URL);
assert.equal(url.hostname, '127.0.0.1', 'only the owned local fixture is allowed');
assert.equal(url.port, '9449');
const api = new URL('http://127.0.0.1:5229/');
const mount = url.pathname.replace(/\/$/, '');
const receipt = {passed:false, checks:[], requests:[], admissions:[], fixtures:[], deletion:[]};
const suffix = `${Date.now().toString(36)}${crypto.randomBytes(3).toString('hex')}`;
const account = {username:`Routing${suffix}`, email:`routing-${suffix}@example.invalid`,
  password:`Qa!${crypto.randomBytes(18).toString('base64url')}`};
const fault = {request:null, response:null, error:null, message:'Owned routing content failure'};
let token, page, observed, frame, playerId, activeMods, releaseDownload;
let step = 'prepare owned account and packages';
const pending = new Set();
const player = () => frame?.snapshot.participants[playerId]?.player;
const digest = value => crypto.createHash('sha256').update(value).digest('hex');

async function request(endpoint, options = {}) {
  const response = await fetch(new URL(endpoint, api), {...options,
    headers:{...(token ? {Authorization:`Bearer ${token}`} : {}), ...options.headers}});
  const text = await response.text();
  assert.ok(response.ok, `${options.method || 'GET'} ${endpoint}: ${response.status} ${text}`);
  return text ? JSON.parse(text) : null;
}
function packageBytes(identity, name, version, color) {
  const file = path.join(output, `${identity}-${version}.zip`);
  execFileSync('python3', ['-c', [
    'import json,sys,zipfile,zlib,struct',
    'file,identity,name,version,color=sys.argv[1:]',
    'manifest={"id":identity,"name":name,"version":version,"runtime":{"apiVersion":"1.0.0","entryScript":"scripts/main.lua"},"requiredMods":[]}',
    'def chunk(kind,data): return struct.pack(">I",len(data))+kind+data+struct.pack(">I",zlib.crc32(kind+data))',
    'pixel=bytes([int(color),70,120,255])',
    'png=b"\\x89PNG\\r\\n\\x1a\\n"+chunk(b"IHDR",struct.pack(">IIBBBBB",2,2,8,6,0,0,0))+chunk(b"IDAT",zlib.compress((b"\\x00"+pixel*2)*2))+chunk(b"IEND",b"")',
    'with zipfile.ZipFile(file,"w",zipfile.ZIP_DEFLATED) as archive:',
    ' archive.writestr("manifest.json",json.dumps(manifest))',
    ' archive.writestr("scripts/main.lua",\'local marker=sd.art.sprite("art/marker.png")\\nreturn sd.mod({api="1.0.0",assets={marker=marker},content={},rules={}})\\n\')',
    ' archive.writestr("art/marker.png",png)',
  ].join('\n'), file, identity, name, version, String(color)]);
  return fs.readFileSync(file);
}
async function publish(letter, version = '1.0.0', previous) {
  const identity = `qa.routing-${suffix}-${letter}`, name = `Routing ${suffix} ${letter}`;
  const bytes = packageBytes(identity, name, version, letter.charCodeAt(0) + (version==='2.0.0' ? 20 : 0));
  const form = new FormData();
  for (const [key,value] of Object.entries({name, version, visibility:'public', tags:'lua',
    summary:'Owned local routing acceptance package.', description:'Minimal authored content for real routing and saved-mod acceptance.'})) form.set(key,value);
  form.set('file',new Blob([bytes],{type:'application/zip'}),`${identity}-${version}.zip`);
  const detail = await request(previous ? `api/mods/${previous.slug}/versions` : 'api/mods', {method:'POST',body:form});
  const content = previous ? {...previous, version} : {id:identity,name,slug:detail.slug,version};
  receipt.fixtures.push({...content,packageSha256:digest(bytes)});
  return content;
}
async function until(predicate, label, timeout = 45000) {
  const end = Date.now()+timeout;
  while (Date.now()<end) {if(predicate()) return; await page.waitForTimeout(40);}
  throw new Error(`Timed out waiting for ${label}`);
}
async function shot(name) {await page.screenshot({path:path.join(output,`${name}.png`)});}
async function title() {
  await ui.ready(page);
  if(ui.findWidget(await ui.readUi(page),'prompt-secondary')) await ui.click(page,'prompt-secondary');
}
async function cloud() {
  await ui.click(page,'explore'); await ui.surface(page,'dark-cloud');
  await ui.waitForUi(page,s=>ui.findWidget(s,'cloud-list-status')
    &&!ui.findWidget(s,'cloud-list-status').value?.includes('CONSULTING')&&pending.size===0,
  'actual catalog requests finish');
}
async function leaveCloud() {
  await until(()=>pending.size===0,'actual catalog completion');
  await ui.click(page,'cloud-menu'); await ui.surface(page,'pause');
  await ui.click(page,'leave'); await ui.surface(page,'title');
}
async function newGameRoute() {
  await ui.click(page,'play'); await ui.click(page,'new-game');
  const next=await ui.waitForUi(page,s=>ui.findWidget(s,'prompt-primary')||s.surface==='routing','New Game consent');
  if(ui.findWidget(next,'prompt-primary')) await ui.click(page,'prompt-primary');
  await ui.surface(page,'routing');
}
async function finishCreate(name) {
  await ui.waitForUi(page,s=>s.surface==='create'||ui.findWidget(s,'college-intro')
    ||ui.findWidget(s,'panel:dialogue'),'Create or initial College');
  if((await ui.readUi(page)).surface!=='create') await reachCollegeLoadout(page);
  await ui.fill(page,'create-name',name); await ui.click(page,'element:air');
  await ui.click(page,'discipline:arcane');
  await ui.waitForUi(page,s=>s.surface==='hidden'&&!s.inputScope&&s.acceptedSelection?.name===name,'admitted wizard');
  await until(()=>player()?.selection.name===name&&!frame.host.loading_run_id&&!frame.host.resume_grace,'native wizard ready');
}
async function savedLeave() {
  await page.keyboard.press('Escape'); await ui.surface(page,'pause');
  await ui.click(page,'leave'); await ui.surface(page,'title');
  return (await request('api/game/saves/0')).save;
}
function admittedMods() {return receipt.admissions.at(-1).mods.map(mod=>`${mod.id}@${mod.version}`).sort();}

(async()=>{
  fs.mkdirSync(output,{recursive:true});
  fs.writeFileSync(path.join(output,'private-account.json'),JSON.stringify(account),{mode:0o600,flag:'wx'});
  const registered=await request('api/auth/register',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(account)});
  token=registered.token;
  const a=await publish('a'),b=await publish('b'),c=await publish('c');
  for(const content of [a,b]) await request(`api/mods/${content.slug}/subscription`,{method:'PUT'});
  const enabled=await request('api/mods/active');
  const asset=enabled.mods.find(mod=>mod.id===a.id).assets.find(asset=>asset.path==='art/marker.png');
  assert.ok(asset?.sha256,'the backend provides an actual downloadable asset descriptor');
  const owner=await launchOwnedBrowser(chromium,{headless:true,executablePath:process.env.CHROME,
    args:['--use-angle=metal','--ignore-gpu-blocklist']});
  let failure;
  try {
    page=await owner.browser.newPage({ignoreHTTPSErrors:true,viewport:{width:1600,height:900}});
    observed=ui.watchErrors(page);await observed.ready;
    page.on('request',r=>{if(new URL(r.url()).pathname.includes('/api/mods'))pending.add(r);});
    page.on('requestfinished',r=>pending.delete(r));page.on('requestfailed',r=>pending.delete(r));
    page.on('response',async response=>{
      const pathname=new URL(response.url()).pathname;
      if(pathname.includes('/api/'))receipt.requests.push({path:pathname,method:response.request().method(),status:response.status()});
      if(pathname.endsWith('/api/mods/active')&&response.ok())activeMods=(await response.json()).mods;
      if(response.request()===fault.request&&response.status()===503) {
        fault.response={url:response.url(),status:503};const error=observed.errors.at(-1);
        if(error?.type==='response'&&error.url===response.url()&&error.status===503)fault.error=error;
      }
    });
    page.on('websocket',socket=>{const read=createNativeMessageReader();socket.on('framereceived',({payload})=>{
      const message=read(JSON.parse(payload.toString()));
      if(message?.type==='welcome') {frame=message.frame;playerId=message.credentials.player_id;
        receipt.admissions.push({kind:message.hosted?.session_kind,cheats:message.hosted?.cheats_enabled,
          mods:message.hosted.content.mods.map(({id,version,contentSha256})=>({id,version,hash:contentSha256}))});}
      else if(message?.type==='snapshot')frame=message.frame;
    });});
    const held=new Promise(resolve=>{releaseDownload=resolve;});
    await page.route(`**/api/game/content/${asset.sha256}`,async route=>{
      if(fault.request)return route.continue();
      fault.request=route.request();await held;
      await route.fulfill({status:503,contentType:'application/json',body:JSON.stringify({error:fault.message})});
    });
    step='ordinary sign-in and held routing download';
    await page.goto(new URL(`${mount}/login`,url).href);
    await page.locator('input[autocomplete="username"]').fill(account.username);
    await page.locator('input[autocomplete="current-password"]').fill(account.password);
    const reads=Promise.all([
      page.waitForResponse(r=>new URL(r.url()).pathname===`${mount}/api/game/saves/0`&&r.ok()),
      page.waitForResponse(r=>{const target=new URL(r.url());return target.pathname===`${mount}/api/mods`&&target.searchParams.get('mine')==='true'&&r.ok();}),
    ]);
    await page.getByRole('button',{name:'Sign in',exact:true}).click();
    await page.waitForURL(target=>target.pathname===`${mount}/account`);
    for(const response of await reads)assert.equal(await response.finished(),null);
    await ui.observeDocumentReplacement(page,()=>page.goto(url.href));await title();
    await newGameRoute();assert.equal((await ui.widget(page,'routing/dialog')).label,'LOCAL PLAY IS ACTIVE');
    await ui.click(page,'routing-continue');await until(()=>fault.request,'actual held content request');
    await ui.waitForUi(page,s=>ui.findWidget(s,'routing-continue')?.enabled===false,'busy consent');
    for(const id of ['routing-back','routing-continue','routing-vanilla'])assert.equal((await ui.widget(page,id)).enabled,false);
    await page.keyboard.press('Escape');await ui.surface(page,'routing');assert.equal(receipt.admissions.length,0);
    receipt.downloadPending=(await ui.widget(page,'routing/dialog')).value;
    assert.match(receipt.downloadPending,/download/i);await shot('routing-download-pending');
    releaseDownload();
    const errorText=`${a.name} was disabled because its content could not be loaded or verified.`;
    await ui.waitForUi(page,s=>ui.findWidget(s,'routing/dialog')?.value.includes(errorText),'actual failed-download alert');
    await until(()=>activeMods?.length===1&&activeMods[0].id===b.id,'failed package actually disabled');
    assert.equal((await ui.widget(page,'routing-back')).enabled,true);await shot('routing-download-error');
    receipt.checks.push('held real download seals all consent actions; exact503 exposes the Source error and disables only its owning package');
    await ui.click(page,'routing-back');await ui.surface(page,'title');
    await cloud();await ui.click(page,'cloud-tab:subscribed');
    await ui.click(page,`cloud-subscription:enable:${a.slug}`);
    await ui.widget(page,`cloud-subscription:disable:${a.slug}`,{enabled:true});
    await until(()=>activeMods?.length===2&&pending.size===0,'ordinary re-enable and content verification');
    await leaveCloud();await newGameRoute();await ui.click(page,'routing-continue');await finishCreate('Lucian');
    assert.deepEqual(admittedMods(),[`${a.id}@1.0.0`,`${b.id}@1.0.0`].sort());
    const baseline=await savedLeave();assert.ok(baseline.document.length>1000);
    receipt.baseline={revision:baseline.revision,sha256:digest(baseline.document)};
    receipt.checks.push('ordinary Cloud re-enable and retry admit both real packages and create a durable Lucian save');

    step='real added, missing and changed save mods';
    await publish('a','2.0.0',a);
    await cloud();await ui.click(page,'cloud-tab:subscribed');
    await ui.click(page,`cloud-subscription:disable:${b.slug}`);
    await ui.widget(page,`cloud-subscription:enable:${b.slug}`,{enabled:true});
    await ui.click(page,'cloud-tab:mods');await ui.click(page,`cloud-subscription:subscribe:${c.slug}`);
    await ui.widget(page,`cloud-subscription:disable:${c.slug}`,{enabled:true});
    await until(()=>activeMods?.length===2&&activeMods.some(mod=>mod.id===a.id&&mod.version==='2.0.0')
      &&activeMods.some(mod=>mod.id===c.id)&&pending.size===0,'real replacement content set');
    await leaveCloud();await ui.click(page,'play');await ui.click(page,'continue');
    const mismatch=await ui.widget(page,'routing/dialog');assert.equal(mismatch.label,'THE MOD LIST HAS CHANGED');
    for(const text of ['ADDED','MISSING','CHANGED',`${c.id} v1.0.0`,`${b.id} v1.0.0`,`${a.id} v1.0.0 → v2.0.0`])
      assert.ok(mismatch.value.includes(text),`actual mismatch includes ${text}`);
    receipt.mismatch=mismatch.value;await shot('routing-save-mismatch');
    await ui.click(page,'routing-back');await ui.surface(page,'title');
    assert.deepEqual((await request('api/game/saves/0')).save,baseline,'cancel preserves the entire saved document/revision');
    await ui.click(page,'play');await ui.click(page,'continue');await ui.widget(page,'routing/dialog');
    await ui.click(page,'routing-continue');
    await ui.waitForUi(page,s=>s.surface==='hidden'&&!s.inputScope&&s.acceptedSelection?.name==='Lucian','same wizard with changed content');
    assert.deepEqual(admittedMods(),[`${a.id}@2.0.0`,`${c.id}@1.0.0`].sort());
    await savedLeave();receipt.checks.push('all three real mismatch groups render; Cancel preserves the full save and Continue admits Lucian with the current content');

    step='ordinary disable-all vanilla choice';
    await newGameRoute();await ui.click(page,'routing-vanilla');
    await ui.waitForUi(page,s=>ui.findWidget(s,'routing/dialog')?.label==='THE MOD LIST HAS CHANGED','vanilla removal mismatch');
    const removed=(await ui.widget(page,'routing/dialog')).value;
    for(const content of [a,c])assert.ok(removed.includes(content.id));
    await ui.click(page,'routing-continue');await finishCreate('Felix');
    assert.deepEqual(admittedMods(),[]);assert.equal(receipt.admissions.at(-1).cheats,false);
    assert.deepEqual((await request('api/mods/active')).mods,[]);
    receipt.vanilla=receipt.admissions.at(-1);await savedLeave();
    await ui.assertGpuOnly(page);receipt.checks.push('the normal vanilla choice disables all actual subscriptions, confirms removed save mods and saves a wizard without authored content');
    receipt.passed=true;
  } catch(error) {
    failure=error;receipt.step=step;receipt.failure=error.stack;
    if(page){receipt.failedUi=await ui.readUi(page).catch(e=>({error:e.message}));await shot('failure').catch(()=>{});}
  } finally {
    releaseDownload?.();
    if(page)try{await page.unrouteAll({behavior:'wait'});await ui.settleErrors(page);}catch(error){failure||=error;receipt.passed=false;}
    receipt.errors=observed?.errors||[];let consoleFaults=0;
    receipt.unexpectedErrors=receipt.errors.filter(error=>{
      if(error===fault.error)return false;
      return !(error.type==='console'&&error.location?.url===fault.response?.url&&error.message.includes('503')&&consoleFaults++===0);
    });
    receipt.fault=fault.request?{url:fault.request.url(),method:fault.request.method(),response:fault.response}:null;
    if(fault.request&&(!fault.error||fault.response?.status!==503))receipt.unexpectedErrors.push('Declared fault lacks exact response evidence');
    if(receipt.unexpectedErrors.length){failure||=new Error('Unexpected browser errors');receipt.passed=false;}
    try{receipt.cleanup=await owner.close();}catch(error){failure||=error;receipt.passed=false;receipt.cleanup=error.cleanup;}
    if(receipt.passed)for(const content of [a,b,c]) {
      await request(`api/mods/${content.slug}`,{method:'DELETE'});receipt.deletion.push({slug:content.slug,deleted:true});
    }
    fs.writeFileSync(path.join(output,'receipt.json'),JSON.stringify(receipt,null,2));
  }
  console.log(JSON.stringify({passed:receipt.passed,checks:receipt.checks,step,cleanup:receipt.cleanup}));
  if(failure)throw failure;
})().catch(error=>{console.error(error);process.exitCode=1;});
