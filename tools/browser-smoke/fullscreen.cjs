// Real trusted GPU pointer input and native browser fullscreen state.
const assert = require('node:assert/strict');
const ui = require('./gpu-ui.cjs');

async function assertFullscreenToggle(page) {
  const enter = await ui.widget(page, 'fullscreen/toggle');
  assert.equal(enter.label, 'Enter fullscreen');
  const bounds = ui.bounds(enter);
  await ui.click(page, 'fullscreen/toggle');
  await page.waitForFunction(() => document.fullscreenElement === document.documentElement);
  await ui.value(page, 'fullscreen/toggle', 'true');
  assert.equal((await ui.widget(page, 'fullscreen/toggle')).label, 'Exit fullscreen');
  await ui.click(page, 'fullscreen/toggle');
  await page.waitForFunction(() => document.fullscreenElement === null);
  await ui.value(page, 'fullscreen/toggle', 'false');
  assert.equal((await ui.widget(page, 'fullscreen/toggle')).label, 'Enter fullscreen');
  return {bounds, enteredDocumentRoot:true, exited:true};
}

module.exports = {assertFullscreenToggle};
