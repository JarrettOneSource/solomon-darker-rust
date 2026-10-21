// Use the actual trusted button clicks: deferring the request to a game tick loses activation.
const assert = require('node:assert/strict');

async function assertFullscreenToggle(page) {
  const enter = page.getByRole('button', {name: 'Enter fullscreen', exact: true});
  await enter.waitFor();
  const bounds = await enter.boundingBox();
  await enter.click();
  await page.waitForFunction(() => document.fullscreenElement === document.documentElement);
  const exit = page.getByRole('button', {name: 'Exit fullscreen', exact: true});
  assert.equal(await exit.getAttribute('aria-pressed'), 'true');
  await exit.click();
  await page.waitForFunction(() => document.fullscreenElement === null);
  await enter.waitFor();
  assert.equal(await enter.getAttribute('aria-pressed'), 'false');
  return {bounds, enteredDocumentRoot: true, exited: true};
}

module.exports = {assertFullscreenToggle};
