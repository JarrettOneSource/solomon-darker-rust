// Real Create variants: stock names, clear/focus, native randomization and Genericus.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const gpu = require('./gpu-ui.cjs');
const {reachCollegeLoadout, startNewGame} = require('./menu-journey.cjs');
const output = process.env.OUT || '/tmp';
fs.mkdirSync(output, {recursive:true});

(async () => {
  const browser = await chromium.launch({headless:true,
    executablePath:process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=metal','--ignore-gpu-blocklist']});
  const receipt = {checks:[]};
  let page;
  try {
    page = await browser.newPage({viewport:{width:1600,height:900},
      ignoreHTTPSErrors:process.env.LOCAL_TLS === '1'});
    const observed = gpu.watchErrors(page);
    receipt.errors = observed.errors;
    await page.goto(process.env.URL || 'http://127.0.0.1:4181/');
    await gpu.ready(page);
    await startNewGame(page, {introOnly:true});
    await reachCollegeLoadout(page);
    await gpu.fill(page, 'create-name', 'Zzz');
    await gpu.value(page, 'create-name', 'Zzz');
    const response = await page.request.get(new URL('assets/stock/menu/wizard-names.json', page.url()).href);
    assert.ok(response.ok());
    const stockNames = await response.json();
    assert.ok(!stockNames.includes('Zzz'), 'the input witness is outside the stock name pool');
    await gpu.click(page, 'randomize-name');
    await gpu.waitForUi(page, snapshot => {
      const value = gpu.findWidget(snapshot, 'create-name')?.value;
      return typeof value === 'string' && stockNames.includes(value);
    }, 'the actual random-name button accepts a stock name');
    receipt.stockName = (await gpu.widget(page, 'create-name')).value;
    await gpu.click(page, 'clear-name');
    await gpu.value(page, 'create-name', '');
    await gpu.focused(page, 'create-name');
    receipt.checks.push('stock-name randomization and Clear restores actual text focus');
    await page.screenshot({path:path.join(output, 'create-cleared-name.png')});
    await gpu.fill(page, 'create-name', 'Felix');
    await gpu.value(page, 'create-name', 'Felix');
    await gpu.click(page, 'randomize-loadout');
    await gpu.surface(page, 'hidden');
    const randomized = await gpu.waitForUi(page, snapshot => snapshot.acceptedSelection?.name === 'Felix',
      'native randomized admission retains the accepted name');
    receipt.randomized = randomized.acceptedSelection;
    assert.ok(['Fire','Water','Air','Earth','Ether'].includes(receipt.randomized.element));
    assert.ok(['Body','Mind','Arcane'].includes(receipt.randomized.discipline));
    receipt.checks.push('whole-loadout randomization retains the name and commits an actual native loadout');
    await page.screenshot({path:path.join(output, 'create-randomized-admitted.png')});
    // Each variant enters through an ordinary fresh-player College journey.
    // The first-visit dialogue is not repeated by an existing player's New Game.
    receipt.errors = [...observed.errors];
    await page.close();
    page = await browser.newPage({viewport:{width:1600,height:900},
      ignoreHTTPSErrors:process.env.LOCAL_TLS === '1'});
    gpu.watchErrors(page, {errors:receipt.errors});
    await page.goto(process.env.URL || 'http://127.0.0.1:4181/');
    await gpu.ready(page);
    receipt.blank = await startNewGame(page, {name:'',element:'earth',discipline:'mind'});
    assert.deepEqual(receipt.blank, {name:'Genericus',element:'Earth',discipline:'Mind'});
    receipt.checks.push('an actual blank draft admits Genericus with the requested Earth/Mind selection');
    await gpu.assertGpuOnly(page);
    await page.screenshot({path:path.join(output, 'create-genericus-admitted.png')});
    assert.deepEqual(receipt.errors, []);
    console.log(JSON.stringify(receipt));
  } catch (error) {
    receipt.failure = String(error);
    if (page && !page.isClosed()) {
      receipt.actual = await gpu.readUi(page).catch(() => null);
      await page.screenshot({path:path.join(output, 'create-loadout-failure.png')});
    }
    throw error;
  } finally {
    fs.writeFileSync(path.join(output, 'create-loadout-receipt.json'), JSON.stringify(receipt,null,2));
    await browser.close();
  }
})().catch(error => {console.error(error);process.exitCode = 1});
