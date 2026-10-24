// Browser capability boundaries only. No game state, action or renderer is replaced.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const ui = require('./gpu-ui.cjs');
const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
const output = path.resolve(process.env.OUT);
const url = new URL(process.env.URL);
const receipt = {passed:false, checks:[], errors:[]};

(async () => {
  fs.mkdirSync(output, {recursive:true});
  const owned = await launchOwnedBrowser(chromium, {headless:true,
    executablePath:process.env.CHROME, args:['--use-angle=metal','--ignore-gpu-blocklist']});
  let page, observed, failure;
  try {
    for (const mode of ['policy-denied','installed-platform','request-rejected']) {
      const context = await owned.browser.newContext({viewport:{width:1280,height:720},
        ignoreHTTPSErrors:process.env.LOCAL_TLS === '1'});
      page = await context.newPage();
      observed = ui.watchErrors(page); await observed.ready;
      if (mode !== 'request-rejected') {
        await page.route(address => address.origin === url.origin && address.pathname === url.pathname,
          async route => {
            const response = await route.fetch();
            await route.fulfill({response, headers:{...response.headers(), 'permissions-policy':'fullscreen=()'}});
          });
      }
      if (mode === 'installed-platform') {
        // Chromium has no navigator.standalone. Emulate that one iOS platform
        // capability; this is branch acceptance, not an installed-device claim.
        await page.addInitScript(() => Object.defineProperty(navigator,'standalone',{value:true}));
      }
      if (mode === 'request-rejected') {
        await page.addInitScript(() => {
          window.fullscreenActivation = [];
          window.consumeFullscreenActivation = false;
          window.addEventListener('click', event => {
            if (!window.consumeFullscreenActivation || !event.isTrusted) return;
            window.consumeFullscreenActivation = false;
            const before = navigator.userActivation.isActive;
            // A real popup consumes transient activation. The native fullscreen
            // promise then rejects through the browser's own permission check.
            const popup = window.open('about:blank','_blank','popup');
            window.fullscreenActivation.push({before,after:navigator.userActivation.isActive,
              opened:!!popup});
            popup?.close();
          }, true);
        });
      }
      await page.goto(url.href); await ui.ready(page);
      if (ui.findWidget(await ui.readUi(page),'prompt-secondary')) {
        await ui.click(page,'prompt-secondary'); await ui.noWidget(page,'prompt-secondary');
      }
      const capabilities = await page.evaluate(() => ({enabled:document.fullscreenEnabled,
        requestAvailable:typeof document.documentElement.requestFullscreen === 'function',
        standalone:navigator.standalone === true}));
      if (mode === 'policy-denied') {
        assert.equal(capabilities.enabled,false);
        assert.equal(capabilities.requestAvailable,true);
        assert.equal((await ui.widget(page,'fullscreen/toggle')).label,'Fullscreen options');
        await ui.click(page,'fullscreen/toggle');
        const help = await ui.widget(page,'fullscreen/message');
        assert.match(help.label,/Add to Home Screen/);
        await page.screenshot({path:path.join(output,'policy-denied-guidance.png')});
        await ui.click(page,'fullscreen/toggle'); await ui.noWidget(page,'fullscreen/message');
        await ui.click(page,'settings');
        await ui.widget(page,'settings-fullscreen');
        receipt.checks.push({mode,capabilities,help:help.label,settingsRow:true});
      } else if (mode === 'installed-platform') {
        assert.equal(capabilities.enabled,false); assert.equal(capabilities.standalone,true);
        await ui.noWidget(page,'fullscreen/toggle');
        await ui.click(page,'settings'); await ui.surface(page,'settings');
        await ui.noWidget(page,'settings-fullscreen');
        receipt.checks.push({mode,capabilities,
          qualification:'real Permissions Policy denial plus emulated navigator.standalone; no installed-device claim'});
      } else {
        assert.equal(capabilities.enabled,true);
        await page.evaluate(() => {window.consumeFullscreenActivation = true;});
        await ui.click(page,'fullscreen/toggle');
        const help = await ui.widget(page,'fullscreen/message');
        const activation = await page.evaluate(() => window.fullscreenActivation);
        assert.deepEqual(activation,[{before:true,after:false,opened:true}]);
        assert.match(help.label,/permission|gesture|activation/i);
        assert.equal(await page.evaluate(() => document.fullscreenElement),null);
        await page.screenshot({path:path.join(output,'actual-request-rejection.png')});
        await ui.click(page,'fullscreen/toggle');
        await page.waitForFunction(() => document.fullscreenElement === document.documentElement);
        await ui.noWidget(page,'fullscreen/message');
        await ui.click(page,'fullscreen/toggle');
        await page.waitForFunction(() => document.fullscreenElement === null);
        receipt.checks.push({mode,capabilities,activation,message:help.label,retry:true});
      }
      await ui.assertGpuOnly(page); await ui.settleErrors(page);
      assert.deepEqual(observed.errors,[]);
      await context.close(); page = null;
    }
    receipt.passed = true;
  } catch(error) {
    failure = error; receipt.failure = error.stack;
    if (page) receipt.ui = await ui.readUi(page).catch(() => null);
  } finally {
    if (page) {
      await ui.settleErrors(page); receipt.errors = observed.errors;
      await page.screenshot({path:path.join(output,'failure.png')}).catch(() => {});
    }
    try {receipt.cleanup = await owned.close();}
    catch(error) {failure ||= error; receipt.passed = false; receipt.cleanup = error.cleanup;}
    fs.writeFileSync(path.join(output,'receipt.json'),JSON.stringify(receipt,null,2));
  }
  console.log(JSON.stringify({passed:receipt.passed,checks:receipt.checks,cleanup:receipt.cleanup}));
  if (failure) throw failure;
})().catch(error => {console.error(error);process.exitCode = 1;});
