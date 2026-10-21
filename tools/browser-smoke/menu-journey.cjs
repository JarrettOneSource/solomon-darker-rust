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
    await page.locator('.game-menus[data-menu-screen="create"]').waitFor({state:'attached',timeout:30000});
  } catch (error) {
    const actual = await page.evaluate(() => ({
      wizard: Array.from(window.wasmBindings.wizard_state()),
      input: Array.from(window.wasmBindings.gameplay_input_state()),
      panel: document.querySelector('.player-panels')?.outerHTML.slice(0, 300),
      menu: document.querySelector('.game-menus')?.dataset.menuScreen,
      status: document.querySelector('#status')?.textContent,
    }));
    throw new Error(`College exit did not reach loadout: ${JSON.stringify(actual)}`, {cause:error});
  } finally {
    await movement(0, 0);
  }
}

// Real College admission precedes the one final character-selection screen.
async function reachCollegeLoadout(page) {
  const dialogue = page.locator('.player-panels:not([hidden]) [data-player-panel="dialogue"]');
  await dialogue.waitFor({timeout:120000});
  await page.locator('[data-panel-action="dialogue-next"]').click();
  await page.locator('[data-panel-action="dialogue-choice:0"]').click();
  await dialogue.locator('[data-dialogue-key="ARCH_Q1_0"]').waitFor();
  await page.keyboard.press('Escape');
  await page.locator('.player-panels').waitFor({state:'hidden'});
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
  element = 'ether', discipline = 'arcane', name = 'Helvidius', introOnly = false,
} = {}) {
  const title = page.locator('.game-menus[data-menu-screen="title"]');
  if (await title.count() === 0) return;
  const prompt = page.locator('.title-prompt:not([hidden])');
  if (await prompt.isVisible()) {
    await prompt.locator('[data-menu-action="prompt-secondary"]').click();
    await prompt.waitFor({state: 'hidden'});
  }
  await page.locator('[data-menu-action="play"]').click();
  await page.locator('[data-menu-action="new-game"]').click();
  // Menu clicks are applied on the next frame. Wait for the actual next surface
  // before deciding whether an existing character needs confirmation.
  await page.waitForFunction(() => {
    const screen = document.querySelector('.game-menus').dataset.menuScreen;
    return document.querySelector('.title-prompt:not([hidden])')
      || screen === 'create'
      || document.querySelector('.college-intro-overlay:not([hidden])')
      || document.querySelector('.player-panels:not([hidden]) [data-player-panel="dialogue"]');
  }, null, { timeout: 20000 });
  if (await prompt.isVisible()) {
    await prompt.locator('[data-menu-action="prompt-primary"]').click();
  }
  // A saved wizard's old tick remains available while retirement is pending.
  // It does not identify whether the next flow is Create or College admission.
  await page.waitForFunction(() => document.querySelector('.game-menus').dataset.menuScreen === 'create'
    || document.querySelector('.college-intro-overlay:not([hidden])')
    || document.querySelector('.player-panels:not([hidden]) [data-player-panel="dialogue"]'),
  null, { timeout: 120000 });
  if (introOnly) return;
  // Declining Source's first-run Tutorial also skips the College introduction.
  if (await page.locator('.game-menus').getAttribute('data-menu-screen') !== 'create') {
    await reachCollegeLoadout(page);
  }
  const nameInput = page.getByRole('textbox', { name: 'Wizard name', exact: true });
  await nameInput.fill(name);
  const acceptedName = await nameInput.inputValue();
  if (acceptedName !== name) {
    throw new Error(`Create rejected the requested wizard name ${JSON.stringify(name)}; retained ${JSON.stringify(acceptedName)}`);
  }
  await page.locator(`[data-menu-action="element:${element}"]`).click();
  await page.locator(`[data-menu-action="discipline:${discipline}"]`).click();
  await page.waitForFunction(() => document.querySelector('.game-menus').dataset.menuScreen === 'hidden'
    && window.wasmBindings.wizard_state()[6] === 0
    && window.wasmBindings.wizard_state()[7] === -1, null, { timeout: 20000 });
}

module.exports = { leaveCollegeOffice, reachCollegeLoadout, startNewGame };
