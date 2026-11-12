// Real hosted belt casts, enemy-caused Game Over, College return and saved Continue.
// Console supplies declared skill/enemy prerequisites; WebGL hooks only observe.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {createHash} = require('node:crypto');
const {chromium} = require(process.env.PW);
const {createNativeMessageReader} = require('./native-wire.cjs');
const {launchOwnedBrowser} = require('./browser-lifecycle.cjs');
const {startNewGame} = require('./menu-journey.cjs');
const {walkSolomonEntrance,prepareBoneyardNavigation} = require('./solomon-entrance.cjs');
const {revealSettingsControl} = require('./settings-journey.cjs');
const ui = require('./gpu-ui.cjs');

assert.ok(process.env.OUT && process.env.URL && process.env.ACCOUNT, 'OUT, URL and ACCOUNT are required');
const output = path.resolve(process.env.OUT), url = process.env.URL;
const journey = process.env.JOURNEY || 'lifecycle';
assert.ok(['lifecycle','rendering'].includes(journey), 'JOURNEY is lifecycle or rendering');
assert.ok(['127.0.0.1','localhost'].includes(new URL(url).hostname),
  'Console prerequisites require the owned isolated local host');
const account = JSON.parse(fs.readFileSync(process.env.ACCOUNT));
const receipt = {passed:false, journey, artifact:process.env.ARTIFACT, checks:[], errors:[],
  qualification:'Private developer Console grants prerequisite skills/mana and spawns enemies. All casts, movement, death, Create, Leave and Continue use ordinary input. Resource observations forward unchanged WebGL calls.',
  welcomes:[], commands:[], results:[], saves:[], leaveReceipts:[], acknowledgements:[], saveUploads:[], worlds:[],
  chats:[], rendering:{samples:[], golemPhases:[], fireProjectiles:[], fireContacts:[]}};
let page, observed, frame, social, playerId, step = 'private hosted admission', observerDocument;
const actor = () => frame?.snapshot.participants[playerId];
const player = () => actor()?.player;
const world = () => frame?.snapshot.boneyard;
const secondary = () => frame?.snapshot.secondary.players[playerId];
const spellActors = () => frame?.snapshot.secondary.actors.filter(row => row.owner_id === playerId
  && ['Leviathan','StormCloud'].includes(row.kind)) || [];
const hash = text => createHash('sha256').update(text).digest('hex');

async function until(predicate, label, timeout=30000) {
  const end = Date.now()+timeout;
  while(Date.now()<end) {
    assert.deepEqual(observed.errors,[]); assert.deepEqual(receipt.errors,[]);
    if(await predicate())return;
    await page.waitForTimeout(25);
  }
  throw new Error(`Timed out: ${label}`);
}
async function shot(name) { await page.screenshot({path:path.join(output,`${name}.png`)}); }
async function ready() {
  await until(()=>player()&&!frame.host.loading_run_id&&!frame.host.resume_grace&&!frame.host.pause,
    'the real host admits gameplay',120000);
  await ui.waitForUi(page,snapshot=>snapshot.surface==='hidden'&&!snapshot.inputScope
    &&!ui.findWidget(snapshot,'cheats/dialog')&&!snapshot.widgets.some(row=>row.id.startsWith('panel:')),
  'ordinary gameplay owns input');
}

