// Actual local Website package -> Cloud -> Rust hosted authority -> save continuation.
// No socket sends, state setters, synthetic save or mocked content are used. The
// hooks below observe accepted messages and real WebAudio submissions only.
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const {reachCollegeLoadout} = require('./menu-journey.cjs');
const {createNativeMessageReader} = require('./native-wire.cjs');
const ui = require('./gpu-ui.cjs');
const output = path.resolve(process.env.OUT);
const contentDirectory = path.resolve(process.env.CONTENT_DIR);
const account = JSON.parse(fs.readFileSync(path.join(contentDirectory, 'private-account.json')));
const acceptedPackage = JSON.parse(fs.readFileSync(path.join(contentDirectory, 'package.json')));
const url = new URL(process.env.URL || 'https://127.0.0.1:9449/game-stg/');
assert.equal(url.hostname, '127.0.0.1', 'this journey uses only the isolated staging backend');
const includeConsole = process.env.CONSOLE === '1';
const witnessConsole = process.env.CONSOLE === 'witness';
const observeConsole = process.env.CONSOLE === 'observe' || witnessConsole;
const continueAuthored = process.env.CONTINUE_AUTHORED === '1';
const authoredPhase = process.env.AUTHORED_PHASE || 'complete';
assert.ok(['complete', 'reforge', 'finalize', 'console'].includes(authoredPhase), 'known authored journey phase');
const state = {id: null, frame: null, runtime: null, content: null, welcomes: [],
  lua: [], luaRequests: [], actions: [], checkpoints: 0, leaveReceipts: 0, lastSave: null,
  audioEvents: [], assets: [], loaded: null, iconProof: [], wireTypes: {}, beltBindings: [],
  cheatCommands: [], cheatModes: [], helloCheats: []};
const errors = [], requests = [], expectedNotFound = [], cancelledMedia = [], checks = [];
let step = 'bootstrap';
let ownedBrowserClosed = false;
const sha256 = value => crypto.createHash('sha256').update(value).digest('hex');
let priorPhase = null;
let priorSackPhase = null;
let priorOnwardPhase = null;
let priorSavePhase = null;
if (authoredPhase === 'reforge') {
  assert.ok(continueAuthored, 'later authored phases continue the real retained profile');
  assert.ok(process.env.PRIOR_AUTHORED_RECEIPT, 'later phases name their accepted earlier receipt');
  const file = path.resolve(process.env.PRIOR_AUTHORED_RECEIPT);
  const bytes = fs.readFileSync(file);
  const receipt = JSON.parse(bytes);
  const check = 'Source local drag admission restores the custom potion without a bind command or inventory change';
  assert.deepEqual(receipt.errors, [], 'the earlier matched-bundle phase has no runtime errors');
  assert.ok(receipt.checks.includes(check), 'the earlier phase proved the actual rejected drag');
  assert.equal(receipt.accepted.tonic, 1, 'the earlier phase kept the real purchased Tonic');
  priorPhase = {file, sha256: sha256(bytes), checks: receipt.checks, accepted: receipt.accepted};
}
if (process.env.PRIOR_SACK_RECEIPT) {
  const file = path.resolve(process.env.PRIOR_SACK_RECEIPT);
  const bytes = fs.readFileSync(file);
  const receipt = JSON.parse(bytes);
  assert.deepEqual(receipt.errors, [], 'the earlier Sack phase has no runtime errors');
  assert.equal(receipt.robeDragProof.ready.page, 'current');
  assert.equal(receipt.robeDragProof.ready.disabled, false);
  assert.ok(receipt.robeDragProof.pointers.some(pointer => pointer.type === 'pointerup'
    && pointer.hit === 'back' && pointer.draggerChildren > 0));
  priorSackPhase = {file, sha256: sha256(bytes), robeDragProof: receipt.robeDragProof};
}
if (authoredPhase === 'finalize' || authoredPhase === 'console') {
  assert.ok(continueAuthored, 'final acceptance continues the real retained run');
  assert.ok(process.env.PRIOR_ONWARD_RECEIPT, 'final acceptance names its earlier actual phases');
  const file = path.resolve(process.env.PRIOR_ONWARD_RECEIPT);
  const bytes = fs.readFileSync(file);
  const receipt = JSON.parse(bytes);
  assert.deepEqual(receipt.errors, [], 'the earlier authored phases have no runtime errors');
  for (const check of [
    'real retained purchase outputs, custom icons, semantic state and new robe affix',
    'actual minimap ping crosses the host rule boundary and submits authored audio',
    'actual custom potion Inventory use commits inventory and source on-use effects',
  ]) assert.ok(receipt.checks.includes(check), `the earlier phase proved ${check}`);
  assert.ok(receipt.accepted.runId, 'the earlier phase entered a real Boneyard');
  assert.equal(receipt.accepted.tonic, 0, 'the actual potion was consumed');
  priorOnwardPhase = {file, sha256: sha256(bytes), checks: receipt.checks,
    accepted: receipt.accepted, priorPhase: receipt.priorPhase, priorSackPhase: receipt.priorSackPhase};
}
if (authoredPhase === 'console') {
  assert.ok(includeConsole, 'the Console-only phase executes the actual Console');
  assert.ok(process.env.PRIOR_SAVE_RECEIPT, 'the Console-only phase names its accepted earlier save');
  const file = path.resolve(process.env.PRIOR_SAVE_RECEIPT);
  const bytes = fs.readFileSync(file);
  const receipt = JSON.parse(bytes);
  assert.equal(receipt.passed, true);
  assert.deepEqual(receipt.errors, []);
  assert.equal(receipt.continued.runId, priorOnwardPhase.accepted.runId);
  priorSavePhase = {file, sha256: sha256(bytes), checks: receipt.checks, continued: receipt.continued};
}

