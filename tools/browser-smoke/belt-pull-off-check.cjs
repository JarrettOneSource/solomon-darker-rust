// Call with a genuinely admitted fresh QA wizard in College. This uses ordinary
// pointer input and observes the accepted HUD; it never sends gameplay commands.
const assert = require('node:assert/strict');

async function checkNativeBeltPullOff(page, slot = 0) {
  const button = page.locator(`.hud-belt-slot[data-belt-index="${slot}"]`);
  await button.waitFor();
  assert.equal(await button.isDisabled(), false, 'College spells remain removable');
  assert.equal(await button.getAttribute('data-belt-cast-eligible'), 'false',
    'this check uses a College-disabled secondary and cannot cast it');
  const scale = Number(await page.locator('.game-hud').getAttribute('data-hud-input-scale'));
  assert.ok(scale > 0);
  const bounds = await button.boundingBox();
  assert.ok(bounds);
  const center = { x: bounds.x + bounds.width / 2, y: bounds.y + bounds.height / 2 };
  const inputCount = () => page.evaluate(() => window.wasmBindings.gameplay_input_state()[8]);
  const before = await inputCount();
  await page.mouse.move(center.x, center.y);
  await page.mouse.down();
  await page.mouse.move(center.x, center.y - 50 * scale);
  await page.mouse.up();
  assert.equal(await button.count(), 1, 'the exact 50-unit boundary keeps the assignment');
  assert.equal(await inputCount(), before, 'a College-secondary tap does not cast');

  await page.evaluate(() => {
    window.beltPullOffReceipt = [];
    window.beltPullOffObserver = new MutationObserver(records => {
      for (const record of records) {
        for (const node of record.addedNodes) {
          if (!(node instanceof Element) || !node.matches('.hud-belt-burst-anchor')) continue;
          window.beltPullOffReceipt.push({
            smoke: node.querySelectorAll('[data-native-ui-record="UI.65"]').length,
            moveFade: node.querySelectorAll('[data-native-ui-record="UI.69"]').length,
          });
        }
      }
    });
    window.beltPullOffObserver.observe(document.querySelector('.game-hud'), { childList: true });
  });
  try {
    await page.mouse.move(center.x, center.y);
    await page.mouse.down();
    await page.mouse.move(center.x, center.y - 50.25 * scale);
    await page.mouse.up();
    await button.waitFor({ state: 'detached' });
    await page.waitForFunction(() => window.beltPullOffReceipt.length > 0);
    const bursts = await page.evaluate(() => window.beltPullOffReceipt);
    assert.equal(bursts.length, 1, 'one pull-off emits one cosmetic burst');
    assert.equal(bursts[0].smoke, 24);
    assert.ok([3, 4].includes(bursts[0].moveFade));
    await page.locator('.hud-belt-burst-anchor').waitFor({ state: 'detached' });
    assert.equal(await inputCount(), before, 'unbinding never activates the removed spell');
    return { slot, threshold: 50, acceptedBeyond: 50.25, bursts };
  } finally {
    await page.evaluate(() => window.beltPullOffObserver.disconnect());
  }
}

module.exports = { checkNativeBeltPullOff };
