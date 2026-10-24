const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
// Real ordinary upload, protected gallery, versions and GPU comment ownership.
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const {execFileSync} = require('node:child_process');
const {chromium} = require(path.resolve(process.env.PW));
const gpu = require('./gpu-ui.cjs');
const game = new URL(process.env.URL);
const mount = game.pathname.replace(/\/$/, '');
const output = path.resolve(process.env.OUT);
fs.mkdirSync(output, {recursive:true});
const suffix = `${Date.now().toString(36)}${crypto.randomBytes(2).toString('hex')}`;
const account = {username:`Gallery${suffix}`, email:`gallery-${suffix}@example.invalid`,
  password:`Qa!${crypto.randomBytes(18).toString('base64url')}`};
fs.writeFileSync(path.join(output, 'private-account.json'), JSON.stringify(account), {mode:0o600,flag:'wx'});
const modName = `GPU Gallery ${suffix} é שלום`;
const checks = [], observations = [], pages = [], requests = [], comments = [];
let slug, token, step = 'ordinary registration', created, secondVersion;
let releasePost = () => {};
function checked(label) {checks.push(label);console.log(label);}

async function until(predicate, label, timeout = 45000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    if (predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 40));
  }
  throw new Error(`Timed out waiting for ${label}`);
}

function packageFile(version) {
  const file = path.join(output, `gallery-${version}.zip`);
  execFileSync('python3', ['-c', [
    'import json,sys,zipfile',
    'file,identity,name,version=sys.argv[1:]',
    'manifest={"id":identity,"name":name,"version":version,"runtime":{"apiVersion":"1.0.0","entryScript":"scripts/main.lua"},"requiredMods":[]}',
    'with zipfile.ZipFile(file,"w",zipfile.ZIP_DEFLATED) as archive:',
    ' archive.writestr("manifest.json",json.dumps(manifest))',
    ' archive.writestr("scripts/main.lua","-- Isolated menu content acceptance package.\\n")',
  ].join('\n'), file, `qa.gallery-${suffix}`, modName, version]);
  return file;
}

async function observed(context) {
  const page = await context.newPage();
  page.setDefaultTimeout(45000);
  const observation = gpu.watchErrors(page);
  await observation.ready;
  await page.addInitScript(()=>{
    const uploads=[];
    Object.defineProperty(window,'__cloudTextureUploads',{value:uploads});
    for(const type of [WebGLRenderingContext,WebGL2RenderingContext]) {
      const upload=type.prototype.texImage2D;
      type.prototype.texImage2D=function(...args){
        const result=Reflect.apply(upload,this,args);
        const source=args.at(-1);
        if(source instanceof HTMLImageElement && source.src.startsWith('blob:'))
          uploads.push({source:source.src,width:source.naturalWidth,height:source.naturalHeight});
        return result;
      };
    }
  });
  observations.push(observation);pages.push(page);
  page.on('response', async response => {
    const pathname = new URL(response.url()).pathname;
    const method = response.request().method();
    if (pathname.includes('/api/')) requests.push({path:pathname,method,status:response.status()});
    if (pathname.endsWith('/api/auth/register') && response.ok()) token = (await response.json()).token;
    if (slug && pathname === `${mount}/api/mods/${slug}/comments` && response.ok()) {
      const body = await response.json();
      comments.push({method,body});
    }
  });
  return page;
}

async function detail(page) {
  await gpu.ready(page);
  if (gpu.findWidget(await gpu.readUi(page),'prompt-secondary')) await gpu.click(page,'prompt-secondary');
  await gpu.click(page,'explore');
  await gpu.widget(page,`cloud-select:mod:${slug}`);
  await gpu.click(page,`cloud-open-mod:${slug}`);
  await gpu.value(page,'cloud-detail-title',modName);
  await gpu.value(page,'cloud-gallery-image','1 / 2');
  await page.waitForFunction(()=>new Set(window.__cloudTextureUploads
    .filter(upload=>upload.width>0&&upload.height>0).map(upload=>upload.source)).size>=2);
}

async function reveal(page,id) {
  for (let count=0;count<25;count++) {
    const state=await gpu.readUi(page);
    if (gpu.findWidget(state,id)) return;
    const title=gpu.bounds(await gpu.widget(page,'cloud-detail-title'));
    await page.mouse.move(title.x+title.width/2,title.y+title.height+160);
    await page.mouse.wheel(0,180);
    await gpu.nextFrame(page);
  }
  throw new Error(`The actual detail scroll did not reveal ${id}`);
}

async function swipe(page,distance) {
  const bounds=gpu.bounds(await gpu.widget(page,'cloud-gallery-image'));
  const start={x:bounds.x+bounds.width/2-distance/2,y:bounds.y+bounds.height/2};
  const input=await page.context().newCDPSession(page);
  try {
    await input.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{...start,id:1}]});
    await gpu.nextFrame(page);
    for(let index=1;index<=12;index++) {
      await input.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{
        x:start.x+distance*index/12,y:start.y,id:1}]});
      await gpu.nextFrame(page);
    }
    await input.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});
    await gpu.nextFrame(page);
  } finally {await input.detach();}
}

