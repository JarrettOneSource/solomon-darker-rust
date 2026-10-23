// Process-start acceptance from Website fa08 renderer/loader-render-contract.ts and
// pages/Game.tsx. No timer-driven progress and no Title before required readiness.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const ui = require('./gpu-ui.cjs');
const url = process.env.URL || 'http://127.0.0.1:4181/';
const output = process.env.OUT || '/tmp/solomon-gpu-startup';
const allCases=['cold','warm','delayedTitle','delayedFont','delayedWasm','delayedResidentMusic','failedRequiredTitle'];
const selectedCases=process.env.CASES?process.env.CASES.split(',').map(value=>value.trim()):allCases;
assert.ok(selectedCases.length>0&&selectedCases.every(value=>allCases.includes(value)),
  `CASES must name existing startup cases: ${allCases.join(',')}`);
const selected=name=>selectedCases.includes(name);
const source = path.resolve(__dirname, '../../web/assets/stock/loader-logo.png');
const expected = {
  'loader:logo': [41,13,388,227],
  'loader:url': [119,251,244,18],
  'loader:frame': [125,263,230,54],
};
fs.mkdirSync(output, {recursive:true});

function loaderGeometry(snapshot) {
  const scale = Math.min(snapshot.canvas.width / 1600, snapshot.canvas.height / 900);
  const x = (snapshot.canvas.width - 480 * scale) / 2;
  const y = (snapshot.canvas.height - 320 * scale) / 2;
  for (const [id, reference] of Object.entries(expected)) {
    const actual = ui.findWidget(snapshot, id);
    assert.ok(actual, `native Loader actually paints ${id}`);
    const rect = [x + reference[0] * scale, y + reference[1] * scale,
      reference[2] * scale, reference[3] * scale];
    actual.rect.forEach((value,index) => assert.ok(Math.abs(value-rect[index]) < 0.02,
      `${id}[${index}] matches the recovered native geometry: ${value} vs ${rect[index]}`));
  }
  const fill = ui.findWidget(snapshot, 'loader:fill');
  const width = 192 * snapshot.loading.progress * scale;
  if(width===0&&!fill)return;
  assert.ok(fill, 'native clipped fill is painted');
  assert.ok(Math.abs(fill.rect[0] - (x+144*scale)) < 0.02
    && Math.abs(fill.rect[1] - (y+282*scale)) < 0.02
    && Math.abs(fill.rect[2] - width) < 0.02
    && Math.abs(fill.rect[3] - 18*scale) < 0.02, 'fill geometry reports the actual work-bound clip');
}

function pending(snapshot) {
  assert.equal(snapshot.surface, 'loading');
  assert.equal(snapshot.ready, false);
  assert.ok(snapshot.loading, 'loading owns a truthful readiness report');
  const load = snapshot.loading;
  assert.ok(Number.isInteger(load.completed) && Number.isInteger(load.total)
    && load.total >= 0 && load.completed >= 0 && load.completed <= load.total,
  'readiness counts concrete completed work');
  const progress=load.total===0?0:load.completed/load.total;
  assert.ok(Math.abs(load.progress-progress) < 1e-7,
    'progress is completed work divided by the complete manifest');
  assert.ok(load.stage.length > 0, 'current loading stage is visible');
  if (load.completed < load.total) {
    assert.ok(load.currentItem, 'a real pending item identifies incomplete work');
    assert.ok(load.progress < 1, 'incomplete work never claims completion');
  }
  assert.ok(!snapshot.widgets.some(widget => widget.id === 'play'
    || widget.id.startsWith('hud:')), 'no partial title or HUD is exposed while loading');
  if(ui.findWidget(snapshot,'loader:logo'))loaderGeometry(snapshot);
}

