// Real browser fullscreen calls observed through the GPU control's public input.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const ui = require('./gpu-ui.cjs');
const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
const output = path.resolve(process.env.OUT);
const receipt = {passed:false, checks:[]};

(async () => {
  fs.mkdirSync(output, {recursive:true});
  const owned = await launchOwnedBrowser(chromium, {headless:true,
    executablePath:process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=metal','--ignore-gpu-blocklist']});
  let page, observed, failure;
  try {
    page = await owned.browser.newPage({viewport:{width:1280,height:720},hasTouch:true,
      ignoreHTTPSErrors:process.env.LOCAL_TLS === '1'});
    observed = ui.watchErrors(page); await observed.ready;
    await page.addInitScript(() => {
      window.fullscreenTrace = [];
      for (const [prototype, name] of [[Element.prototype,'requestFullscreen'],[Document.prototype,'exitFullscreen']]) {
        const original = prototype[name];
        prototype[name] = function(...args) {
          window.fullscreenTrace.push({kind:name,at:performance.now(),active:!!document.fullscreenElement});
          return Reflect.apply(original,this,args);
        };
      }
      document.addEventListener('fullscreenchange',() => window.fullscreenTrace.push({kind:'change',
        at:performance.now(),active:!!document.fullscreenElement}));
    });
    await page.goto(process.env.URL); await ui.ready(page);
    if (ui.findWidget(await ui.readUi(page),'prompt-secondary')) {
      await ui.click(page,'prompt-secondary'); await ui.noWidget(page,'prompt-secondary');
    }
    for (const [name, activate] of [['pointer',ui.click],['touch',ui.tap]]) {
      const before = await page.evaluate(() => window.fullscreenTrace.length);
      await activate(page,'fullscreen/toggle');
      await page.waitForFunction(() => document.fullscreenElement === document.documentElement);
      await ui.value(page,'fullscreen/toggle','true',{timeout:5000});
      await ui.nextFrame(page); await ui.nextFrame(page);
      assert.equal(await page.evaluate(() => document.fullscreenElement === document.documentElement),true);
      assert.equal((await ui.widget(page,'fullscreen/toggle')).label,'Exit fullscreen');
      await activate(page,'fullscreen/toggle');
      await page.waitForFunction(() => document.fullscreenElement === null);
      await ui.value(page,'fullscreen/toggle','false',{timeout:5000});
      const trace = await page.evaluate(before => window.fullscreenTrace.slice(before),before);
      assert.deepEqual(trace.filter(row => row.kind !== 'change').map(row => row.kind),
        ['requestFullscreen','exitFullscreen'],'each actual activation calls its browser capability once');
      receipt.checks.push({name,trace});
    }
    await ui.assertGpuOnly(page); await ui.settleErrors(page);
    assert.deepEqual(observed.errors,[]); receipt.passed = true;
  } catch(error) {
    failure = error; receipt.failure = error.stack;
    if (page) {
      receipt.ui = await ui.readUi(page).catch(() => null);
      await page.screenshot({path:path.join(output,'fullscreen-failure.png')}).catch(() => {});
    }
  } finally {
    if (page) {
      receipt.trace = await page.evaluate(() => window.fullscreenTrace).catch(() => null);
      await ui.settleErrors(page);
      receipt.errors = observed.errors;
      if (observed.errors.length) {failure ||= new Error(JSON.stringify(observed.errors)); receipt.passed = false;}
    }
    try {receipt.cleanup = await owned.close();}
    catch(error) {failure ||= error; receipt.passed = false; receipt.cleanup = error.cleanup;}
    fs.writeFileSync(path.join(output,'receipt.json'),JSON.stringify(receipt,null,2));
  }
  console.log(JSON.stringify({passed:receipt.passed,checks:receipt.checks,trace:receipt.trace,cleanup:receipt.cleanup}));
  if (failure) throw failure;
})().catch(error => {console.error(error);process.exitCode = 1;});
