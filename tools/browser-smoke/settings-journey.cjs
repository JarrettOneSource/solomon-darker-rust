const ui = require('./gpu-ui.cjs');

// GPU controls enter the diagnostic list only when their real scroll owner
// paints them. Wheel that owner as a person would reveal a lower Settings row.
async function revealSettingsControl(page, id) {
  for (let attempt = 0; attempt < 14; attempt++) {
    const state = await ui.readUi(page);
    const control = ui.findWidget(state, id), content = ui.findWidget(state, 'settings-content');
    if (control && content) {
      const row = ui.bounds(control, {paint:true}), viewport = ui.bounds(content);
      if (row.y >= viewport.y && row.y + row.height <= viewport.y + viewport.height) return control;
    }
    await ui.wheel(page, 'settings-content', 0, 330);
    await ui.nextFrame(page);
  }
  throw new Error(`Settings control did not scroll into view: ${id}`);
}

module.exports = {revealSettingsControl};