function observeWire() {
  page.on('websocket',socket=>{
    const read=createNativeMessageReader();
    socket.on('framesent',({payload})=>{
      const row=JSON.parse(payload.toString());
      if(row.type==='hosted'&&row.command.kind==='lua_execute')receipt.commands.push(row.command);
      else if(row.type==='command')receipt.commands.push(row.command);
      else if(row.type==='save_checkpoint_chunk_ack')receipt.acknowledgements.push(row);
    });
    socket.on('framereceived',({payload})=>{
      const row=read(JSON.parse(payload.toString())); if(!row)return;
      if(row.type==='welcome') {
        playerId=row.credentials.player_id; frame=row.frame;
        receipt.welcomes.push({playerId,sessionKind:row.hosted?.session_kind,
          developerAccess:row.hosted?.developer_access,cheatsEnabled:row.hosted?.cheats_enabled});
      } else if(row.type==='snapshot')frame=row.frame;
      else if(row.type==='social')social=row.snapshot;
      else if(row.type==='chat')receipt.chats.push({sequence:row.message.sequence,
        channel:row.message.channel,text:row.message.text,sender:row.message.sender.player_id});
      else if(row.type==='chat_rejected')receipt.errors.push(row.rejection);
      else if(row.type==='error')receipt.errors.push({code:row.code,reason:row.reason});
      else if(row.type==='hosted') {
        const message=row.message;
        if(message.kind==='lua_result')receipt.results.push(message);
        else if(message.kind==='save_checkpoint')receipt.saves.push({sequence:message.sequence,
          reason:message.reason,bytes:Buffer.byteLength(message.save),sha256:hash(message.save)});
        else if(message.kind==='save_before_leave')receipt.leaveReceipts.push(message);
      }
      if(journey==='rendering'&&(row.type==='welcome'||row.type==='snapshot'))observeRendering();
    });
  });
  page.on('response',response=>{
    const request=response.request();
    if(request.method()==='PUT'&&new URL(request.url()).pathname.endsWith('/api/game/saves/0')) {
      const document=request.postDataJSON().document;
      receipt.saveUploads.push({status:response.status(),bytes:Buffer.byteLength(document),sha256:hash(document)});
    }
  });
}

