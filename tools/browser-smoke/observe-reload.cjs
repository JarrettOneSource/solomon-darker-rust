// Private bounded subset of the ordinary settings journey to locate slow reloads.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
const gpu = require('./gpu-ui.cjs');
const output = process.env.OUT;

(async () => {
  const owned = await launchOwnedBrowser(chromium, {headless: true,
    executablePath: process.env.CHROME,
    args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist']});
  const receipt = {pass: false, checks: []};
  let page;
  try {
    page = await owned.browser.newPage({viewport: {width: 1600, height: 900}});
    const {errors, ready} = gpu.watchErrors(page);
    receipt.errors = errors;
    await ready;
    await page.goto(process.env.URL);
    await gpu.ready(page);
    await page.screenshot({timeout: 120000, path: path.join(output, 'title.png')});
    if (gpu.findWidget(await gpu.readUi(page), 'prompt-secondary')) await gpu.click(page, 'prompt-secondary');
    await gpu.click(page, 'settings');
    await gpu.setSlider(page, 'settings-range:sound', 37);
    await gpu.value(page, 'settings-range:sound', '37');
    await page.screenshot({timeout: 120000, path: path.join(output, 'settings.png')});
    await gpu.click(page, 'settings-back');
    await gpu.surface(page, 'title');
    receipt.checks.push('ordinary settings input stored 37% sound');
    await gpu.settleErrors(page);
    await gpu.observeDocumentReplacement(page, () => page.reload());
    await gpu.ready(page);
    await page.screenshot({timeout: 120000, path: path.join(output, 'reloaded.png')});
    if (gpu.findWidget(await gpu.readUi(page), 'prompt-secondary')) await gpu.click(page, 'prompt-secondary');
    await gpu.click(page, 'settings');
    await gpu.value(page, 'settings-range:sound', '37');
    await gpu.settleErrors(page);
    assert.deepEqual(errors, []);
    receipt.checks.push('actual document reload and retained setting');
    receipt.pass = true;
  } catch (error) {
    receipt.failure = String(error.stack || error);
    if (page && !page.isClosed()) {
      try {await page.screenshot({timeout: 120000, path: path.join(output, 'failure.png')});}
      catch (captureError) {receipt.captureFailure = String(captureError);}
    }
    process.exitCode = 1;
  } finally {
    try {if (page && !page.isClosed()) await gpu.settleErrors(page);}
    catch (error) {receipt.settleFailure = String(error);receipt.pass = false;process.exitCode = 1;}
    try {receipt.browserCleanup = await owned.close();}
    catch (error) {receipt.browserCleanup = error.cleanup || {failure: String(error)};receipt.pass = false;process.exitCode = 1;}
    fs.writeFileSync(path.join(output, 'receipt.json'), JSON.stringify(receipt, null, 2)+'\n');
    console.log(JSON.stringify({pass: receipt.pass, checks: receipt.checks.length}));
  }
})().catch(error => {console.error(error); process.exitCode = 1;});
