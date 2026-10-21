// Browser smoke for the Rust/WASM client: loads the page in headless Chrome, checks that the
// wizard spawns in the arch, walks on key presses, and that no console errors occur. Writes a
// screenshot. Run with the dev server up (cd web && trunk serve):
//   PW=<path to a playwright-core package, relative to the current directory> node tools/browser-smoke/smoke.cjs
// Optional: URL (default http://127.0.0.1:4180/), SHOT (screenshot path), CHROME (executable).
const { chromium } = require(require('node:path').resolve(process.env.PW));
const url = process.env.URL || 'http://127.0.0.1:4180/';
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const { startNewGame } = require('./menu-journey.cjs');

async function waitForServer() {
  for (let attempt = 0; attempt < 90; attempt += 1) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch {}
    await sleep(1000);
  }
  throw new Error(`server at ${url} did not answer`);
}

(async () => {
  await waitForServer();
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'],
  });
  const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
  // The stock asset fixture has no site icon; keep that browser request out of
  // the game error check without suppressing actual asset or runtime failures.
  await page.route('**/favicon.ico', route => route.fulfill({ status: 204 }));
  const problems = [];
  const warnings = [];
  page.on('console', (message) => {
    if (message.type() === 'error') problems.push(`console.error: ${message.text()}`);
    else if (message.type() === 'warning') warnings.push(message.text());
  });
  page.on('pageerror', (error) => problems.push(`pageerror: ${error.message}`));
  await page.goto(url, { waitUntil: 'load' });
  await page.waitForFunction(
    () => window.wasmBindings && typeof window.wasmBindings.wizard_state === 'function'
      && window.wasmBindings.wizard_state().length > 0
      && document.getElementById('status').textContent === '',
    null,
    { timeout: 60000 },
  );
  const state = () => page.evaluate(() => Array.from(window.wasmBindings.wizard_state()));
  await startNewGame(page);
  await sleep(300);
  const spawned = await state();
  const initialStudents = await page.evaluate(() => Array.from(window.wasmBindings.npc_state()));
  await page.keyboard.down('KeyS');
  await sleep(700);
  await page.keyboard.up('KeyS');
  await sleep(100);
  const afterSouth = await state();
  await page.keyboard.down('KeyD');
  await sleep(500);
  await page.keyboard.up('KeyD');
  await sleep(400);
  const afterEast = await state();
  const laterStudents = await page.evaluate(() => Array.from(window.wasmBindings.npc_state()));
  const initialStudentPositions = new Map();
  for (let index = 1; index < initialStudents.length; index += 3) {
    initialStudentPositions.set(initialStudents[index], initialStudents.slice(index + 1, index + 3));
  }
  let walkingStudentCount = 0;
  for (let index = 1; index < laterStudents.length; index += 3) {
    const initial = initialStudentPositions.get(laterStudents[index]);
    if (initial && Math.hypot(laterStudents[index + 1] - initial[0], laterStudents[index + 2] - initial[1]) > 1) {
      walkingStudentCount += 1;
    }
  }
  await page.screenshot({ path: process.env.SHOT || 'rust-game-smoke.png' });
  const status = await page.evaluate(() => document.getElementById('status').textContent);
  await browser.close();
  const checks = {
    spawnedInArch: Math.abs(spawned[0] - 950.64) < 0.01 && Math.abs(spawned[1] - 164.04) < 0.01,
    walkedSouth: afterSouth[1] - spawned[1] > 30 && afterSouth[2] === 12,
    walkedEast: afterEast[0] - afterSouth[0] > 20 && afterEast[2] === 6,
    ticking: afterEast[5] > afterSouth[5] && afterSouth[5] > spawned[5],
    studentsPresent: initialStudents[0] >= 1 && laterStudents[0] >= 1,
    studentsWalking: walkingStudentCount > 0,
    quietConsole: problems.length === 0,
    emptyStatus: status === '',
  };
  console.log(JSON.stringify({ spawned, afterSouth, afterEast,
    studentCounts: [initialStudents[0], laterStudents[0]], walkingStudentCount,
    problems, warnings, checks }, null, 1));
  process.exit(Object.values(checks).every(Boolean) ? 0 : 1);
})().catch((error) => { console.error('smoke failed:', error.message); process.exit(1); });