// Observe handles, attachments and draw use without editing state, resources or pixels.
function observeResources() {
  const contexts = new Map(), resources = [], byHandle = new WeakMap();
  let sequence = 0;
  function context(gl) {
    if(!contexts.has(gl))contexts.set(gl,{id:contexts.size,gl,drawFramebuffer:null});
    return contexts.get(gl);
  }
  function resource(gl,kind,handle) {
    const row=handle&&byHandle.get(handle);
    return row?.gl===gl&&row.kind===kind?row:undefined;
  }
  const proto=WebGL2RenderingContext.prototype;
  for(const kind of ['Texture','Framebuffer']) {
    const create=proto[`create${kind}`], destroy=proto[`delete${kind}`];
    proto[`create${kind}`]=function(...args) {
      const handle=Reflect.apply(create,this,args);
      if(handle) {
        const row={id:resources.length,context:context(this).id,kind,handle,gl:this,
          created:++sequence,deletes:0,draws:0};
        resources.push(row);byHandle.set(handle,row);
      }
      return handle;
    };
    proto[`delete${kind}`]=function(handle) {
      const result=Reflect.apply(destroy,this,[handle]);
      const row=resource(this,kind,handle);
      if(row){row.deletes++;row.deleted=++sequence;}
      return result;
    };
  }
  const allocate=proto.texImage2D;
  proto.texImage2D=function(...args) {
    const result=Reflect.apply(allocate,this,args);
    const row=resource(this,'Texture',this.getParameter(this.TEXTURE_BINDING_2D));
    // wasm-bindgen passes an absent optional pixel buffer as undefined.
    if(row&&args.length===9){row.size=[args[3],args[4]];row.emptyAllocation=args[8]==null;}
    return result;
  };
  const attach=proto.framebufferTexture2D;
  proto.framebufferTexture2D=function(...args) {
    const result=Reflect.apply(attach,this,args);
    if(args[1]===this.COLOR_ATTACHMENT0) {
      const buffer=resource(this,'Framebuffer',this.getParameter(this.FRAMEBUFFER_BINDING));
      const texture=resource(this,'Texture',args[3]);
      if(buffer&&texture){buffer.texture=texture.id;texture.framebuffer=buffer.id;}
    }
    return result;
  };
  const bind=proto.bindFramebuffer;
  proto.bindFramebuffer=function(target,handle) {
    const result=Reflect.apply(bind,this,[target,handle]);
    if(target===this.FRAMEBUFFER||target===this.DRAW_FRAMEBUFFER)context(this).drawFramebuffer=handle;
    return result;
  };
  for(const name of ['drawArrays','drawElements']) {
    const draw=proto[name];
    proto[name]=function(...args) {
      const result=Reflect.apply(draw,this,args);
      const buffer=resource(this,'Framebuffer',context(this).drawFramebuffer);
      if(buffer)buffer.draws++;
      return result;
    };
  }
  const documentId=crypto.randomUUID();
  window.secondaryResourceState=()=>({documentId,resources:resources.map(({gl,handle,...row})=>({
    ...row,live:gl[`is${row.kind}`](handle),canvas:gl.canvas.id||'(unnamed)',
  })),contexts:[...contexts.values()].map(({gl,id})=>{
    const extension=gl.getExtension('WEBGL_debug_renderer_info');
    return {id,canvas:gl.canvas.id,renderer:extension?gl.getParameter(extension.UNMASKED_RENDERER_WEBGL):gl.getParameter(gl.RENDERER)};
  })});
}
async function resources() { return page.evaluate(()=>window.secondaryResourceState()); }
function targets(state) {
  const textures=state.resources.filter(row=>row.kind==='Texture'&&row.emptyAllocation
    &&row.size?.[0]===256&&row.size?.[1]===256&&row.framebuffer!==undefined);
  return textures.map(texture=>({texture,framebuffer:state.resources.find(row=>row.id===texture.framebuffer)}));
}
function assertLive(targets,label) {
  assert.ok(targets.length>0,`${label}: real 256-square render targets exist`);
  for(const target of targets) {
    assert.equal(target.texture.live,true,`${label}: texture remains live`);
    assert.equal(target.framebuffer.live,true,`${label}: framebuffer remains live`);
    assert.equal(target.texture.deletes,0);assert.equal(target.framebuffer.deletes,0);
    assert.ok(target.framebuffer.draws>0,`${label}: actual renderer painted the target`);
  }
}
async function assertRetired(selected,label) {
  const state=await resources();
  assert.equal(state.documentId,observerDocument,`${label}: retirement happened in the same document`);
  const retired=selected.flatMap(row=>[row.texture.id,row.framebuffer.id])
    .map(id=>state.resources.find(row=>row.id===id));
  assert.ok(retired.every(row=>!row.live&&row.deletes===1),`${label}: all captured resources delete exactly once`);
  return retired;
}

