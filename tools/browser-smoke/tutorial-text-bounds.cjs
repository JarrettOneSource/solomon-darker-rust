// Actual rendered text bounds: wrapping and the phone font floor cannot be checked in sim.
const assert = require('node:assert/strict');

async function assertTutorialTextBounds(page) {
  const bounds = await page.evaluate(() => {
    const rectangle = name => {
      const node = document.querySelector(`.tutorial-ui:not([hidden]) [data-tutorial-text="${name}"]`);
      if (!node) return null;
      const box = node.getBoundingClientRect();
      return {left: box.left, right: box.right, top: box.top, bottom: box.bottom};
    };
    const selected = [...document.querySelectorAll('.game-hud:not([hidden]) .hud-skill-action')]
      .map(node => node.getBoundingClientRect()).filter(box => box.width > 0 && box.height > 0)
      .map(box => ({left: box.left, right: box.right, top: box.top, bottom: box.bottom, width: box.width, height: box.height}));
    const controls = [...document.querySelectorAll('.game-hud:not([hidden]) :is(.hud-belt-slot,.hud-backpack,.hud-tome),.movement-joystick:not([data-blocked])')]
      .map(node => node.getBoundingClientRect())
      .filter(box => box.width > 0 && box.height > 0 && box.top >= innerHeight / 2)
      .map(box => ({left: box.left, right: box.right, top: box.top, bottom: box.bottom}));
    return {controls,width: innerWidth, height: innerHeight, coarse: matchMedia('(pointer:coarse)').matches,
      heading: rectangle('heading'), subheading: rectangle('subheading'), selected};
  });
  assert.ok(bounds.heading && bounds.subheading, 'the current lesson has both instruction lines');
  for (const name of ['heading', 'subheading']) {
    const box = bounds[name];
    assert.ok(box.top >= 0 && box.bottom <= bounds.height, `${name} stays vertically in the viewport`);
    assert.ok(box.left >= 0 && box.right <= bounds.width, `${name} stays horizontally in the viewport`);
  }
  assert.ok(bounds.subheading.top >= bounds.heading.bottom + 4, 'the subheading clears every wrapped heading line');
  if (bounds.coarse && bounds.width < 1000) {
    for (const name of ['heading', 'subheading']) {
      for (const control of bounds.controls) {
        const text = bounds[name];
        const clear = text.bottom <= control.top || text.top >= control.bottom
          || text.right <= control.left || text.left >= control.right;
        assert.ok(clear, `${name} clears the actual lower controls`);
      }
    }
    for (const button of bounds.selected) {
      assert.ok(button.width >= 43.99 && button.height >= 43.99, 'selectors retain a real finger-sized target');
      assert.ok(button.top >= 0 && button.left >= 0 && button.right <= bounds.width, 'selector targets stay in the viewport');
      assert.ok(bounds.heading.top >= button.bottom + 7, 'the lesson clears the visible selector strip');
    }
  }
  return bounds;
}

module.exports = {assertTutorialTextBounds};