async function visual(page, name) {
  const screenshot = await page.screenshot({path:path.join(output,`${name}.png`)});
  const result = await page.evaluate(async ({screen,logo}) => {
    const bitmap = async value => createImageBitmap(new Blob([
      Uint8Array.from(atob(value), character => character.charCodeAt(0)),
    ], {type:'image/png'}));
    const actualImage = await bitmap(screen);
    const sourceImage = await bitmap(logo);
    const actual = new OffscreenCanvas(actualImage.width,actualImage.height).getContext('2d');
    actual.drawImage(actualImage,0,0);
    const reference = new OffscreenCanvas(388,227).getContext('2d');
    reference.fillStyle = '#000054';
    reference.fillRect(0,0,388,227);
    reference.drawImage(sourceImage,0,0,388,227);
    const x = (actualImage.width-480)/2+41;
    const y = (actualImage.height-320)/2+13;
    const painted = actual.getImageData(x,y,388,227).data;
    const wanted = reference.getImageData(0,0,388,227).data;
    let maximum=0,differing=0;
    for(let index=0;index<painted.length;index+=4) {
      let different=false;
      for(let channel=0;channel<4;channel++) {
        const difference=Math.abs(painted[index+channel]-wanted[index+channel]);
        maximum=Math.max(maximum,difference);
        different ||= difference>0;
      }
      if(different)differing++;
    }
    return {corner:Array.from(actual.getImageData(0,0,1,1).data),maximum,differing,
      pixels:388*227,width:actualImage.width,height:actualImage.height};
  },{screen:screenshot.toString('base64'),logo:fs.readFileSync(source).toString('base64')});
  assert.deepEqual(result.corner,[0,0,84,255],'the native blue fills the loading viewport');
  // Canvas2D reference and premultiplied WebGL may round a channel by one byte.
  assert.ok(result.maximum<=1,`native Loader logo pixels match source: ${JSON.stringify(result)}`);
  const draws=await ui.gpuDraws(page);
  assert.ok(draws.canvasCalls['[data-gpu-loader]']>0,'the actual Rust loader canvas submitted GPU draws');
  return {...result,draws};
}

async function finishedLoader(page) {
  await page.waitForFunction(()=>!document.querySelector('canvas[data-gpu-loader]'));
  const before=(await ui.gpuDraws(page)).canvasCalls['[data-gpu-loader]']||0;
  await ui.nextFrame(page);
  const after=(await ui.gpuDraws(page)).canvasCalls['[data-gpu-loader]']||0;
  assert.equal(after,before,'the retired loader stops submitting GPU frames');
  return {removed:true,drawsBefore:before,drawsAfter:after};
}

async function collect(page, name, navigate) {
  const observed=ui.watchErrors(page);
  const samples=[];
  await navigate();
  const deadline=Date.now()+240000;
  let previous=0;
  let previousProgress=0;
  let lastLoading;
  while(Date.now()<deadline) {
    const snapshot=await ui.readUi(page);
    if(snapshot?.error)throw new Error(snapshot.error);
    if(snapshot?.loading) {
      pending(snapshot);
      assert.ok(snapshot.loading.completed>=previous,'startup never regresses across batches');
      assert.ok(snapshot.loading.progress+1e-7>=previousProgress,
        'the overall fill never regresses when the stage or pending item changes');
      previous=snapshot.loading.completed;
      previousProgress=snapshot.loading.progress;
      lastLoading=snapshot;
      samples.push(snapshot);
    }
    if(snapshot?.ready) {
      assert.equal(snapshot.loading,null);
      assert.equal(snapshot.surface,'title');
      assert.ok(snapshot.draw.triangles>0&&snapshot.draw.textures>0,'ready includes actual GPU painting');
      const handoff=await finishedLoader(page);
      await ui.assertGpuOnly(page);
      await page.screenshot({path:path.join(output,`${name}-title.png`)});
      assert.deepEqual(observed.errors,[],'ordinary startup has no runtime or required asset errors');
      return {samples,lastLoading,ready:snapshot,handoff,errors:observed.errors,warnings:observed.warnings,
        submissions:await ui.gpuDraws(page)};
    }
    await page.waitForTimeout(50);
  }
  throw new Error(`${name} did not complete: ${JSON.stringify(samples.at(-1))}`);
}

