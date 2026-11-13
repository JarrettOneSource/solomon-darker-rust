// Rejection presentation at the native command-result boundary. The real host
// supplies the College and roster. The fixture selects private-College social
// presentation, adds a pending row and refuses the ordinary Leave action.
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const ui = require('./gpu-ui.cjs');
const {startNewGame} = require('./menu-journey.cjs');
const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');

const output = path.resolve(process.env.OUT);
const touch = process.env.TOUCH === '1';
const activate = touch ? ui.tap : ui.click;

(async () => {
  await fs.mkdir(output, {recursive:true});
  const owned = await launchOwnedBrowser(chromium, {
    headless:true,
    executablePath:process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=metal','--ignore-gpu-blocklist'],
  });
  const receipt = {passed:false, touch, checks:[], errors:[]};
  const cases = [
    ['That invitation is already pending.',2],
    ['Only the party leader can do that.',2],
    ['That party is full.',1],
    ['That wizard is not in the Courtyard.',2],
    ['That wizard is already in a party.',2],
    ['That join request is already pending.',2],
    ['That party is private.',1],
    ['You cannot target yourself.',2],
    ['That invitation has expired.',2],
    ['That join request has expired.',2],
    ['That invitation belongs to another wizard.',2],
    ['That wizard is no longer available.',2],
    ['That wizard is already in your party.',2],
    ['That party is no longer available.',2],
    ['',0],
  ];
  let page, observed;
  let rejection = 'That invitation is already pending.';
  let rejected = 0;
  try {
    page = await owned.browser.newPage({viewport:{width:1600,height:900},hasTouch:touch});
    observed = ui.watchErrors(page, {errors:receipt.errors});
    await observed.ready;
    await page.routeWebSocket('**/session', socket => {
      const server = socket.connectToServer();
      server.onMessage(raw => {
        const message = JSON.parse(raw.toString());
        if (message.type === 'social') {
          message.snapshot.session_kind = 'PrivateCollege';
          message.snapshot.party_state.join_requests = [{id:'error-layout-request',requester:{
            requester_id:'error-layout-requester',account_username:null,display_name:'Waiting Wizard',
          }}];
          socket.send(JSON.stringify(message));
        } else socket.send(raw);
      });
      socket.onMessage(raw => {
        const message = JSON.parse(raw.toString());
        if (message.type === 'command' && message.command.kind === 'social'
          && message.command.action === 'LeaveParty') {
          rejected++;
          socket.send(JSON.stringify({type:'command_result',sequence:message.sequence,
            snapshot_sequence:0,accepted:false,reason:rejection,inventory_result:null}));
        } else server.send(raw);
      });
    });
    await page.goto(process.env.URL || 'http://127.0.0.1:43186/?host=1', {waitUntil:'domcontentloaded'});
    await ui.ready(page);
    console.log('assets ready');
    await startNewGame(page, {touch});
    console.log('College ready');
    await ui.widget(page,'hud:pause');
    if (touch) {
      await page.setViewportSize({width:896,height:414});
      await activate(page,'social/party');
      await ui.waitForUi(page, snapshot => snapshot.widgets.filter(widget =>
        widget.id.startsWith('social/member:') || widget.id.startsWith('social/request:'))
        .length===2,'the touch Party card expands its member and pending request');
    }
    const bodyBounds = async () => ui.bounds(await ui.widget(page,'social/party-chip-body'),{paint:true});
    const baseline = await bodyBounds();
    const scale = touch ? 0.55 : 1;
    const rowOffsets = async () => {
      const snapshot = await ui.readUi(page);
      const body = ui.bounds(ui.findWidget(snapshot,'social/party-chip-body'),{paint:true});
      return snapshot.widgets.filter(widget=>widget.id.startsWith('social/member:')
        || widget.id.startsWith('social/request:'))
        .map(widget=>ui.bounds(widget,{paint:true}).y-body.y);
    };
    const baselineRows = await rowOffsets();
    assert.equal(baselineRows.length,2,'a fresh host supplies one member alongside the pending row');
    for (const [index,[message,lines]] of cases.entries()) {
      rejection = message;
      await activate(page,'social/party-settings-shortcut');
      // Opening the Party menu retires its previous feedback and restores the card.
      await ui.waitForUi(page, snapshot => {
        const body = ui.findWidget(snapshot,'social/party-chip-body');
        return ui.findWidget(snapshot,'social/party-menu') && body
          && Math.abs(ui.bounds(body,{paint:true}).height-baseline.height)<0.02;
      },'opening Party clears its previous message and padding');
      await activate(page,'social/leave');
      await ui.noWidget(page,'social/party-menu');
      for (let i=0;i<100 && rejected<index+1;i++) await page.waitForTimeout(50);
      assert.equal(rejected,index+1,'the ordinary UI command reaches the refusal fixture');
      await page.waitForTimeout(300);
      const body = await bodyBounds();
      const rows = await rowOffsets();
      if (!touch || index===0 || index===2) {
        await page.screenshot({path:path.join(output,`error-${index}-expanded.png`)});
      }
      receipt.checks.push({message,lines,expanded:body});
      assert.ok(Math.abs(body.height-baseline.height-lines*22*scale)<0.02,
        `${message}: all message lines reserve their full height before the roster`);
      assert.equal(rows.length,baselineRows.length);
      rows.forEach((offset,row)=>assert.ok(Math.abs(offset-baselineRows[row]-lines*22*scale)<0.02,
        'painted member and request hit regions move with the complete error'));
      if (touch) {
        await activate(page,'social/party');
        await ui.waitForUi(page, snapshot => !snapshot.widgets.some(widget =>
          widget.id.startsWith('social/member:') || widget.id.startsWith('social/request:')),
        'collapsed Party has no roster rows');
        const collapsed = await bodyBounds();
        receipt.checks.at(-1).collapsed = collapsed;
        assert.ok(Math.abs(collapsed.height-(lines ? 62+lines*22 : 50)*scale)<0.02,
          'collapsing hides the roster while retaining every error line');
        if(index===0 || index===2) {
          await page.screenshot({path:path.join(output,`error-${index}-collapsed.png`)});
        }
        await activate(page,'social/party');
        await ui.waitForUi(page, snapshot => snapshot.widgets.filter(widget =>
          widget.id.startsWith('social/member:') || widget.id.startsWith('social/request:'))
          .length===baselineRows.length,'expanded Party restores the roster');
        assert.ok(Math.abs((await bodyBounds()).height-body.height)<0.02,
          'expanding restores the same complete card');
      }
      console.log(JSON.stringify({message,lines,touch}));
    }
    await ui.settleErrors(page);
    assert.deepEqual(receipt.errors,[]);
    receipt.passed = true;
  } catch (error) {
    receipt.failure = error.stack;
    if(page) await page.screenshot({path:path.join(output,'failure.png')}).catch(()=>{});
    throw error;
  } finally {
    try {if(observed) await ui.settleErrors(page);}
    finally {
      receipt.warnings = observed?.warnings;
      receipt.browser = await owned.close();
      await fs.writeFile(path.join(output,'receipt.json'),JSON.stringify(receipt,null,2));
    }
  }
})().catch(error=>{console.error(error);process.exitCode=1;});
