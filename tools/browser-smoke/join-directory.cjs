const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
// Owned parties only: real visibility, directory requests, code Enter and admission.
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const gpu = require('./gpu-ui.cjs');
const {startNewGame} = require('./menu-journey.cjs');
const {createNativeMessageReader} = require('./native-wire.cjs');
const game = new URL(process.env.URL);
const mount = game.pathname.replace(/\/$/, '');
const output = path.resolve(process.env.OUT);
fs.mkdirSync(output, {recursive:true});
const checks = [], clients = [], accounts = [];
let step = 'registration';
function checked(label) {checks.push(label);console.log(label);}

async function until(predicate, description, timeout = 45000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    if (predicate()) return;
    await new Promise(resolve => setTimeout(resolve, 40));
  }
  throw new Error(`Timed out waiting for ${description}`);
}

async function client(browser, label, authenticated = true) {
  const context = await browser.newContext({ignoreHTTPSErrors:process.env.LOCAL_TLS === '1',
    viewport:{width:1600,height:900}});
  const page = await context.newPage();
  page.setDefaultTimeout(45000);
  const observation = gpu.watchErrors(page);
  await observation.ready;
  const state = {label, id:null, frame:null, social:null, directories:[], joins:[], sockets:[], saveWrites:[]};
  page.on('response', async response => {
    const pathname = new URL(response.url()).pathname;
    if (pathname.endsWith('/api/game/parties') && response.ok()) state.directories.push(await response.json());
    if (pathname.includes('/api/game/join/')) state.joins.push({path:pathname,
      method:response.request().method(),status:response.status()});
    if (pathname.endsWith('/api/game/saves/0') && response.request().method() === 'PUT')
      state.saveWrites.push(response.status());
  });
  page.on('websocket', socket => {
    const connection = {path:new URL(socket.url()).pathname, closed:false};
    state.sockets.push(connection);
    socket.on('close', () => {connection.closed = true;});
    const read = createNativeMessageReader();
    socket.on('framereceived', ({payload}) => {
      const message = read(JSON.parse(payload.toString()));
      if (!message) return;
      if (message.type === 'welcome') {state.id = message.credentials.player_id;state.frame = message.frame;}
      else if (message.type === 'snapshot') state.frame = message.frame;
      else if (message.type === 'social') state.social = message.snapshot;
      else if (message.type === 'error') observation.errors.push(`host ${message.code}: ${message.reason}`);
    });
  });
  const result = {context,page,state,observation};
  clients.push(result);
  if (authenticated) {
    const suffix = `${Date.now().toString(36)}${crypto.randomBytes(2).toString('hex')}`;
    const account = {username:`Directory${label}${suffix}`,email:`directory-${suffix}@example.invalid`,
      password:`Qa!${crypto.randomBytes(18).toString('base64url')}`};
    accounts.push(account);
    fs.writeFileSync(path.join(output,'private-accounts.json'),JSON.stringify(accounts),{mode:0o600});
    fs.chmodSync(path.join(output,'private-accounts.json'),0o600);
    await page.goto(new URL(`${mount}/register`, game).href);
    for (const [autocomplete,value] of [['username',account.username],['email',account.email],
      ['new-password',account.password]]) await page.locator(`input[autocomplete="${autocomplete}"]`).fill(value);
    await page.getByRole('button',{name:'Create account',exact:true}).click();
    await page.waitForURL(url => url.pathname === `${mount}/account`);
    await gpu.observeDocumentReplacement(page, () =>
      page.getByRole('navigation').getByRole('link',{name:'Play',exact:true}).click());
  } else await page.goto(game.href);
  await gpu.ready(page);
  if (gpu.findWidget(await gpu.readUi(page),'prompt-secondary')) await gpu.click(page,'prompt-secondary');
  return result;
}

async function visibility(owner, value, native) {
  const {page,state} = owner;
  await gpu.useNativeFocus(page);
  await page.bringToFront();
  if (!gpu.findWidget(await gpu.readUi(page),`social/visibility:${value}`)) {
    if (gpu.findWidget(await gpu.readUi(page),'social/party-menu')) await gpu.click(page,'social/tab:settings');
    else await gpu.click(page,'social/party-settings-shortcut');
  }
  await gpu.click(page,`social/visibility:${value}`);
  await until(() => state.social?.party_state?.party.visibility === native, `actual ${value} party visibility`);
  await gpu.click(page,'social/close');
}

