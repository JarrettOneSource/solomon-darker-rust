// Actual Website directory data and GPU input; never joins an unrelated party.
const assert = require('node:assert/strict');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const gpu = require('./gpu-ui.cjs');
(async () => {
  const browser = await chromium.launch({headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
  try {
    const page = await browser.newPage({viewport: {width: 1600, height: 900}, ignoreHTTPSErrors: process.env.LOCAL_TLS === '1'});
    const {errors} = gpu.watchErrors(page);
    const responses = [];
    page.on('response', response => {
      if (new URL(response.url()).pathname.endsWith('/api/game/parties') && response.ok()) responses.push(response.json());
    });
    await page.goto(process.env.URL || 'http://127.0.0.1:4181/');
    await gpu.ready(page);
    if (gpu.findWidget(await gpu.readUi(page), 'prompt-secondary')) await gpu.click(page, 'prompt-secondary');
    await gpu.click(page, 'play');
    await gpu.click(page, 'join-party');
    await gpu.surface(page, 'join-party');
    await gpu.waitForUi(page, () => responses.length > 0, 'real directory response');
    const directory = await responses.at(-1);
    await gpu.nextFrame(page);
    const visible = (await gpu.readUi(page)).widgets.filter(w => w.id.startsWith('join-listing:'));
    const byId = new Map(directory.items.map(party => [party.id, party]));
    for (const control of visible) {
      const party = byId.get(control.id.slice('join-listing:'.length));
      assert.ok(party, 'only actual broker listings are displayed');
      assert.equal(control.label, party.status === 'playing' ? 'IN GAME' : party.visibility === 'public' ? 'JOIN' : 'REQUEST');
      assert.equal(control.enabled, party.status !== 'playing');
    }
    await gpu.fill(page, 'join-party-code', '0I-abcd-efgh');
    await gpu.value(page, 'join-party-code', 'ABCD-EFGH');
    await gpu.focused(page, 'join-party-code');
    assert.equal((await gpu.widget(page, 'join-code')).enabled, true);
    await gpu.fill(page, 'join-party-code', '0I');
    await gpu.value(page, 'join-party-code', '');
    assert.equal((await gpu.widget(page, 'join-code')).enabled, false);
    await page.keyboard.press('Enter');
    await gpu.nextFrame(page);
    await page.keyboard.press('Enter');
    await gpu.nextFrame(page);
    await gpu.focused(page, 'join-party-code');
    await page.screenshot({path: path.join(process.env.OUT || '/tmp', 'join-party-desktop.png')});
    const beforeRefresh = responses.length;
    await gpu.click(page, 'join-refresh');
    await gpu.waitForUi(page, () => responses.length > beforeRefresh, 'manual directory refresh');
    await page.setViewportSize({width: 896, height: 414});
    await gpu.nextFrame(page);
    const list = gpu.bounds(await gpu.widget(page, 'join-party-list'));
    assert.ok(list.y >= 0 && list.y + list.height <= 414);
    await gpu.wheel(page, 'join-party-list', 0, 300);
    await page.screenshot({path: path.join(process.env.OUT || '/tmp', 'join-party-landscape.png')});
    await gpu.click(page, 'join-back');
    await gpu.surface(page, 'play');
    await gpu.assertGpuOnly(page);
    assert.deepEqual(errors, []);
    console.log(JSON.stringify({checks: ['actual broker data', 'listing policy', 'normalized real paste', 'editable focus without submission', 'incomplete code disabled and Enter retains editing focus', 'manual refresh', 'compact viewport/scroll', 'Back to Play'], actualParties: directory.items.length, errors}));
  } finally { await browser.close(); }
})().catch(error => {console.error(error); process.exitCode = 1;});