const pause = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
async function bounded(promise, label, milliseconds) {
  let timeout;
  try {
    return await Promise.race([promise, new Promise((resolve, reject) => {
      timeout = setTimeout(() => reject(new Error(`${label} timed out`)), milliseconds);
    })]);
  } finally {
    clearTimeout(timeout);
  }
}
async function until(predicate, label, timeout = 30000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (await predicate()) return;
    await pause(40);
  }
  throw new Error(`Timed out waiting for ${label}`);
}
function mark(label) {step = label; console.log(label);}
function check(label) {checks.push(label); console.log(label);}
function player() {return state.frame?.snapshot.participants[state.id]?.player;}
function position() {return state.frame?.snapshot.participants[state.id]?.wizard.position;}
function items(name) {return player()?.economy.backpack.filter(item => item.name === name) || [];}
function itemQuantity(name) {return items(name).reduce((total, item) => total + item.quantity, 0);}
function firstPurchase() {return state.runtime?.state_values?.some(row =>
  row.key === 'tutorial.first_purchase' && row.value === true && row.scope_id === state.id);}
const snapshot = () => ({tick: state.frame?.snapshot.ticks, runId: state.frame?.snapshot.boneyard?.run_id,
  gold: player()?.economy.gold, moondust: itemQuantity('Moondust'), tonic: itemQuantity('Ward Tonic'),
  firstPurchase: Boolean(firstPurchase()), position: position(), runtimeRevision: state.runtimeRevision,
  host: state.frame?.host});

async function gameplayReady() {
  await until(() => state.frame && !state.frame.host.pause && !state.frame.host.resume_grace
    && !state.frame.host.loading_run_id, 'actual resumed authority and completed resume grace');
}