(async()=>{
  const owned = await launchOwnedBrowser(chromium,{headless:true,
    executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=metal','--ignore-gpu-blocklist']});
  const browser = owned.browser;
  const context=await browser.newContext({ignoreHTTPSErrors:process.env.LOCAL_TLS==='1',hasTouch:true,
    viewport:{width:1600,height:900}});
  let page, failure, cleanup;
  try {
    page=await observed(context);
    await page.goto(new URL(`${mount}/register`,game).href);
    for(const [autocomplete,value] of [['username',account.username],['email',account.email],['new-password',account.password]])
      await page.locator(`input[autocomplete="${autocomplete}"]`).fill(value);
    await page.getByRole('button',{name:'Create account',exact:true}).click();
    await page.waitForURL(url=>url.pathname===`${mount}/account`);
    await until(()=>typeof token==='string','actual registration token');
    checked('a new isolated account registers through the ordinary Website form');

    step='ordinary package and screenshot upload';
    await gpu.observeDocumentReplacement(page,()=>page.goto(new URL(`${mount}/mods/upload`,game).href));
    await page.locator('input[type=file][accept=".zip"]').setInputFiles(packageFile('1.0.0'));
    await page.getByLabel('Mod name',{exact:true}).fill(modName);
    await page.getByLabel(/^Summary/).fill('Ordinary gallery and comments acceptance content.');
    await page.getByLabel(/^Description/).fill('Two actual game screenshots and two real uploaded releases.\nThis package is removed after menu acceptance.');
    await page.locator('input[type=file][multiple]').setInputFiles([process.env.SHOT_A,process.env.SHOT_B]);
    const publication=page.waitForResponse(response=>new URL(response.url()).pathname===`${mount}/api/mods`
      &&response.request().method()==='POST');
    await page.getByRole('button',{name:'Publish mod',exact:true}).click();
    const response=await publication;
    assert.equal(response.status(),201);
    created=await response.json();slug=created.slug;
    assert.equal(created.screenshots.length,2);
    assert.equal(created.author.username,account.username);
    await page.waitForURL(url=>url.pathname===`${mount}/mods/${slug}`);
    const versions=page.locator(`a[href="${mount}/mods/${slug}/versions"]`).first();
    await versions.click();
    await page.getByLabel(/^Version/).fill('1.1.0');
    await page.getByLabel('ZIP file',{exact:true}).setInputFiles(packageFile('1.1.0'));
    await page.getByLabel('Changelog',{exact:true}).fill('Second ordinary release for native version history.');
    const versionPosting=page.waitForResponse(response=>new URL(response.url()).pathname===`${mount}/api/mods/${slug}/versions`
      &&response.request().method()==='POST');
    await page.getByRole('button',{name:'Publish version',exact:true}).click();
    const versionResponse=await versionPosting;
    assert.equal(versionResponse.status(),201);secondVersion=await versionResponse.json();
    assert.deepEqual(secondVersion.versions.map(version=>version.version),['1.1.0','1.0.0']);
    checked('ordinary Website file inputs publish the owned package, two screenshots and a second release');

    step='actual GPU protected gallery';
    await gpu.observeDocumentReplacement(page,async()=>{
      await page.getByRole('navigation').getByRole('link',{name:'Play',exact:true}).click();
      await page.waitForURL(url=>url.pathname===`${mount}/`);
    });
    await detail(page);
    await gpu.click(page,'cloud-gallery-previous');
    await gpu.value(page,'cloud-gallery-image','2 / 2');
    await gpu.click(page,'cloud-gallery-next');
    await gpu.value(page,'cloud-gallery-image','1 / 2');
    await gpu.click(page,'cloud-gallery:1');
    await gpu.value(page,'cloud-gallery-image','2 / 2');
    await page.keyboard.press('ArrowRight');
    await gpu.value(page,'cloud-gallery-image','1 / 2');
    await gpu.assertGpuOnly(page);
    await page.screenshot({path:path.join(output,'owned-gallery-desktop.png')});
    checked('protected screenshot textures render through GPU with previous/next wrap, thumbnail and real arrow input');

    step='portrait touch gallery';
    await gpu.click(page,'cloud-detail-done');
    await gpu.noWidget(page,'cloud-detail-title');
    await page.setViewportSize({width:390,height:844});
    await gpu.click(page,`cloud-open-mod:${slug}`);
    await gpu.value(page,'cloud-gallery-image','1 / 2');
    await swipe(page,-30);
    await gpu.value(page,'cloud-gallery-image','1 / 2');
    await swipe(page,-72);
    await gpu.value(page,'cloud-gallery-image','2 / 2');
    await swipe(page,72);
    await gpu.value(page,'cloud-gallery-image','1 / 2');
    await page.screenshot({path:path.join(output,'owned-gallery-touch.png')});
    await gpu.assertGpuOnly(page);
    checked('portrait gallery uses real multi-frame touch with the Source40px threshold and both directions');
    await page.setViewportSize({width:1600,height:900});
    await gpu.nextFrame(page);

    step='multiline comment and busy ownership';
    await reveal(page,'dark-cloud-comment');
    await gpu.fill(page,'dark-cloud-comment','😀'.repeat(501));
    await gpu.value(page,'dark-cloud-comment','😀'.repeat(500));
    const body='Ordinary GPU comment é\nשלום 😀';
    await gpu.fill(page,'dark-cloud-comment',body);
    await gpu.value(page,'dark-cloud-comment',body);
    await reveal(page,'cloud-comment-resize');
    const fieldBefore=gpu.bounds(await gpu.widget(page,'dark-cloud-comment'));
    const grip=gpu.center(gpu.bounds(await gpu.widget(page,'cloud-comment-resize')));
    await page.mouse.move(grip.x,grip.y);await page.mouse.down();
    await page.mouse.move(grip.x,grip.y+64,{steps:12});await page.mouse.up();
    await gpu.waitForUi(page,snapshot=>gpu.findWidget(snapshot,'dark-cloud-comment')?.paintRect[3]>=fieldBefore.height+60,
      'actual vertical textarea resize preserves its native minimum and current text');
    await gpu.value(page,'dark-cloud-comment',body);
    const postGate=new Promise(resolve=>{releasePost=resolve;});
    await context.route(`**/api/mods/${slug}/comments`,async route=>{
      if(route.request().method()==='POST')await postGate;
      await route.continue();
    });
    await reveal(page,'cloud-post-comment');
    await gpu.click(page,'cloud-post-comment');
    await gpu.waitForUi(page,snapshot=>gpu.findWidget(snapshot,'cloud-post-comment')?.enabled===false,
      'actual in-flight comment owns its draft');
    await gpu.fill(page,'dark-cloud-comment','Edited while posting');
    await gpu.value(page,'dark-cloud-comment','Edited while posting');
    releasePost();
    await until(()=>comments.some(row=>row.method==='POST'&&row.body.body===body),'actual comment POST receipt');
    const posted=comments.find(row=>row.method==='POST').body;
    await gpu.value(page,'dark-cloud-comment','');
    await reveal(page,`cloud-delete-comment:${posted.id}`);
    await page.screenshot({path:path.join(output,'owned-comment-and-versions.png')});
    await gpu.click(page,`cloud-delete-comment:${posted.id}`);
    await gpu.noWidget(page,`cloud-delete-comment:${posted.id}`);
    await until(()=>requests.some(request=>request.path.endsWith(`/comments/${posted.id}`)
      &&request.method==='DELETE'&&request.status===204),'actual author comment deletion');
    checked('real multiline UTF16-limited comments resize and remain editable during copied submission; success clears the draft and author Delete refreshes the actual list');

    step='guest comment sign-in';
    const guest=await browser.newContext({ignoreHTTPSErrors:process.env.LOCAL_TLS==='1',viewport:{width:1600,height:900}});
    const guestPage=await observed(guest);
    await guestPage.goto(game.href);await detail(guestPage);
    await reveal(guestPage,'cloud-detail-signin-comment');
    await gpu.observeDocumentReplacement(guestPage,()=>gpu.click(guestPage,'cloud-detail-signin-comment'));
    await guestPage.waitForURL(url=>url.pathname===`${mount}/login`);
    await guestPage.locator('input[autocomplete="username"]').waitFor();
    checked('the real guest comment action opens the ordinary Website sign-in form');
    for(const observedPage of pages)await gpu.settleErrors(observedPage);
    for(const observation of observations)assert.deepEqual(observation.errors,[]);
  }catch(error){
    failure={step,message:error.message,stack:error.stack};
    if(page&&!page.isClosed()){
      failure.ui=await gpu.readUi(page).catch(()=>null);
      await page.screenshot({path:path.join(output,'failure.png')}).catch(()=>{});
    }
    throw error;
  }finally{
    releasePost();
    try{
      for(const observedPage of pages)if(!observedPage.isClosed())await gpu.settleErrors(observedPage);
      if(slug){
        const response=await context.request.delete(new URL(`api/mods/${slug}`,game).href,
          {headers:{Authorization:`Bearer ${token}`}});
        cleanup={slug,status:response.status()};assert.equal(response.status(),204);
      }
      fs.writeFileSync(path.join(output,'cloud-content.json'),JSON.stringify({passed:!failure,checks,failure,
        mod:{slug,name:modName,screenshots:created?.screenshots,versions:secondVersion?.versions},
        requests,comments,cleanup,errors:observations.flatMap(observation=>observation.errors)},null,2));
      console.log(JSON.stringify({passed:!failure,checks,failure:failure?.message.split(': {')[0],cleanup}));
    }finally{console.log(JSON.stringify({browserCleanup:await owned.close()}));}
  }
})().catch(error=>{console.error(error.message.split(': {')[0]);process.exitCode=1;});