async function consoleRun(code) {
  await page.keyboard.press('Backquote'); await ui.click(page,'cheats/tab:console');
  await ui.fill(page,'cheats/lua-editor',code);
  const before=receipt.commands.length;
  await ui.click(page,'cheats/run');
  await until(()=>receipt.commands.slice(before).some(row=>row.code===code),'the real Console request');
  const request=receipt.commands.slice(before).find(row=>row.code===code);
  await until(()=>receipt.results.some(row=>row.request_id===request.request_id),'the real Console result');
  const result=receipt.results.find(row=>row.request_id===request.request_id);
  assert.equal(result.ok,true,result.error||'Console accepted');
  await page.keyboard.press('Backquote');await ready();
  return {request,result};
}
async function requireSkillRanks(ranks) {
  const grants=[];
  for(const [id,rank] of ranks) {
    const current=player().book.permanent_ranks[id];
    assert.ok(current<=rank,`skill ${id} starts within the declared fixture rank`);
    if(current<rank)grants.push(`sd.dev.grant_skill(${id},${rank-current})`);
  }
  if(grants.length)await consoleRun(`${grants.join('; ')}; return true`);
  for(const [id,rank] of ranks)assert.equal(player().book.permanent_ranks[id],rank);
}
async function configureSkills() {
  await requireSkillRanks([[11,1],[27,1],[56,2]]);
  await consoleRun('return sd.player.restore_mana()');
  await page.keyboard.press('KeyK');await ui.widget(page,'panel:surface:skills');
  for(const [slot,id] of [[1,11],[2,27]]) {
    await ui.drag(page,`panel:skill:${id}`,`panel:belt:${slot}`);
    await until(()=>player().belt.entries[slot]?.Skill?.skill_id===id,'actual Skillbook drag binds the skill');
  }
  await page.keyboard.press('Escape');await ui.noWidget(page,'panel:surface:skills');await ready();
}
async function enterBoneyard(name) {
  await ui.click(page,(await ui.byLabel(page,'Enter the Boneyard',{role:'button'})).id);
  await until(()=>world(),'actual Boneyard admission');await ready();
  const target=path.join(output,name);fs.mkdirSync(target,{recursive:true});
  await walkSolomonEntrance(page,{snapshot:()=>frame.snapshot,playerId:()=>playerId,output:target});
  await ready();
  assert.ok(world().encounter.run_event_id>0);
  return world().run_id;
}
async function castBoth(name) {
  assert.equal(spellActors().length,0,'the fresh world starts without either spell');
  const before=await resources();
  observerDocument=before.documentId;
  const oldIds=new Set(before.resources.map(row=>row.id));
  const casts=[];
  // The defaults bind belt slots 1 and 2 to these keyboard keys, preserving world aim.
  for(const [key,kind] of [['Digit1','Leviathan'],['Digit2','StormCloud']]) {
    await until(()=>!secondary()||secondary().global_cooldown_ticks<=0,'real secondary cooldown completes');
    const previousIds=new Set((await resources()).resources.map(row=>row.id));
    await page.mouse.move(1200,350);await page.keyboard.press(key);
    await until(()=>spellActors().some(row=>row.kind===kind),`real belt cast creates ${kind}`);
    const actor=spellActors().find(row=>row.kind===kind);
    await until(async()=>targets(await resources()).some(row=>!previousIds.has(row.texture.id)
      &&row.texture.live&&row.framebuffer.draws>0),`${kind} paints its live target`);
    const created=targets(await resources()).filter(row=>!previousIds.has(row.texture.id));
    assert.equal(created.length,1,`${kind} allocates one target before any hit response`);
    casts.push({key,kind,actor:structuredClone(actor),tick:frame.snapshot.ticks,targets:created});
  }
  const state=await resources(), selected=targets(state).filter(row=>!oldIds.has(row.texture.id));
  assert.equal(selected.length,2,'ordinary Leviathan and stationary Storm each allocate one target');
  assertLive(selected,name);await shot(name);
  return {runId:world().run_id,casts,targets:selected,contexts:state.contexts};
}
async function pause() {
  await page.keyboard.press('Escape');await ui.surface(page,'pause');
  await until(()=>frame.host.pause,'actual host pause freezes active spells');
}
async function leave(name,selected) {
  const before=receipt.leaveReceipts.length;
  await ui.click(page,'leave');await ui.surface(page,'title');
  await until(()=>receipt.leaveReceipts.length>before,'the host acknowledged saved Leave');
  const acknowledgement=receipt.leaveReceipts.at(-1);
  const checkpoint=receipt.saves.findLast(row=>row.sequence===acknowledgement.checkpoint_sequence);
  assert.ok(checkpoint,'saved Leave references a completely received host checkpoint');
  await until(()=>receipt.saveUploads.some(row=>row.status===200&&row.sha256===checkpoint.sha256),
    'cloud upload matches the exact acknowledged Leave checkpoint');
  const upload=receipt.saveUploads.find(row=>row.status===200&&row.sha256===checkpoint.sha256);
  const retired=selected?await assertRetired(selected,name):[];
  await shot(name);
  return {acknowledgement,checkpoint,upload,retired};
}