async function moveTo(page, x, y, admitted) {
  // Courtyard waypoints keep the public input journey outside the desk/fountain.
  const held = new Set();
  const started = Date.now();
  try {
    while (Math.hypot(position().x - x, position().y - y) > 9) {
      if (admitted && await admitted()) return;
      if (Date.now() - started >= 25000) {
        // Preserve admission and velocity while the failing real keys are still
        // held. The outer failure handler runs after finally releases them.
        state.movementFailure = {
          target: {x, y},
          heldKeys: [...held],
          accepted: snapshot(),
          wizard: state.frame.snapshot.participants[state.id].wizard,
          host: {
            pause: state.frame.host.pause,
            loadingRunId: state.frame.host.loading_run_id,
            resumeGrace: state.frame.host.resume_grace,
          },
          browser: await page.evaluate(() => ({
            wizard: Array.from(window.wasmBindings.wizard_state()),
            input: Array.from(window.wasmBindings.gameplay_input_state()),
            focusedElement: {
              tag: document.activeElement?.tagName,
              id: document.activeElement?.id,
              type: document.activeElement?.getAttribute('type'),
            },
            gpu: JSON.parse(window.wasmBindings.gpu_ui_state()),
          })),
        };
        assert.fail(`movement blocked approaching ${x},${y}`);
      }
      const point = position();
      const desired = new Set();
      if (Math.abs(x - point.x) > 6) desired.add(x > point.x ? 'KeyD' : 'KeyA');
      if (Math.abs(y - point.y) > 6) desired.add(y > point.y ? 'KeyS' : 'KeyW');
      for (const key of held) if (!desired.has(key)) {await page.keyboard.up(key); held.delete(key);}
      for (const key of desired) if (!held.has(key)) {await page.keyboard.down(key); held.add(key);}
      await pause(40);
    }
  } finally {for (const key of held) await page.keyboard.up(key);}
}
async function hiddenMenu(page) {
  await ui.surface(page, 'hidden');
}
async function resumeFromPause(page) {
  await ui.click(page, 'resume'); await hiddenMenu(page);
}
async function actionByLabel(page, label, options = {}) {
  return ui.byLabel(page, label, {role:'button', ...options});
}
async function clickLabel(page, label, options) {
  await ui.click(page, (await actionByLabel(page,label,options)).id);
}
async function approachApothecary(page) {
  const admitted = async () => (await ui.readUi(page)).widgets.some(widget =>
    widget.id.startsWith('mod/panel:shop:') && /Apprentice Apothecary · 8 gold/.test(widget.label));
  if (await admitted()) return;
  await moveTo(page,1510,740,admitted); if (await admitted()) return;
  await moveTo(page,1450,740,admitted);
  await actionByLabel(page,/Apprentice Apothecary · 8 gold/,{prefix:'mod/panel:'});
}
async function openInventory(page) {
  if (!ui.findWidget(await ui.readUi(page),'panel:surface:inventory')) await ui.click(page,'hud:inventory');
  await ui.widget(page,'panel:surface:inventory');
  await ui.waitForUi(page,snapshot => snapshot.widgets.some(widget => widget.id.startsWith('panel:item:') && widget.enabled)
    && !ui.findWidget(snapshot,'panel:inventory-flyby'), 'settled enabled Inventory cells');
}
async function assertAuthoredIcon(page, itemId) {
  const widget = await ui.widget(page,`panel:mod-icon:${itemId}`);
  const proof = {bounds:ui.bounds(widget),frame:JSON.parse(widget.value)};
  assert.ok(proof.bounds.width>0 && proof.bounds.height>0,'the actual authored texture crop occupies visible GPU pixels');
  assert.match(proof.frame.url,/^blob:/);
  state.iconProof.push(proof);
}
async function observePanelInput(page) {
  await page.evaluate(() => {
    const observation={pointers:[],flybys:[]};let running=true;
    const read=()=>JSON.parse(window.wasmBindings.gpu_ui_state());
    const observe=event=>{
      const snapshot=read();const [x,y]=[event.clientX,event.clientY];
      const hit=[...snapshot.widgets].reverse().find(widget=>widget.role==='button' && widget.rect[0]<=x && widget.rect[1]<=y && x<widget.rect[0]+widget.rect[2] && y<widget.rect[1]+widget.rect[3]);
      observation.pointers.push({type:event.type,trusted:event.isTrusted,client:{x,y},target:event.target.tagName,
        hit:hit?.id.replace(/^panel:/,''),captured:snapshot.capture.pointer,
        draggerChildren:snapshot.widgets.some(widget=>widget.id==='panel:dragger')?1:0});
    };
    const sample=()=>{if(!running)return;const flyby=read().widgets.find(widget=>widget.id==='panel:inventory-flyby');if(flyby){const value=JSON.parse(flyby.value);observation.flybys.push({...value,bounds:flyby.rect});}requestAnimationFrame(sample);};
    for(const type of ['pointerdown','pointermove','pointerup','pointercancel'])document.addEventListener(type,observe,true);
    requestAnimationFrame(sample);
    window.stopAuthoredPanelObservation=()=>{running=false;for(const type of ['pointerdown','pointermove','pointerup','pointercancel'])document.removeEventListener(type,observe,true);return observation;};
  });
}
async function assertRejectedCustomPotionBinding(page, tonicId) {
  const previousBelt=JSON.stringify(player().belt.entries),previousTonic=JSON.stringify(items('Ward Tonic'));
  const tick=state.frame.snapshot.ticks,beforeBindings=state.beltBindings.length;
  const source=ui.bounds(await ui.widget(page,`panel:item:${tonicId}`));
  const destination=ui.bounds(await ui.widget(page,'panel:belt:7'));
  await observePanelInput(page);
  const start=ui.center(source),end=ui.center(destination);
  await page.mouse.move(start.x,start.y);await page.mouse.down();
  await page.mouse.move(end.x,end.y,{steps:15});await ui.widget(page,'panel:dragger');await page.mouse.up();
  await until(()=>state.frame.snapshot.ticks>tick+30,'later authoritative ticks after local binding rejection');
  const observation=await page.evaluate(()=>window.stopAuthoredPanelObservation());
  state.rejectedBeltProof={source,destination,...observation,beforeTick:tick,afterTick:state.frame.snapshot.ticks,bindingCommands:state.beltBindings.length-beforeBindings};
  assert.ok(observation.flybys.some(frame=>frame.phase==='flying'),'the actual item restoration drew its GPU flight');
  await ui.widget(page,`panel:item:${tonicId}`);
  assert.equal(state.beltBindings.length,beforeBindings,'Source rejects the custom item before a BindBelt command');
  assert.equal(JSON.stringify(player().belt.entries),previousBelt);
  assert.equal(JSON.stringify(items('Ward Tonic')),previousTonic);assert.equal(itemQuantity('Ward Tonic'),1);
  check('Source local drag admission restores the custom potion without a bind command or inventory change');
}
async function runLua(page, code) {
  const before = state.luaRequests.length;
  await ui.fill(page,'cheats/lua-editor',code);
  await ui.click(page,'cheats/run');
  await until(() => state.luaRequests.slice(before).some(request => request.code === code),
    'actual Console request');
  const request = state.luaRequests.slice(before).find(request => request.code === code);
  await until(() => state.lua.some(response => response.request_id === request.requestId),
    'matching authoritative Lua response');
  const response = state.lua.find(response => response.request_id === request.requestId);
  await ui.widget(page,`cheats/console-result:${response.request_id}`);
  return response;
}