async function joinMenu(visitor) {
  const {page} = visitor;
  await gpu.useNativeFocus(page);
  await page.bringToFront();
  const surface = (await gpu.readUi(page)).surface;
  if (surface === 'title') {
    await gpu.click(page,'play');
    await gpu.surface(page,'play');
  }
  if ((await gpu.readUi(page)).surface === 'play') await gpu.click(page,'join-party');
  await gpu.surface(page,'join-party');
}

async function refresh(visitor, id, label) {
  const {page,state} = visitor;
  await gpu.useNativeFocus(page);
  await page.bringToFront();
  const count = state.directories.length;
  await gpu.click(page,'join-refresh');
  await until(() => state.directories.length > count, 'actual directory refresh response');
  const actual = state.directories.at(-1).items.find(item => item.id === id);
  if (label == null) {
    assert.equal(actual,undefined,'the owned private party is not published');
    await gpu.noWidget(page,`join-listing:${id}`);
  } else {
    assert.ok(actual,'the row belongs to the real owned party');
    const snapshot = await gpu.waitForUi(page, snapshot =>
      gpu.findWidget(snapshot,`join-listing:${id}`)?.label === label, `owned listing ${label}`);
    return gpu.findWidget(snapshot,`join-listing:${id}`);
  }
}

(async () => {
  const owned = await launchOwnedBrowser(chromium,{headless:true,
    executablePath:process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=metal','--ignore-gpu-blocklist']});
  const browser = owned.browser;
  let failure;
  try {
    const leader = await client(browser,'A');
    await gpu.click(leader.page,'settings');
    await gpu.click(leader.page,'settings-page:cloud');
    if ((await gpu.widget(leader.page,'settings-toggle:shared-hub')).value === 'true')
      await gpu.click(leader.page,'settings-toggle:shared-hub');
    await gpu.value(leader.page,'settings-toggle:shared-hub','false');
    await gpu.click(leader.page,'settings-back');
    await gpu.click(leader.page,'settings-back');
    await gpu.surface(leader.page,'title');
    await startNewGame(leader.page,{name:'Leader'});
    await until(() => leader.state.social?.party_state?.party.listing_id, 'owned private College party');
    const party = leader.state.social.party_state.party;
    const listing = party.listing_id;
    const code = party.join_code;
    assert.equal(typeof code,'string');
    assert.equal(party.leader_player_id,leader.state.id);
    checked('ordinary registration and real College admission create an isolated owned party');

    const visitor = await client(browser,'B');
    await joinMenu(visitor);
    step = 'private visibility';
    await visibility(leader,'private','Private');
    await refresh(visitor,listing,null);
    checked('the owned Private party remains absent from the actual broker and GPU directory');

    step = 'public directory admission cancellation';
    await visibility(leader,'public','Public');
    assert.equal((await refresh(visitor,listing,'JOIN')).enabled,true);
    await gpu.click(visitor.page,`join-listing:${listing}`);
    await gpu.widget(visitor.page,'routing/dialog');
    assert.ok(visitor.state.joins.some(row => row.status === 200));
    assert.equal(visitor.state.id,null,'the consent has not admitted a player');
    await visitor.page.screenshot({path:path.join(output,'owned-public-consent.png')});
    await gpu.click(visitor.page,'routing-back');
    await joinMenu(visitor);
    assert.equal(visitor.state.id,null);
    checked('the real public row opens owned-party consent and Back cancels before membership');

    step = 'invite-only request';
    await visibility(leader,'invite-only','InviteOnly');
    assert.equal((await refresh(visitor,listing,'REQUEST')).enabled,true);
    await gpu.click(visitor.page,`join-listing:${listing}`);
    await gpu.waitForUi(visitor.page,snapshot =>
      gpu.findWidget(snapshot,`join-listing:${listing}`)?.label === 'REQUESTED','actual pending invitation request');
    await until(() => leader.state.social?.party_state?.join_requests.length > 0,'the leader receives the real request');
    const request = leader.state.social.party_state.join_requests.find(request =>
      request.requester.account_username === accounts[1].username);
    assert.ok(request,'only the owned visitor request is acted on');
    await gpu.useNativeFocus(leader.page);
    await leader.page.bringToFront();
    await gpu.click(leader.page,'social/party');
    await gpu.click(leader.page,'social/tab:members');
    await gpu.click(leader.page,`social/deny:${request.id}`);
    await gpu.click(leader.page,'social/close');
    await until(() => !leader.state.social.party_state.join_requests.some(row => row.id === request.id), 'actual leader denial');
    await refresh(visitor,listing,'REQUEST');
    checked('Invite Only publishes REQUEST, records REQUESTED, and the actual leader can deny it');

    step = 'code Enter admission';
    await visibility(leader,'public','Public');
    await refresh(visitor,listing,'JOIN');
    await gpu.fill(visitor.page,'join-party-code',code.toLowerCase());
    await gpu.value(visitor.page,'join-party-code',code);
    await visitor.page.keyboard.press('Enter');
    await gpu.widget(visitor.page,'routing/dialog');
    await gpu.click(visitor.page,'routing-continue');
    await gpu.surface(visitor.page,'create');
    await gpu.fill(visitor.page,'create-name','Visitor');
    await gpu.click(visitor.page,'element:ether');
    await gpu.click(visitor.page,'discipline:arcane');
    await gpu.surface(visitor.page,'hidden');
    await until(() => visitor.state.social?.party_state?.party.id === party.id
      && leader.state.frame?.host.connected_player_ids.includes(visitor.state.id),'actual owned-party authority admission');
    const selection = (await gpu.readUi(visitor.page)).acceptedSelection;
    assert.deepEqual(selection,{name:'Visitor',element:'Ether',discipline:'Arcane'});
    checked('real Party ID typing and Enter resolve, consent and admit the requested wizard to the owned authority');

    step = 'playing directory policy';
    await gpu.useNativeFocus(leader.page);
    await leader.page.bringToFront();
    await gpu.click(leader.page,'hud:map');
    await until(() => leader.state.frame?.snapshot.boneyard && visitor.state.frame?.snapshot.boneyard,'actual shared Boneyard entry',120000);
    const spectator = await client(browser,'C',false);
    await joinMenu(spectator);
    const playing = await refresh(spectator,listing,'IN GAME');
    assert.equal(playing.enabled,false,'the actual playing party cannot be joined through its row');
    await spectator.page.screenshot({path:path.join(output,'owned-party-in-game.png')});
    checked('the actual running owned party is displayed IN GAME with its Join action disabled');
    for (const owner of [leader,visitor]) {
      await gpu.useNativeFocus(owner.page);
      await owner.page.bringToFront();
      const before = owner.state.saveWrites.length;
      await gpu.click(owner.page,'hud:pause');
      await gpu.surface(owner.page,'pause');
      await gpu.click(owner.page,'leave');
      await gpu.surface(owner.page,'title');
      await until(() => owner.state.saveWrites.length > before,'actual final account checkpoint persistence');
      assert.equal(owner.state.saveWrites.at(-1),200);
    }
    checked('both admitted players leave through the real menu and persist their actual final checkpoint');
    for (const owner of clients) {
      await gpu.assertGpuOnly(owner.page);
      await gpu.settleErrors(owner.page);
      assert.deepEqual(owner.observation.errors,[]);
    }
  } catch (error) {
    failure = {step,error:error.stack};
    for (const owner of clients) {
      owner.state.failureUi = await gpu.readUi(owner.page).catch(() => null);
      await owner.page.screenshot({path:path.join(output,`failure-${owner.state.label}.png`)}).catch(() => {});
    }
    process.exitCode = 1;
  } finally {
    try {
      for (const owner of clients) await gpu.settleErrors(owner.page);
      fs.writeFileSync(path.join(output,'receipt.json'),JSON.stringify({passed:!failure,checks,failure,
        clients:clients.map(({state,observation}) => ({label:state.label,id:state.id,
          partyId:state.social?.party_state?.party.id,joins:state.joins,
          directories:state.directories,errors:observation.errors,sockets:state.sockets,saveWrites:state.saveWrites,
          failureUi:state.failureUi}))},null,2));
      console.log(JSON.stringify({checks,failure}));
    } finally {console.log(JSON.stringify({browserCleanup:await owned.close()}));}
  }
})().catch(error => {console.error(error);process.exitCode = 1});