const golem = () => frame?.snapshot.secondary.actors.find(row=>row.owner_id===playerId&&row.kind==='Golem');
function observeRendering() {
  const evidence=receipt.rendering, tick=frame.snapshot.ticks, summon=golem();
  if(summon&&!evidence.golemPhases.some(row=>row.actor.golem.phase===summon.golem.phase))
    evidence.golemPhases.push({tick,actor:structuredClone(summon)});
  for(const projectile of frame.snapshot.spells.projectiles) {
    if(projectile.Fire?.common.owner_id===playerId&&evidence.fireProjectiles.length<30)
      evidence.fireProjectiles.push({tick,projectile:structuredClone(projectile.Fire)});
  }
  for(const transient of frame.snapshot.spells.transients) {
    const [kind,effect]=Object.entries(transient)[0];
    if(['FireImpact','FireExplosion'].includes(kind)&&effect.common.owner_id===playerId
      &&evidence.fireContacts.length<30)evidence.fireContacts.push({tick,kind,effect:structuredClone(effect)});
  }
}
async function renderShot(name) {
  const uiState=await ui.readUi(page), state=await resources();
  receipt.rendering.samples.push({name,viewport:page.viewportSize(),tick:frame.snapshot.ticks,
    wizard:structuredClone(actor().wizard),golem:structuredClone(golem()),
    overlays:uiState.worldOverlays,draw:uiState.draw,contexts:state.contexts});
  assert.ok(state.contexts.some(row=>row.canvas==='game'&&/Metal/.test(row.renderer)),
    'the composed rendering journey uses the actual Mac Metal backend');
  await shot(name);
}
async function ownSpeech(name) {
  // Boneyard chat in this private solo run delivers only to this account.
  // The local nameplate is intentionally absent; this exercises the actual world font.
  assert.deepEqual(frame.host.connected_player_ids,[playerId]);
  assert.deepEqual(social.party_state.party.member_player_ids,[playerId]);
  await ui.click(page,'social/chat-open');await ui.click(page,'social/channel:boneyard');
  const text=`Golem fire . i ${name}`;
  await ui.fill(page,'social/chat-message',text);await ui.click(page,'social/chat-send');
  await until(()=>receipt.chats.some(row=>row.text===text&&row.channel==='Boneyard'&&row.sender===playerId),
    'the private Boneyard delivers the ordinary chat message');
  const message=receipt.chats.find(row=>row.text===text);
  // Sending a valid message closes the composer in the normal social UI.
  await ui.noWidget(page,'social/chat-close');
  await ui.waitForUi(page,state=>state.worldOverlays.speech_sequences.includes(message.sequence)
    &&state.worldOverlays.speech_player_ids.includes(playerId)
    &&state.worldOverlays.speech_alphas.some(alpha=>alpha>0.9),'actual world speech draws at full opacity');
  await renderShot(name);
}
async function renderingJourney() {
  step='Golem and Fire prerequisite setup';
  await requireSkillRanks([[45,3],[56,2],[64,4]]);
  await consoleRun('sd.player.restore_mana(); sd.player.restore_health(1000); return true');
  await page.keyboard.press('KeyK');await ui.widget(page,'panel:surface:skills');
  await ui.drag(page,'panel:skill:45','panel:belt:1');
  await until(()=>player().belt.entries[1]?.Skill?.skill_id===45,'the Skillbook binds Raise Golem');
  await shot('rendering-skillbook');await page.keyboard.press('Escape');await ready();
  await enterBoneyard('rendering-entrance');
  const runId=world().run_id, plan=await prepareBoneyardNavigation(world());
  try {
    step='ordinary Fire projectile and enemy contact';
    const at=actor().wizard.position, bounds=world().arena_transition.combat_bounds;
    const x=at.x+(at.x<bounds.x+bounds.width/2?160:-160);
    const before=new Set(world().enemies.actors.map(row=>row.id));
    await consoleRun(`return sd.enemies.spawn("skeleton", {x=${x},y=${at.y}})`);
    const target=world().enemies.actors.find(row=>!before.has(row.id));
    assert.ok(target,'the declared enemy prerequisite is present');
    const targetId=target.id, health=target.current_health, startTick=frame.snapshot.ticks;
    const screen=await plan.screenPoint(target.position,actor().wizard.position,
      world().arena_transition.camera_bounds,{x:1600,y:900});
    await page.mouse.move(screen.x,screen.y);await page.mouse.down();
    try {
      await until(()=>receipt.rendering.fireProjectiles.length>0,'ordinary input creates the Fire projectile');
      await renderShot('rendering-fire-flight');
      await until(()=>receipt.rendering.fireContacts.length>0
        &&world().enemies.actors.some(row=>row.id===targetId&&row.current_health<health
          &&row.last_damage_tick>=startTick&&row.last_damaged_by_player_id===playerId),
      'the Fire primary contacts and damages the declared enemy');
      receipt.rendering.fireTarget=structuredClone(world().enemies.actors.find(row=>row.id===targetId));
      await renderShot('rendering-fire-contact');
    } finally {await page.mouse.up();}
    receipt.checks.push('Ordinary Fire input produces projectiles, contact effects and actual enemy damage before Golem exists');

    step='ordinary Golem assembly and active rendering';
    await page.mouse.move(1100,400);await page.keyboard.press('Digit1');
    await until(()=>golem()?.golem.phase==='Assembly','ordinary belt cast starts Golem assembly');
    await renderShot('rendering-golem-assembly');
    await until(()=>golem()?.golem.phase==='Active','the actual Golem completes assembly');
    const golemId=golem().id;
    await renderShot('rendering-golem-active');
    const position=golem().position;
    await consoleRun(`return sd.enemies.spawn("skeleton", {x=${position.x}+55,y=${position.y}})`);
    await until(()=>golem()?.golem.phase==='Attack','the actual Golem attacks a nearby enemy');
    await renderShot('rendering-golem-attack');
    assert.ok(['Assembly','Active','Attack'].every(phase=>receipt.rendering.golemPhases
      .some(row=>row.actor.golem.phase===phase)));
    receipt.checks.push('The ordinary Raise Golem belt cast renders assembly, active and attack phases with the player in the same Boneyard');

    step='world speech and resize';
    await until(()=>social?.party_state,'the actual private Party exists');
    await ownSpeech('rendering-world-text-1600');
    for(const viewport of [{width:1280,height:720},{width:1920,height:1080}]) {
      await page.setViewportSize(viewport);await ui.nextFrame(page);await ready();
      await ownSpeech(`rendering-world-text-${viewport.width}`);
    }
    await page.setViewportSize({width:1600,height:900});await ui.nextFrame(page);await ready();
    receipt.checks.push('Self-only Boneyard speech draws the world font with player and Golem at three viewport sizes');

    step='rendering saved Leave and fresh Continue';
    await pause();
    assert.equal(golem()?.id,golemId,'the same Golem remains active before saved Leave');
    const savedPlayer=playerId, oldDocument=(await resources()).documentId;
    receipt.rendering.savedGolem=structuredClone(golem());
    receipt.rendering.leave=await leave('rendering-saved-leave');
    frame=null;playerId=null;
    await ui.observeDocumentReplacement(page,()=>page.reload({waitUntil:'domcontentloaded'}));await ui.ready(page);
    await ui.settleErrors(page);
    assert.notEqual((await resources()).documentId,oldDocument);
    await ui.click(page,'play');await ui.click(page,'continue');
    await until(()=>world()?.run_id===runId,'fresh Continue restores the same saved Boneyard',120000);await ready();
    assert.equal(playerId,savedPlayer);assert.equal(golem()?.id,golemId);
    assert.equal(player().selection.element,'Fire');
    await renderShot('rendering-fresh-continue');
    await pause();receipt.rendering.finalLeave=await leave('rendering-final-leave');
    receipt.checks.push('Exact acknowledged checkpoint bytes upload successfully; fresh-document Continue restores the same player, run and Golem');
  } finally {receipt.rendering.navigationExitCode=await plan.close();}
}