async function heldAsset(browser, name, pattern, {wasm=false, fallback=false}={}) {
  const context=await browser.newContext({viewport:{width:1600,height:900},deviceScaleFactor:1});
  const page=await context.newPage();
  await ui.observeGpu(page);
  await ui.observeAnnouncements(page,{forceFallback:fallback});
  const observed=ui.watchErrors(page);
  let release;
  const held=new Promise(resolve=>{release=resolve});
  let arrived;
  const requested=new Promise(resolve=>{arrived=resolve});
  let requestUrl;
  await page.route(pattern,async route=>{
    requestUrl=route.request().url();arrived();await held;await route.continue();
  });
  try {
    await page.goto(url,{waitUntil:'domcontentloaded'});
    await Promise.race([requested,page.waitForTimeout(45000).then(()=>{
      throw new Error(`${name} never requested the expected required asset`);
    })]);
    let before;
    if(!wasm) {
      before=await ui.waitForUi(page,snapshot=>snapshot.loading
        && ui.findWidget(snapshot,'loader:logo'),`${name} native loader`,{timeout:45000});
      pending(before);
    }
    await page.waitForFunction(()=>window.__solomonGpuDraws?.canvasCalls['[data-gpu-loader]']>0,
      null,{timeout:45000});
    const first=await visual(page,`${name}-held`);
    await page.waitForTimeout(500);
    const still=await ui.readUi(page);
    if(still) {
      pending(still);
      if(before)assert.ok(still.loading.completed>=before.loading.completed,
        'parallel completions cannot regress while another required item remains held');
      if(still.loading.total>0)assert.ok(still.loading.completed<still.loading.total,
        'a held required item prevents completion regardless of elapsed time');
      else assert.equal(still.loading.progress,0,'unknown code loading invents no percentage');
    }
    const second=await visual(page,`${name}-still-held`);
    let heldAnnouncements;
    if(fallback) {
      await page.waitForFunction(()=>window.__solomonAnnouncementOwnership
        .some(row=>row.owners>=2&&row.loaders===1&&row.count===1));
      heldAnnouncements=await page.evaluate(()=>({
        events:window.__solomonAnnouncements,
        owners:window.__solomonAnnouncementOwnership,
      }));
      assert.ok(heldAnnouncements.events.length>0,'the actual loader stage is announced');
      assert.ok(heldAnnouncements.owners.every(row=>row.count<=1),
        'the tiny loader and main share exactly one nonvisual fallback');
      assert.ok(heldAnnouncements.events.every(event=>event.loaderActive
        &&!/(?:, button|, textbox|, tab|, checkbox)/i.test(event.text)),
      'only visible loader work is announced before the actual handoff');
    }
    const resizes=[];
    if(wasm) {
      for(const viewport of [{width:1920,height:1080},{width:390,height:844}]) {
        await page.setViewportSize(viewport);
        const resized=await ui.waitForUi(page,snapshot=>snapshot.loading
          &&snapshot.canvas.width===viewport.width&&snapshot.canvas.height===viewport.height,
        'the actual loader canvas resize');
        pending(resized);
        await page.screenshot({path:path.join(output,
          `${name}-${viewport.width}x${viewport.height}.png`)});
        resizes.push(resized);
      }
      await page.setViewportSize({width:1600,height:900});
      await ui.waitForUi(page,snapshot=>snapshot.canvas.width===1600&&snapshot.canvas.height===900,
        'the restored loading viewport');
    }
    release();
    await ui.ready(page);
    await ui.surface(page,'title');
    const handoff=await finishedLoader(page);
    let announcementHandoff;
    if(fallback) {
      await page.waitForFunction(()=>{
        const shared=document[Symbol.for('solomon.graphics.announcements')];
        return shared?.owners===1&&shared.loaders===0
          &&document.querySelectorAll('[data-gpu-announcer]').length===1;
      });
      const afterIndex=(await ui.announcements(page)).length;
      await page.keyboard.press('Tab');
      const focused=await ui.waitForUi(page,snapshot=>snapshot.widgets.some(widget=>widget.focused),
        'the actual first focused GPU control after loading');
      const control=focused.widgets.find(widget=>widget.focused);
      await ui.announced(page,control.label,{afterIndex});
      announcementHandoff=await page.evaluate(()=>({
        events:window.__solomonAnnouncements,
        owners:window.__solomonAnnouncementOwnership,
      }));
      assert.ok(announcementHandoff.owners.every(row=>row.count<=1));
      assert.ok(announcementHandoff.events.slice(afterIndex).some(event=>!event.loaderActive
        &&event.text.toLowerCase().includes(control.label.toLowerCase())),
      'the retained main owner announces real control focus after loader retirement');
    }
    await ui.assertGpuOnly(page);
    await page.screenshot({path:path.join(output,`${name}-title.png`)});
    assert.deepEqual(observed.errors,[],'delayed successful work has no runtime/asset errors');
    return {requestUrl,before,still,first,second,resizes,handoff,heldAnnouncements,announcementHandoff,
      ready:await ui.readUi(page),...observed};
  } finally {release();await context.close()}
}