(async () => {
  fs.mkdirSync(output, {recursive: true});
  const server = await chromium.launchServer({headless: true, host: '127.0.0.1',
    executablePath: process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args: ['--use-angle=metal', '--ignore-gpu-blocklist']});
  const browser = await chromium.connect(server.wsEndpoint());
  const context = await browser.newContext({ignoreHTTPSErrors: true, viewport: {width: 1280, height: 720}});
  await context.addInitScript(token => {
    localStorage.setItem('sdr.token:/game-stg', token);
    window.authoredAudioStarts = [];
    const decodedAssets = new WeakMap();
    const decode = AudioContext.prototype.decodeAudioData;
    AudioContext.prototype.decodeAudioData = function (...args) {
      // Observe encoded identity without changing the returned decode promise or
      // its callbacks. Native decode may detach the input, so copy it first.
      const header = new Uint8Array(args[0], 0, Math.min(4, args[0].byteLength));
      const ogg = header[0] === 79 && header[1] === 103 && header[2] === 103 && header[3] === 83;
      const identity = {sha256: null};
      if (ogg) {
        const encoded = args[0].slice(0);
        crypto.subtle.digest('SHA-256', encoded).then(bytes => {
          identity.sha256 = [...new Uint8Array(bytes)].map(value => value.toString(16).padStart(2, '0')).join('');
        });
      }
      const result = decode.apply(this, args);
      result.then(buffer => decodedAssets.set(buffer, identity), () => {});
      return result;
    };
    const start = AudioBufferSourceNode.prototype.start;
    AudioBufferSourceNode.prototype.start = function (...args) {
      window.authoredAudioStarts.push({duration: this.buffer?.duration, loop: this.loop,
        identity: decodedAssets.get(this.buffer)});
      return start.apply(this, args);
    };
  }, account.token);
  const page = await context.newPage();
  await ui.watchRequestFailures(page, {errors}).ready;
  page.setDefaultTimeout(30000);
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => {
    if (message.type() !== 'error') return;
    const location = message.location().url;
    if (message.text().includes('404') && (/\/api\/game\/saves\/0$/.test(location) || location.endsWith('/api/game/rejoin'))) return;
    errors.push(message.text());
  });
  page.on('response', response => {
    const pathname = new URL(response.url()).pathname;
    if (pathname.includes('/api/')) requests.push({path: pathname, status: response.status(), method: response.request().method()});
    if (response.status() === 404 && (/\/api\/game\/saves\/0$/.test(pathname) || pathname.endsWith('/api/game/rejoin'))) expectedNotFound.push(pathname);
    else if (response.status() >= 400) errors.push(`${response.status()} ${pathname}`);
    if (/\/api\/game\/content\//.test(pathname) && response.ok()) state.assets.push(pathname.split('/').at(-1));
  });
  page.on('websocket', socket => {
    const readMessage = createNativeMessageReader();
    socket.on('framesent', ({payload}) => {
      const message = JSON.parse(payload.toString());
      if (message.type === 'hosted' && message.command.kind === 'lua_execute') {
        state.luaRequests.push({requestId: message.command.request_id, code: message.command.code});
      }
      if (message.type === 'hosted' && message.command.kind === 'mod_action') {
        const {action, target, arguments: args} = message.command;
        state.actions.push({action, target, arguments: args});
      }
      if (message.type === 'hosted' && message.command.kind === 'hello') {
        state.helloCheats.push({enabled: message.command.hello.cheats_enabled});
      }
      if (message.type === 'hosted' && message.command.kind === 'set_cheats') {
        state.cheatCommands.push({enabled: message.command.enabled, tick: state.frame?.snapshot.ticks});
      }
      if (message.type === 'command' && message.command.kind === 'bind_belt') {
        state.beltBindings.push(message.command);
      }
    });
    socket.on('framereceived', ({payload}) => {
      const raw = JSON.parse(payload.toString());
      state.wireTypes[raw.type] = (state.wireTypes[raw.type] || 0) + 1;
      const message = readMessage(raw);
      if (message === null) return;
      if (message.type === 'welcome') {
        state.id = message.credentials.player_id;
        state.frame = message.frame;
        state.welcomes.push(message.hosted);
      } else if (message.type === 'snapshot') state.frame = message.frame;
      else if (message.type === 'social') state.social = message.snapshot;
      else if (message.type === 'boneyard_loaded') state.loaded = message.boneyard;
      else if (message.type === 'error') errors.push(`host ${message.code}: ${message.reason}`);
      else if (message.type === 'hosted') {
        const event = message.message;
        if (event.kind === 'mod_runtime') {
          state.runtime = event.projection;
          state.runtimeRevision = event.revision;
          for (const row of event.projection.presentation_events || []) {
            if (!state.audioEvents.some(event => event.owner === row.owner)) state.audioEvents.push(row);
          }
        } else if (event.kind === 'mod_content') state.content = event.projection;
        else if (event.kind === 'cheat_mode') state.cheatModes.push({enabled: event.enabled,
          tick: state.frame?.snapshot.ticks});
        else if (event.kind === 'lua_result') state.lua.push(event);
        else if (event.kind === 'save_checkpoint') {state.checkpoints++; state.lastSave = event.save;}
        else if (event.kind === 'save_before_leave') state.leaveReceipts++;
      }
    });
  });
  try {
    await page.goto(url.href, {waitUntil: 'load'});
    await ui.ready(page);
    const consentVisible=async()=> (await ui.readUi(page)).surface==='routing';
    if(ui.findWidget(await ui.readUi(page),'prompt-secondary')) await ui.click(page,'prompt-secondary');
    if (!priorOnwardPhase) {
      mark('enable the real uploaded package through Cloud');
      await ui.click(page,'explore');
      const subscribe=`cloud-subscription:subscribe:${acceptedPackage.slug}`;
      const enable=`cloud-subscription:enable:${acceptedPackage.slug}`;
      const disable=`cloud-subscription:disable:${acceptedPackage.slug}`;
      await ui.widget(page,`cloud-open-mod:${acceptedPackage.slug}`);
      let activatedHere=false;
      if(ui.findWidget(await ui.readUi(page),subscribe)){await ui.click(page,subscribe);activatedHere=true;}
      await ui.click(page,'cloud-tab:subscribed');
      await ui.waitForUi(page,snapshot=>[enable,disable].some(id=>ui.findWidget(snapshot,id)),'accepted Subscribed Mods row');
      if(ui.findWidget(await ui.readUi(page),enable)){await ui.click(page,enable);activatedHere=true;}
      await ui.widget(page,disable);
      await page.screenshot({path: path.join(output, 'authored-enabled.png')});
      check(activatedHere ? 'actual account Cloud subscription enables the unchanged uploaded Source package'
        : 'retained Source package remains enabled after its earlier actual Cloud activation');
      await page.keyboard.press('Escape');
      await ui.click(page,'leave');
    }
    await ui.click(page,'play');
    if (continueAuthored) {
      await ui.click(page,'continue');
      await until(async () => player() || await consentVisible(), 'actual retained authored profile');
      if (await consentVisible()) {
        await ui.click(page,'routing-continue');
      }
      await hiddenMenu(page);
      await until(() => player() && !state.frame.host.loading_run_id && !state.frame.host.resume_grace,
        'actual retained authority readiness');
      state.continuedProfile = snapshot();
    } else {
      await ui.click(page,'new-game');
      await ui.waitForUi(page,snapshot=>snapshot.surface==='routing'||ui.findWidget(snapshot,'prompt-primary'),'local consent or character replacement prompt');
      if(ui.findWidget(await ui.readUi(page),'prompt-primary')) await ui.click(page,'prompt-primary');
      await ui.surface(page,'routing');
      assert.ok((await ui.widget(page,'routing/dialog')).value.includes('Apprentice Apothecary'));
      await ui.click(page,'routing-continue');
      await ui.waitForUi(page,snapshot=>snapshot.surface==='create'||ui.findWidget(snapshot,'panel:dialogue'),'real Create or College admission');
      if(ui.findWidget(await ui.readUi(page),'panel:dialogue'))await reachCollegeLoadout(page);
      await ui.fill(page,'create-name','PipStudent');await ui.click(page,'element:ether');await ui.click(page,'discipline:arcane');
      await hiddenMenu(page);
    }
    await until(() => state.runtime?.shops?.length && player(), 'real accepted content and runtime');
    assert.equal(state.welcomes.at(-1).session_kind, 'PrivateCollege');
    const identity = state.welcomes.at(-1).content.mods.find(mod => mod.id === acceptedPackage.packageId);
    assert.ok(identity, 'the authority accepted the enabled package identity');
    assert.equal(identity.contentSha256, acceptedPackage.contentSha256);
    check('actual package identity admits a private authority and nonempty prepared runtime');
    let runId;
    if (priorOnwardPhase) {
      const accepted = priorOnwardPhase.accepted;
      assert.equal(state.frame.snapshot.boneyard?.run_id, accepted.runId);
      assert.equal(player().economy.gold, accepted.gold);
      assert.equal(itemQuantity('Moondust'), accepted.moondust);
      assert.equal(itemQuantity('Ward Tonic'), accepted.tonic);
      assert.ok(firstPurchase());
      runId = accepted.runId;
      check('the real retained authored run continues after the independently proven earlier phases');
    } else {
      mark(continueAuthored ? 'continue with the real purchased authored items' : 'walk to Pip and purchase through Source ModPanels');
      if (continueAuthored) {
        assert.equal(itemQuantity('Ward Tonic'), 1, 'the saved profile retains the purchased Tonic');
        assert.equal(itemQuantity('Moondust'), 3, 'the saved profile retains actual purchase outputs');
        assert.ok(firstPurchase(), 'the real prepared profile state survived Continue');
      } else {
        // The native north arch blocks a direct southward walk (hub.rs's source
        // regression covers it). Use the western stair corridor; each segment was
        // checked with Source isPathTraversableAgainstHubSegments at radius25.
        for (const [x, y] of [[840, 195], [810, 195], [720, 345], [735, 465],
          [1450, 740]]) await moveTo(page, x, y);
        const beforeGold = player().economy.gold;
        await clickLabel(page,/Apprentice Apothecary · 8 gold/,{prefix:'mod/panel:'});
        await until(() => itemQuantity('Moondust') === 2 && firstPurchase(), 'actual purchase plus Lua grant and semantic state');
        assert.equal(player().economy.gold, beforeGold - 8);
        assert.ok(state.audioEvents.some(event => event.path === 'audio/handleCoins.ogg'));
        const buyTonic=(await actionByLabel(page,/Apprentice Apothecary · 30 gold/,{prefix:'mod/panel:'})).id;
        if(!(await ui.widget(page,buyTonic)).enabled){mark('wait for actual retained shop stock');await ui.waitForUi(page,snapshot=>ui.findWidget(snapshot,buyTonic)?.enabled,'actual two-minute restock',{timeout:180000});}
        await ui.click(page,buyTonic);
        await until(() => itemQuantity('Ward Tonic') === 1 && itemQuantity('Moondust') === 3, 'custom potion inventory grant');
      }
      if (priorPhase) {
        check('matched-bundle prior receipt retains the proven custom potion rejection/restoration');
      } else {
        await openInventory(page);
        const tonicId=items('Ward Tonic')[0].id;
        await assertAuthoredIcon(page,tonicId);await assertAuthoredIcon(page,items('Moondust')[0].id);
        await assertRejectedCustomPotionBinding(page,tonicId);
        await page.screenshot({path: path.join(output, 'authored-inventory.png')});
        await ui.click(page,'panel:close');
        await ui.noWidget(page,'panel:surface:inventory');
      }
      // Source forbids removing worn robes. The earlier real character replacement
      // left its gear in Luthacus's scavenged bag; retrieve that actual spare robe.
      // ModPanels reforges the first root-backpack equipment, so move only the robe
      // out of the bag through the visible Back target before returning to Pip.
      const isRobe = item => String(item.equipment_type).toLowerCase() === 'robe';
      const rootRobe = player().economy.backpack.find(isRobe);
      const hasSpareRobe = item => item.contents?.some(isRobe);
      const bag = player().economy.backpack.find(hasSpareRobe)
        || player().economy.storage.find(hasSpareRobe);
      assert.ok(rootRobe || bag, 'the retained profile supplies its real scavenged robe');
      const robeId = rootRobe?.id ?? bag.contents.find(isRobe).id;
      state.spareRobeProof = {robeId, location: rootRobe ? 'root backpack' : 'scavenged bag'};
      if (rootRobe) {
        // A prior ordinary child→Back drag already moved this item. Continue from
        // the actual accepted inventory instead of repeating that passed phase.
        check('the real spare robe remains in the root backpack for the remaining reforge phase');
      } else {
        if (!player().economy.backpack.some(item => item.id === bag.id)) {
          await moveTo(page, 1640, 520);
          await ui.click(page,'hud:shortcut:2');
          await ui.widget(page,'panel:luthacus');
          await ui.click(page,`panel:storage:${bag.id}`,{clickCount:2});
          await until(() => player().economy.backpack.some(item => item.id === bag.id),
            'actual scavenged bag transfer');
          await ui.click(page,'panel:close');
          await ui.noWidget(page,'panel:surface:inventory');
        }
        await openInventory(page);
        await ui.click(page,`panel:item:${bag.id}`,{clickCount:2});
        const robeAdmission=async()=>{
          const snapshot=await ui.readUi(page);const item=ui.findWidget(snapshot,`panel:item:${robeId}`);
          const sack=ui.findWidget(snapshot,'panel:sack-page');const pageState=sack?.value?JSON.parse(sack.value):null;
          return {disabled:!item?.enabled,page:pageState?.phase==='current'?'current':pageState?.phase,
            pageState,bookOpacity:item?.enabled?'1':'0',flyby:Boolean(ui.findWidget(snapshot,'panel:inventory-flyby'))};
        };
        state.robeDragProof={before:await robeAdmission()};
        await until(async()=>{const state=await robeAdmission();return !state.disabled&&state.page==='current'&&!state.flyby;},'enabled robe on settled current Sack page');
        state.robeDragProof.ready=await robeAdmission();
        const source=ui.bounds(await ui.widget(page,`panel:item:${robeId}`));const destination=ui.bounds(await ui.widget(page,'panel:back'));
        state.robeDragProof.source=source;state.robeDragProof.destination=destination;
        await observePanelInput(page);const start=ui.center(source),end=ui.center(destination);
        await page.mouse.move(start.x,start.y);await page.mouse.down();await page.mouse.move(end.x,end.y,{steps:12});await ui.widget(page,'panel:dragger');await page.mouse.up();
        state.robeDragProof.pointers=(await page.evaluate(()=>window.stopAuthoredPanelObservation())).pointers;
        await until(() => player().economy.backpack.some(item => item.id === robeId),
          'actual spare robe movement into the root backpack');
        state.spareRobeProof.movedToRoot = true;
        await ui.noWidget(page,'panel:inventory-flyby');
        // The footer closes the book directly; Back starts another Sack slide and
        // an immediate Escape can be consumed by that still-active transition.
        await ui.click(page,'panel:close');
        await ui.noWidget(page,'panel:surface:inventory');
      }
      await approachApothecary(page);
      const goldBeforeReforge = player().economy.gold;
      await clickLabel(page,/Apprentice Apothecary · Reforge · 25 gold/,{prefix:'mod/panel:'});
      await until(() => player().economy.backpack.some(item => item.id === robeId
        && item.mod_affixes?.some(affix => affix.name === 'Clear Minded')), 'actual Clear Minded reforge');
      assert.equal(player().economy.gold, goldBeforeReforge - 25);
      await openInventory(page);
      await ui.click(page,`panel:item:${robeId}`,{clickCount:2});
      await until(() => !player().economy.backpack.some(item => item.id === robeId), 'actual robe re-equip');
      await page.screenshot({path: path.join(output, 'authored-affix.png')});
      await page.keyboard.press('Escape');
      check(continueAuthored
        ? 'real retained purchase outputs, custom icons, semantic state and new robe affix'
        : 'real stock, gold, custom icons, purchase rule, bonus item, semantic state, robe affix and authored audio');
      mark('enter an ordinary Boneyard and activate the authored minimap');
      await clickLabel(page,'Enter the Boneyard',{prefix:'mod/panel:'});
      await until(() => state.frame?.snapshot.boneyard && !state.frame.host.loading_run_id && !state.frame.host.resume_grace && !state.frame.host.pause, 'actual Boneyard readiness', 120000);
      runId = state.frame.snapshot.boneyard.run_id;
      await ui.widget(page,'mod/minimap');
      const beforeAudio = await page.evaluate(() => window.authoredAudioStarts.length);
      const beforePresentation = new Set(state.audioEvents.map(event => event.owner));
      await ui.click(page,'mod/minimap-action:ping');
      await until(() => state.actions.some(action => action.action === 'ui-action') && state.audioEvents.some(event => event.path === 'audio/bookOpen.ogg' && !beforePresentation.has(event.owner)), 'actual authored UI rule and sound event');
      const pageSoundHash = state.welcomes.at(-1).mod_assets.find(asset => asset.path === 'audio/bookOpen.ogg').sha256;
      await until(() => page.evaluate(({count, hash}) => window.authoredAudioStarts.slice(count)
        .some(source => source.identity?.sha256 === hash), {count: beforeAudio, hash: pageSoundHash}),
        'the actual authored audio buffer submission');
      state.authoredAudioProof = await page.evaluate(({count, hash}) => window.authoredAudioStarts.slice(count)
        .filter(source => source.identity?.sha256 === hash), {count: beforeAudio, hash: pageSoundHash});
      await page.screenshot({path: path.join(output, 'authored-minimap.png')});
      check('actual minimap ping crosses the host rule boundary and submits authored audio');
      mark('consume the actual custom potion');
      await openInventory(page);
      await ui.click(page,`panel:item:${items('Ward Tonic')[0].id}`,{clickCount:2});
      await until(() => itemQuantity('Ward Tonic') === 0 && state.content?.statuses?.some(status =>
        status.targetId === state.id), 'real custom potion consume and on-use status');
      const appliedStatus = state.content.statuses.find(status => status.targetId === state.id);
      assert.equal(appliedStatus.expiresTick - appliedStatus.startedTick, 2000,
        'the accepted 20-second Source status is applied by the real rule');
      await page.keyboard.press('Escape');
      await ui.noWidget(page,'panel:surface:inventory');
      check('actual custom potion Inventory use commits inventory and source on-use effects');
    }
    if (includeConsole || observeConsole) {
      mark('execute real authoritative Console work');
      await page.keyboard.press('Escape');
      await ui.click(page,'pause-settings');
      await ui.click(page,'settings-page:cloud');
      const cheats='settings-toggle:cheats';
      state.cheatProof = {before: String((await ui.widget(page,cheats)).selected)};
      if (String((await ui.widget(page,cheats)).selected) === 'false') await ui.click(page,cheats);
      await pause(1000);
      if (witnessConsole) {
        let replyCount = state.cheatModes.length;
        await ui.click(page,cheats);
        await until(() => state.cheatModes.slice(replyCount).some(reply => !reply.enabled),
          'actual disabled cheat-mode reply');
        replyCount = state.cheatModes.length;
        await ui.click(page,cheats);
        await until(() => state.cheatModes.slice(replyCount).some(reply => reply.enabled),
          'actual re-enabled cheat-mode reply');
      }
      state.cheatProof.after = String((await ui.widget(page,cheats)).selected);
      state.cheatProof.disabled = !(await ui.widget(page,cheats)).enabled;
      const welcome = state.welcomes.at(-1);
      const party = state.social?.party_state?.party;
      state.cheatProof.admission = {
        localPlayerId: state.id,
        authorityPlayerId: state.frame.host.authority_player_id,
        connectedPlayerIds: state.frame.host.connected_player_ids,
        sessionKind: welcome.session_kind,
        developerAccess: welcome.developer_access,
        welcomeCheatsEnabled: welcome.cheats_enabled,
        helloCheats: state.helloCheats,
        commands: state.cheatCommands,
        replies: state.cheatModes,
        party: party && {id: party.id, leaderPlayerId: party.leader_player_id,
          memberPlayerIds: party.member_player_ids, visibility: party.visibility},
      };
      fs.writeFileSync(path.join(output, 'console-admission.json'), JSON.stringify(state.cheatProof, null, 2));
      await ui.click(page,'settings-back');
      await ui.widget(page,'settings-page:cloud');
      await ui.click(page,'settings-back');
      await ui.surface(page,'pause');
      state.cheatProof.pauseRowVisible = Boolean(ui.findWidget(await ui.readUi(page),'pause-cheats'));
      state.cheatProof.pauseRows=(await ui.readUi(page)).widgets.filter(widget=>['resume','pause-cheats','pause-settings','leave'].includes(widget.id)).map(widget=>({action:widget.id,label:widget.label}));
      fs.writeFileSync(path.join(output, 'console-admission.json'), JSON.stringify(state.cheatProof, null, 2));
      if (witnessConsole) {
        assert.equal(state.cheatProof.admission.authorityPlayerId, state.id);
        assert.equal(state.cheatModes.at(-1).enabled, true);
        assert.equal(state.cheatProof.after, 'true');
        const expectedRow = process.env.EXPECT_CHEAT_ROW === 'present';
        assert.equal(state.cheatProof.pauseRowVisible, expectedRow);
        await page.screenshot({path: path.join(output, 'console-row-witness.png')});
        await ui.settleErrors(page);
        fs.writeFileSync(path.join(output, 'console-row-witness.json'), JSON.stringify({
          matched: true, expectedRow, errors, admission: state.cheatProof, accepted: snapshot()}, null, 2));
        assert.deepEqual(errors, []);
        return;
      }
      if (includeConsole) await ui.widget(page,'pause-cheats');
      await resumeFromPause(page);
      await gameplayReady();
      if (includeConsole) {
        await page.keyboard.press('Backquote');
        await ui.widget(page,'cheats/dialog');
        await ui.click(page,'cheats/tab:console');
        await ui.widget(page,'cheats/lua-editor');
        let result = await runLua(page, "return _VERSION, sd.runtime.api_version, sd.player.get_state().gold, type(sd.dev)");
        assert.equal(result.ok, true, result.error);
        assert.deepEqual(result.values, ['Lua 5.4', '1.0.0', player().economy.gold, 'nil']);
        result = await runLua(page, "sd.state.set('parity.counter',41); sd.timer.after(10,function() sd.state.set('parity.counter',42) end); return true");
        assert.equal(result.ok, true, result.error);
        await pause(40);
        result = await runLua(page, "return sd.state.get('parity.counter'), sd.state.is_authority(), type(io), type(sd.events.filter)");
        assert.equal(result.ok, true, result.error);
        assert.deepEqual(result.values, [42, true, 'nil', 'nil']);
        const gold = player().economy.gold - 1;
        result = await runLua(page, `return sd.player.set_gold(${gold})`);
        assert.equal(result.ok, true, result.error);
        await until(() => player().economy.gold === gold, 'actual Console mutation');
        result = await runLua(page, 'while true do end');
        assert.equal(result.ok, false);
        assert.match(result.error, /thread timeout exceeded/);
        result = await runLua(page, 'return 42');
        assert.equal(result.ok, true, result.error);
        assert.deepEqual(result.values, [42]);
        await page.screenshot({path: path.join(output, 'authored-console.png')});
        await page.keyboard.press('Backquote');
        await ui.noWidget(page,'cheats/dialog');
        check('real Lua values, state, timer, sandbox, queued mutation and timeout recovery keep the client alive');
      }
    }
    if (authoredPhase === 'console') {
      await ui.settleErrors(page);
      assert.deepEqual(errors, []);
      fs.writeFileSync(path.join(output, 'authored-console.json'), JSON.stringify({
        passed: true, source: acceptedPackage.source, package: acceptedPackage,
        checks, errors, priorOnwardPhase, priorSavePhase, consoleExecuted: true,
        cheatProof: state.cheatProof, accepted: snapshot(),
        lua: state.lua.map(({request_id, ok, output, values, error}) =>
          ({request_id, ok, output, values, error})),
      }, null, 2));
      return;
    }
    mark('save and resume the same authored run');
    const beforeLeave = state.leaveReceipts;
    await gameplayReady();
    await ui.click(page,'hud:pause');
    await ui.surface(page,'pause');
    await ui.click(page,'leave');
    await ui.surface(page,'title');
    assert.ok(state.leaveReceipts > beforeLeave);
    assert.ok(state.lastSave, 'the actual host supplied its signed source continuation');
    const saved = JSON.parse(state.lastSave);
    const modState = saved.modState;
    assert.ok(modState && Object.keys(modState).length, 'accepted authored runtime reaches the real save');
    const beforeReload = snapshot();
    state.frame = null; state.runtime = null;
    await ui.observeDocumentReplacement(page, () => page.reload({waitUntil: 'load'}));
    await ui.ready(page);
    await ui.click(page,'play');
    await ui.click(page,'continue');
    await until(async () => state.frame?.snapshot.boneyard?.run_id === runId || await consentVisible(),
      'actual resume admission or local-content consent', 120000);
    if (await consentVisible()) await ui.click(page,'routing-continue');
    await until(() => state.frame?.snapshot.boneyard?.run_id === runId && firstPurchase() && itemQuantity('Moondust') === 3 && !state.frame.host.loading_run_id && !state.frame.host.resume_grace, 'real saved run and prepared state continuation', 120000);
    assert.equal(player().selection.name, 'PipStudent');
    assert.equal(player().economy.gold, beforeReload.gold);
    const resumedTick = state.frame.snapshot.ticks;
    await until(() => state.frame.snapshot.ticks > resumedTick + 30, 'continued authored authority');
    await page.screenshot({path: path.join(output, 'authored-resumed.png')});
    check('real checkpoint persistence and same-run reload retain inventory, semantic state and later authority ticks');
    await ui.settleErrors(page);
    assert.deepEqual(errors, []);
    const receipt = {passed: true, package: acceptedPackage, checks, requests, expectedNotFound,
      continuedProfile: state.continuedProfile, priorPhase, priorSackPhase, priorOnwardPhase,
      cheatProof: state.cheatProof, consoleExecuted: includeConsole,
      cancelledMedia, errors, actions: state.actions, assets: [...new Set(state.assets)],
      lua: state.lua.map(({request_id, ok, output, values, error}) => ({request_id, ok, output, values, error})),
      savedModStateSha256: sha256(JSON.stringify(modState)), continued: snapshot(),
      audioEvents: state.audioEvents, authoredAudioProof: state.authoredAudioProof, iconProof: state.iconProof,
      rejectedBeltProof: state.rejectedBeltProof,
      robeDragProof: state.robeDragProof,
      spareRobeProof: state.spareRobeProof,
      webAudioStarts: await page.evaluate(() => window.authoredAudioStarts)};
    fs.writeFileSync(path.join(output, 'authored-content.json'), JSON.stringify(receipt, null, 2));
    console.log(JSON.stringify({passed: true, checks, errors}));
  } catch (error) {
    await ui.settleErrors(page);
    await page.screenshot({path: path.join(output, 'authored-content-failure.png')});
    fs.writeFileSync(path.join(output, 'authored-content-failure.json'), JSON.stringify({step, error: error.stack, checks, errors, requests, expectedNotFound, continuedProfile: state.continuedProfile, priorPhase, priorSackPhase, priorOnwardPhase, accepted: snapshot(), wireTypes: state.wireTypes, movementFailure: state.movementFailure, rejectedBeltProof: state.rejectedBeltProof,
      cheatProof: state.cheatProof,
      spareRobeProof: state.spareRobeProof,
      robeDragProof: state.robeDragProof ? {...state.robeDragProof,
        pointers: await page.evaluate(() => window.authoredRobePointers || [])} : undefined,
      ui: await ui.readUi(page)}, null, 2));
    throw error;
  } finally {
    const cleanup = {};
    await bounded(browser.close(), 'browser connection close', 5000).catch(error => {
      cleanup.connectionError = error.message;
    });
    try {
      await bounded(server.close(), 'owned Chrome close', 15000);
      cleanup.mode = 'graceful';
    } catch (error) {
      cleanup.error = error.message;
      await bounded(server.kill(), 'owned Chrome termination', 5000).catch(error => {
        cleanup.terminationError = error.message;
      });
      cleanup.mode = 'forced';
    }
    ownedBrowserClosed = server.process().exitCode !== null || server.process().signalCode !== null;
    cleanup.browserClosed = ownedBrowserClosed;
    fs.writeFileSync(path.join(output, 'browser-cleanup.json'), JSON.stringify(cleanup, null, 2));
    assert.ok(ownedBrowserClosed, 'the owned Chrome process exited');
  }
})().then(() => {
  // Playwright's server socket can outlive its already-exited Chrome child.
  // Finish only after the cleanup receipt confirms that child is closed.
  if (ownedBrowserClosed) process.exit(0);
}).catch(error => {
  console.error(error.message);
  if (ownedBrowserClosed) process.exit(1);
  process.exitCode = 1;
});