(async()=>{
  fs.mkdirSync(output,{recursive:true});
  const owner=await launchOwnedBrowser(chromium,{headless:true,executablePath:process.env.CHROME,
    args:['--use-angle=metal','--ignore-gpu-blocklist']});
  let failure;
  try {
    const context=await owner.browser.newContext({viewport:{width:1600,height:900},ignoreHTTPSErrors:true});
    await context.addInitScript(token=>localStorage.setItem('sdr.token:/game-stg',token),account.token);
    await context.addInitScript(observeResources);
    page=await context.newPage();observed=ui.watchErrors(page);await observed.ready;observeWire();
    await page.goto(url);await ui.ready(page);
    const me=await context.request.get(new URL('api/auth/me',url).href,{headers:{Authorization:`Bearer ${account.token}`}});
    assert.equal(me.status(),200);assert.equal((await me.json()).user.developerAccess,true);
    if(ui.findWidget(await ui.readUi(page),'prompt-secondary'))await ui.click(page,'prompt-secondary');
    await ui.click(page,'settings');await ui.click(page,'settings-page:cloud');
    for(const key of ['shared-hub','cheats']) {
      if(key==='cheats')await revealSettingsControl(page,`settings-toggle:${key}`);
      if((await ui.widget(page,`settings-toggle:${key}`)).selected)await ui.click(page,`settings-toggle:${key}`);
      await ui.value(page,`settings-toggle:${key}`,'false');
    }
    await ui.click(page,'settings-back');await ui.click(page,'settings-back');await ui.surface(page,'title');
    await startNewGame(page,{name:journey==='rendering'?'Rendering':'Lifecycle',
      element:journey==='rendering'?'fire':'ether',discipline:'arcane',continueLocal:true});await ready();
    assert.equal(receipt.welcomes.at(-1).sessionKind,'PrivateCollege');
    if(journey==='rendering')await renderingJourney();
    else {
      await configureSkills();

      step='active spells through enemy-caused Game Over';
      await enterBoneyard('first-entrance');
      const first=await castBoth('first-world-active-spells');receipt.worlds.push(first);
      const at=actor().wizard.position;
      first.enemies=await consoleRun(`for i=1,8 do sd.enemies.spawn("skeleton", {x=${at.x}+i*3,y=${at.y}}) end return true`);
      await until(()=>frame.snapshot.run.phase==='GameOver','enemy damage ends the actual run',45000);
      first.gameOver={actors:structuredClone(spellActors()),combat:structuredClone(player().combat),
        targets:targets(await resources()).filter(row=>first.targets.some(before=>before.texture.id===row.texture.id))};
      assert.deepEqual(first.gameOver.actors.map(row=>row.kind).sort(),['Leviathan','StormCloud']);
      assertLive(first.gameOver.targets,'Game Over retains the active world until Continue');
      await shot('game-over-with-retained-targets');
      await ui.widget(page,'game-over:continue',{enabled:true});
      await ui.click(page,'game-over:continue');await ui.surface(page,'create');
      first.retiredAtCreate=await assertRetired(first.targets,'ordinary Game Over Continue');
      await shot('post-game-create');
      await ui.click(page,'element:ether');await ui.click(page,'discipline:arcane');
      await until(()=>!world()&&frame.snapshot.run.phase==='Hub','ordinary post-run College');await ready();
      assert.equal((await resources()).documentId,observerDocument);
      first.retiredAtCollege=await assertRetired(first.targets,'ordinary College return');
      await shot('college-after-world-retirement');
      receipt.checks.push('Both active target families survive Game Over and delete exactly once at ordinary same-document world replacement');

      step='new-world cast and saved Leave';
      await configureSkills();
      await enterBoneyard('second-entrance');
      assert.notEqual(world().run_id,first.runId);
      const second=await castBoth('second-world-active-spells');receipt.worlds.push(second);
      assert.ok(second.targets.every(row=>first.targets.every(previous=>row.texture.id!==previous.texture.id)),
        'the next world allocates fresh actual GPU handles');
      await assertRetired(first.targets,'new world never reuses deleted old handles');
      await pause();
      second.savedActors=structuredClone(spellActors());
      assert.equal(second.savedActors.length,2);assertLive(targets(await resources())
        .filter(row=>second.targets.some(before=>before.texture.id===row.texture.id)),'paused active spell save');
      second.leave=await leave('saved-leave-active-spells',second.targets);
      const savedRun=second.runId,savedPlayer=playerId,oldDocument=observerDocument;
      frame=null;playerId=null;
      await ui.observeDocumentReplacement(page,()=>page.reload({waitUntil:'domcontentloaded'}));await ui.ready(page);
      await ui.settleErrors(page);
      assert.notEqual((await resources()).documentId,oldDocument,'Continue occurs in a fresh browser document');
      observerDocument=(await resources()).documentId;
      const beforeContinue=new Set((await resources()).resources.map(row=>row.id));
      await ui.click(page,'play');await ui.click(page,'continue');
      await until(()=>world()?.run_id===savedRun,'ordinary signed saved Continue restores the actual run',120000);await ready();
      assert.equal(playerId,savedPlayer);
      await pause();
      const restored=spellActors();
      assert.deepEqual(restored.map(row=>[row.id,row.kind]),second.savedActors.map(row=>[row.id,row.kind]),
        'the checkpoint restores both actual active spell identities');
      await until(async()=>targets(await resources()).filter(row=>row.texture.live
        &&!beforeContinue.has(row.texture.id)).length>=2,
        'fresh Continue reconstructs both active targets');
      const continuedTargets=targets(await resources()).filter(row=>row.texture.live
        &&!beforeContinue.has(row.texture.id));
      assert.equal(continuedTargets.length,2,'restored Leviathan and Storm allocate their own targets');
      assertLive(continuedTargets,'saved Continue');
      second.continued={actors:structuredClone(restored),targets:continuedTargets,documentId:observerDocument};
      await shot('fresh-continue-active-spells');
      second.finalLeave=await leave('final-saved-leave',continuedTargets);
      receipt.checks.push('A new world uses fresh resources; active spells survive saved Leave and fresh-document Continue, then retire again');
    }
    await ui.assertGpuOnly(page);await ui.settleErrors(page);assert.deepEqual(observed.errors,[]);assert.deepEqual(receipt.errors,[]);
    receipt.passed=true;
  } catch(error) {
    failure=error;receipt.failure=error.stack;receipt.step=step;
    receipt.failedPlayer=player();receipt.failedActors=spellActors();receipt.failedHost=frame?.host;
    receipt.failedRun=frame?.snapshot.run;
    if(page){receipt.failedUi=await ui.readUi(page).catch(error=>({error:error.message}));
      receipt.failedResources=await resources().catch(error=>({error:error.message}));await shot('failure').catch(()=>{});}
  } finally {
    if(page)try{await ui.settleErrors(page);assert.deepEqual(observed.errors,[]);assert.deepEqual(receipt.errors,[]);}
    catch(error){failure||=error;receipt.passed=false;receipt.observerFailure=error.stack;}
    receipt.browserErrors=observed?.errors||[];
    try{receipt.cleanup=await owner.close();}catch(error){failure||=error;receipt.passed=false;receipt.cleanup=error.cleanup;}
    fs.writeFileSync(path.join(output,'receipt.json'),JSON.stringify(receipt,null,2));
  }
  console.log(JSON.stringify({passed:receipt.passed,step,checks:receipt.checks,cleanup:receipt.cleanup}));
  if(failure)throw failure;
})().catch(error=>{console.error(error);process.exitCode=1;});