async function failedAsset(browser) {
  const context=await browser.newContext({viewport:{width:1600,height:900},deviceScaleFactor:1});
  const page=await context.newPage();
  await ui.observeGpu(page);
  const observed=ui.watchErrors(page);
  let failedUrl;
  await page.route('**/assets/stock/Title.png',route=>{
    failedUrl=route.request().url();
    return route.fulfill({status:503,body:'Required-title availability acceptance fixture'});
  });
  try {
    await page.goto(url,{waitUntil:'domcontentloaded'});
    await page.waitForFunction(()=>{
      const read=window.wasmBindings?.gpu_ui_state;
      return read&&JSON.parse(read()).error;
    },null,{timeout:90000});
    const snapshot=await ui.readUi(page);
    assert.equal(snapshot.ready,false);
    assert.equal(snapshot.surface,'loading');
    assert.ok(snapshot.error.includes('Title.png'),'the real failed asset owns the error');
    assert.ok(!snapshot.widgets.some(widget=>widget.id==='play'||widget.id.startsWith('hud:')),
      'a required asset failure never exposes partial menus or a playable game');
    const display=ui.findWidget(snapshot,'loader:error');
    assert.ok(display&&display.label.includes('Title.png'),'the GPU paints the actionable failure');
    await ui.assertGpuOnly(page);
    await page.screenshot({path:path.join(output,'failed-required-title.png')});
    const unexpected=observed.errors.filter(error=>{
      if(error.type==='response')return !(error.url===failedUrl&&error.status===503);
      if(error.type==='console')return !(error.location.url===failedUrl
        ||error.message.includes('assets/stock/Title.png'));
      return true;
    });
    assert.ok(observed.errors.some(error=>error.type==='response'
      &&error.url===failedUrl&&error.status===503),'the deliberately failed request is retained');
    assert.deepEqual(unexpected,[],'all errors outside the deliberately failed resource remain fatal');
    return {failedUrl,snapshot,errors:observed.errors,warnings:observed.warnings,
      ownership:'This case deliberately returns HTTP503 only for the required Title.png request; its exact response/console failure is expected. No other error or cancellation is suppressed.'};
  } finally {await context.close()}
}

async function main() {
  const browser=await chromium.launch({headless:true,
    executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
  const receipt={source:'Website fa08ae24 loader-render-contract.ts, loader-renderer.ts, pages/Game.tsx; native ledger041',
    url,cases:selectedCases,scope:process.env.CASES?'focused startup cases':'complete startup matrix'};
  try {
    if(selected('cold')||selected('warm')) {
    const context=await browser.newContext({viewport:{width:1600,height:900},deviceScaleFactor:1});
    const page=await context.newPage();
    await ui.observeGpu(page);
    await ui.observeAnnouncements(page);
    if(selected('cold'))receipt.cold=await collect(page,'cold',()=>page.goto(url,{waitUntil:'domcontentloaded'}));
    if(selected('warm')) {
      if(!selected('cold'))receipt.warmPrimer=await collect(page,'warm-primer',()=>page.goto(url,{waitUntil:'domcontentloaded'}));
      receipt.warm=await collect(page,'warm',()=>page.reload({waitUntil:'domcontentloaded'}));
    }
    receipt.announcements=await page.evaluate(()=>({platform:window.__solomonAnnouncementPlatform,
      ownership:window.__solomonAnnouncementOwnership,events:window.__solomonAnnouncements}));
    if(receipt.announcements.platform.nativeAvailable) {
      assert.ok(receipt.announcements.ownership.every(row=>row.count===0),
        'a supported native ariaNotify path mounts no fallback text node');
      assert.ok(receipt.announcements.events.some(event=>event.source==='ariaNotify'),
        'ordinary loader progress uses the actual browser native announcement capability');
    }
    await context.close();
    }
    if(selected('delayedTitle'))receipt.delayedTitle=await heldAsset(browser,'delayed-title','**/assets/stock/Title.png');
    if(selected('delayedFont'))receipt.delayedFont=await heldAsset(browser,'delayed-font','**/assets/stock/fonts/cinzel-latin-500-normal.woff2');
    if(selected('delayedWasm'))receipt.delayedWasm=await heldAsset(browser,'delayed-wasm',
      /\/solomon[-_]web(?:-[a-z0-9]+)?(?:_bg)?\.wasm(?:\?.*)?$/,{wasm:true});
    if(selected('delayedResidentMusic'))receipt.delayedResidentMusic=await heldAsset(browser,'delayed-resident-music',
      '**/assets/stock/audio/music/solomondarktheme.mp3',{fallback:true});
    if(selected('failedRequiredTitle'))receipt.failedRequiredTitle=await failedAsset(browser);
    receipt.passed=true;
  } catch(error) {receipt.passed=false;receipt.failure=error.stack;throw error}
  finally {
    fs.writeFileSync(path.join(output,'startup-receipt.json'),JSON.stringify(receipt,null,2));
    await browser.close();
  }
  console.log(JSON.stringify({passed:receipt.passed,output}));
}
module.exports={loaderGeometry,pending,visual,finishedLoader,collect,heldAsset,failedAsset};
if(require.main===module)main().catch(error=>{console.error(error);process.exitCode=1});
