const ui = require('./gpu-ui.cjs');

// The stock Office desk occupies x402.5..638.5, y644.5..741.5. Walk around its
// left edge before approaching the south doorway; straight down hits its top.
async function leaveCollegeOffice(page, movement) {
  try {
    for (const [x, y, axis, threshold, increasing] of [
      [-1, 0, 0, 350, false],
      [0, 1, 1, 800, true],
      [1, 0, 0, 512, true],
    ]) {
      await movement(x, y);
      await page.waitForFunction(({axis, threshold, increasing}) => {
        const coordinate = window.wasmBindings.wizard_state()[axis];
        return increasing ? coordinate >= threshold : coordinate <= threshold;
      }, {axis, threshold, increasing}, {timeout:30000});
    }
    await movement(0, 1);
    await ui.surface(page, 'create', {timeout:30000});
  } catch (error) {
    const actual = await page.evaluate(() => ({
      wizard: Array.from(window.wasmBindings.wizard_state()),
      input: Array.from(window.wasmBindings.gameplay_input_state()),
      gpu: JSON.parse(window.wasmBindings.gpu_ui_state()),
    }));
    throw new Error(`College exit did not reach loadout: ${JSON.stringify(actual)}`, {cause:error});
  } finally {
    await movement(0, 0);
  }
}

// Real College admission precedes the one final character-selection screen.
async function reachCollegeLoadout(page, {touch = false} = {}) {
  const activate = touch ? ui.tap : ui.click;
  await ui.widget(page, 'panel:dialogue', {timeout:120000});
  await activate(page, 'panel:dialogue-next');
  await activate(page, 'panel:dialogue-choice:0');
  await ui.value(page, 'panel:dialogue-key', 'ARCH_Q1_0');
  if (touch) {
    await activate(page, 'panel:dialogue-next');
    await ui.widget(page, 'panel:dialogue-choice:0');
    if((await ui.widget(page,'panel:dialogue-next')).label!=='Done')
      throw new Error('the actual question list must expose its Done action');
    await activate(page, 'panel:dialogue-next');
  } else await page.keyboard.press('Escape');
  await ui.noWidget(page, 'panel:dialogue');
  if (touch) {
    const joystick = ui.bounds(await ui.widget(page, 'hud:joystick:movement'));
    const center = ui.center(joystick);
    const input = await page.context().newCDPSession(page);
    await input.send('Input.dispatchTouchEvent', {type:'touchStart',touchPoints:[{...center,id:1}]});
    try {
      await leaveCollegeOffice(page, (dx,dy) => input.send('Input.dispatchTouchEvent', {
        type:'touchMove',touchPoints:[{x:center.x+dx*joystick.width*0.4,
          y:center.y+dy*joystick.height*0.4,id:1}],
      }));
    } finally {
      await input.send('Input.dispatchTouchEvent', {type:'touchEnd',touchPoints:[]});
      await input.detach();
    }
    return;
  }
  const held = new Set();
  await leaveCollegeOffice(page, async (x, y) => {
    const desired = x < 0 ? 'KeyA' : x > 0 ? 'KeyD' : y > 0 ? 'KeyS' : null;
    for (const key of held) {
      if (key !== desired) { await page.keyboard.up(key); held.delete(key); }
    }
    if (desired && !held.has(desired)) {
      await page.keyboard.down(desired);
      held.add(desired);
    }
  });
}

async function startNewGame(page, {
  element = 'ether', discipline = 'arcane', name = 'Helvidius', introOnly = false, touch = false,
} = {}) {
  const activate = touch ? ui.tap : ui.click;
  const initial = await ui.readUi(page);
  if (initial.surface !== 'title') return;
  if (ui.findWidget(initial, 'prompt-secondary')) {
    await activate(page, 'prompt-secondary');
    await ui.noWidget(page, 'prompt-secondary');
  }
  await activate(page, 'play');
  await activate(page, 'new-game');
  // A saved wizard's old tick remains available during retirement. Observe the
  // actual GPU surface rather than treating that old state as a fresh admission.
  const next = await ui.waitForUi(page, snapshot => snapshot.surface === 'create'
    || ui.findWidget(snapshot, 'prompt-primary')
    || ui.findWidget(snapshot, 'college-intro')
    || ui.findWidget(snapshot, 'panel:dialogue'), 'new-game admission', {timeout:120000});
  if (ui.findWidget(next, 'prompt-primary')) await activate(page, 'prompt-primary');
  await ui.waitForUi(page, snapshot => snapshot.surface === 'create'
    || ui.findWidget(snapshot, 'college-intro')
    || ui.findWidget(snapshot, 'panel:dialogue'), 'Create or College admission', {timeout:120000});
  if (introOnly) return;
  if ((await ui.readUi(page)).surface !== 'create') await reachCollegeLoadout(page, {touch});
  // A real player may keep Create's generated name. Name-edit acceptance keeps
  // its explicit draft; unrelated family routes can choose this ordinary path.
  if(name!==null) {
    await ui.fill(page, 'create-name', name, {touch});
    await ui.value(page, 'create-name', name);
  }
  const draft=(await ui.widget(page,'create-name')).value;
  if(typeof draft!=='string')throw new Error('Create must expose its actual accepted name draft');
  const expectedName=draft||'Genericus';
  await activate(page, `element:${element}`);
  await activate(page, `discipline:${discipline}`);
  await ui.surface(page, 'hidden');
  await page.waitForFunction(() => window.wasmBindings.wizard_state()[6] === 0
    && window.wasmBindings.wizard_state()[7] === -1, null, {timeout:20000});
  const accepted=await ui.waitForUi(page,snapshot=>{
    const selection=snapshot.acceptedSelection;
    return selection?.name===expectedName&&selection.element.toLowerCase()===element
      &&selection.discipline.toLowerCase()===discipline;
  },`actual admitted ${expectedName}/${element}/${discipline}`);
  return accepted.acceptedSelection;
}

module.exports = { leaveCollegeOffice, reachCollegeLoadout, startNewGame };
