// Required real-input smoke for the Rust/WASM game and its GPU UI.
// PW=<playwright-core> URL=http://127.0.0.1:4181/ node tools/browser-smoke/smoke.cjs
const path = require('node:path');
const fs = require('node:fs');
const { chromium } = require(path.resolve(process.env.PW));
const { startNewGame } = require('./menu-journey.cjs');
const ui = require('./gpu-ui.cjs');
const url = process.env.URL || 'http://127.0.0.1:4181/';
const screenshot = process.env.SHOT || path.join(process.env.OUT || '.', 'rust-game-smoke.png');
const receiptPath = path.join(path.dirname(screenshot),'smoke-receipt.json');
fs.mkdirSync(path.dirname(screenshot),{recursive:true});
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

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
  let page;
  let observed;
  let step='startup';
  try {
    page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
    observed = ui.watchErrors(page);
    await ui.observeGpu(page);
    await page.goto(url, {waitUntil:'domcontentloaded'});
    await ui.ready(page);
    await ui.surface(page, 'title');
    const titleGpu = await ui.assertGpuOnly(page);
    const filename = path.parse(screenshot);
    await page.screenshot({path:path.join(filename.dir, `${filename.name}-title${filename.ext || '.png'}`)});
    const state = () => page.evaluate(() => Array.from(window.wasmBindings.wizard_state()));
    step='ordinary College and Create admission';
    await startNewGame(page);
    await ui.widget(page, 'hud:pause');
    await page.waitForTimeout(300);
    const spawned = await state();
    const initialStudents = await page.evaluate(() => Array.from(window.wasmBindings.npc_state()));
    step='actual College movement and ambient students';
    await page.keyboard.down('KeyS');
    await page.waitForTimeout(700);
    await page.keyboard.up('KeyS');
    await page.waitForTimeout(100);
    const afterSouth = await state();
    await page.keyboard.down('KeyD');
    await page.waitForTimeout(500);
    await page.keyboard.up('KeyD');
    await page.waitForTimeout(400);
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
    const gameplayGpu = await ui.assertGpuOnly(page);
    const submissions = await ui.gpuDraws(page);
    await page.screenshot({path:screenshot});
    const checks = {
      spawnedInArch: Math.abs(spawned[0] - 950.64) < 0.01 && Math.abs(spawned[1] - 164.04) < 0.01,
      walkedSouth: afterSouth[1] - spawned[1] > 30 && afterSouth[2] === 12,
      walkedEast: afterEast[0] - afterSouth[0] > 20 && afterEast[2] === 6,
      ticking: afterEast[5] > afterSouth[5] && afterSouth[5] > spawned[5],
      studentsPresent: initialStudents[0] >= 1 && laterStudents[0] >= 1,
      studentsWalking: walkingStudentCount > 0,
      titleOnGpu: titleGpu.gpu.draw.triangles > 0,
      hudOnGpu: gameplayGpu.gpu.draw.triangles > 0,
      actualCanvasSubmissions: submissions.canvasCalls.game > 0 && submissions.vertices > 0,
      noRuntimeOrAssetErrors: observed.errors.length === 0,
      ready: gameplayGpu.gpu.ready && gameplayGpu.gpu.error === null,
    };
    const receipt={passed:Object.values(checks).every(Boolean),url,spawned,afterSouth,afterEast,
      studentCounts:[initialStudents[0],laterStudents[0]],walkingStudentCount,
      titleGpu:titleGpu.gpu,gameplayGpu:gameplayGpu.gpu,submissions,
      problems:observed.errors,warnings:observed.warnings,checks};
    fs.writeFileSync(receiptPath,JSON.stringify(receipt,null,2));
    console.log(JSON.stringify(receipt,null,1));
    if (!receipt.passed) process.exitCode = 1;
  } catch(error) {
    const receipt={passed:false,url,step,failure:error.stack,problems:observed?.errors,
      warnings:observed?.warnings};
    if(page&&!page.isClosed()) {
      try {receipt.gpu=await ui.readUi(page)}
      catch(observationError) {receipt.observationError=observationError.stack}
      try {await page.screenshot({path:path.join(path.dirname(screenshot),'smoke-failure.png')})}
      catch(screenshotError) {receipt.screenshotError=screenshotError.stack}
    }
    fs.writeFileSync(receiptPath,JSON.stringify(receipt,null,2));
    throw error;
  } finally {await browser.close()}
})().catch(error => {console.error('smoke failed:',error);process.exitCode=1});
