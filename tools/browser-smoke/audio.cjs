// Exercise real decoded audio, trusted resume and settings through the shipped browser UI.
// Native APIs are observed without replacing their buffers, graph or playback promises.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {chromium} = require(path.resolve(process.env.PW));
const {reachCollegeLoadout} = require('./menu-journey.cjs');
const tree = path.resolve(__dirname,'../..');
const output = process.env.OUT || '/tmp';
const catalog = fs.readFileSync(path.join(tree,'crates/sim/src/audio/catalog.rs'),'utf8');
const files = {};
for(const match of catalog.matchAll(/id: "([^"]+)",\s*path: "([^"]+)"/g)) {
  const length=fs.statSync(path.join(tree,'web/assets/stock/audio',match[2])).size;
  (files[length] ||= []).push(match[1]);
}
(async()=>{
const browser = await chromium.launch({headless:true,executablePath:process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',args:['--autoplay-policy=document-user-activation-required','--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
const errors=[];
try {
const page=await browser.newPage({viewport:{width:1600,height:900}});
page.on('pageerror',error=>errors.push(error.message));
page.on('console',message=>{if(message.type()==='error')errors.push(message.text())});
page.on('response',response=>{if(response.status()>=400)errors.push(`${response.status()} ${response.url()}`)});
await page.addInitScript(({files})=>{
  const contexts=[],events=[],gains=[],metadata=new WeakMap(),connections=new WeakMap();
  window.audioReceipt={contexts,events,gains};
  const Context=window.AudioContext;
  window.AudioContext=new Proxy(Context,{construct(target,args){const context=Reflect.construct(target,args);contexts.push(context);return context}});
  const decode=Context.prototype.decodeAudioData;
  Context.prototype.decodeAudioData=function(encoded,...args){
    const cues=files[encoded.byteLength]||[];
    return decode.call(this,encoded,...args).then(buffer=>{metadata.set(buffer,cues);return buffer});
  };
  const gain=Context.prototype.createGain;
  Context.prototype.createGain=function(){const node=gain.call(this);gains.push(node);return node};
  const connect=AudioNode.prototype.connect;
  AudioNode.prototype.connect=function(destination,...args){connections.set(this,destination);return connect.call(this,destination,...args)};
  const start=AudioBufferSourceNode.prototype.start;
  AudioBufferSourceNode.prototype.start=function(...args){
    const result=start.call(this,...args);
    events.push({action:'start',cues:metadata.get(this.buffer),rate:this.playbackRate.value,gain:connections.get(this)?.gain?.value,loop:this.loop,when:args[0],offset:args[1],at:this.context.currentTime});
    return result;
  };
  const stop=AudioScheduledSourceNode.prototype.stop;
  AudioScheduledSourceNode.prototype.stop=function(...args){events.push({action:'stop',cues:metadata.get(this.buffer),at:this.context.currentTime});return stop.call(this,...args)};
},{files});
await page.goto(process.env.URL || 'http://127.0.0.1:4185/');
await page.waitForFunction(()=>window.wasmBindings?.wizard_state().length>0&&!document.querySelector('#status').textContent,null,{timeout:60000});
assert.equal(await page.evaluate(()=>audioReceipt.contexts.length),1);
const musicBeforeActivation=()=>page.locator('audio').evaluateAll(nodes=>nodes.map(node=>({paused:node.paused})));
const initialState=await page.evaluate(()=>audioReceipt.contexts[0].state);
await page.evaluate(()=>audioReceipt.contexts[0].suspend());
assert.equal(await page.evaluate(()=>audioReceipt.contexts[0].state),'suspended');
assert.ok((await musicBeforeActivation()).every(track=>track.paused));
const tutorialPrompt=page.locator('.title-prompt:not([hidden])');
if(await tutorialPrompt.isVisible()) {
  await tutorialPrompt.locator('[data-menu-action="prompt-secondary"]').click();
}
await page.locator('[data-menu-action="play"]').click();
await page.waitForFunction(()=>audioReceipt.contexts[0].state==='running');
await page.waitForTimeout(1100);
const music=()=>page.locator('audio').evaluateAll(nodes=>nodes.map(node=>({cue:node.dataset.audioCue,volume:node.volume,paused:node.paused,time:node.currentTime,ready:node.readyState})));
let tracks=await music();
assert.ok(tracks.find(track=>track.cue==='solomondarktheme').time>0);
assert.equal(tracks.filter(track=>!track.paused).length,1);
await page.locator('[data-menu-action="new-game"]').click();
await reachCollegeLoadout(page);
await page.waitForFunction(()=>audioReceipt.events.some(event=>event.action==='start'&&event.cues?.includes('choose-element')),null,{timeout:10000});
let events=await page.evaluate(()=>audioReceipt.events);
assert.ok(events.some(event=>event.action==='start'&&event.cues.includes('start-cast')));
assert.ok(events.some(event=>event.action==='start'&&event.cues.includes('choose-element')));
assert.ok(events.some(event=>event.action==='stop'&&event.cues.includes('start-cast')));
await page.getByRole('textbox',{name:'Wizard name',exact:true}).fill('Felix');
await page.locator('[data-menu-action="element:ether"]').click();
await page.locator('[data-menu-action="discipline:arcane"]').waitFor({state:'visible'});
await page.locator('[data-menu-action="discipline:arcane"]').click();
await page.waitForFunction(()=>document.querySelector('.game-menus').dataset.menuScreen==='hidden');
const hubEntryBeforeReady=await page.evaluate(()=>Array.from(window.wasmBindings.wizard_state()));
// Final Create includes the stock incoming walk; inventory commands are rejected
// during that transition, just as the shared startNewGame helper expects.
await page.waitForFunction(()=>window.wasmBindings.wizard_state()[6]===0
  && window.wasmBindings.wizard_state()[7]===-1);
const hubMusicBeforeSettlement=await music();
// The scene change schedules a fade. Its final media state needs a presentation
// update, which can arrive after a fixed wall-time delay on a busy machine.
await page.waitForFunction(()=>{
  const active=[...document.querySelectorAll('audio')].filter(track=>!track.paused);
  return active.length===1 && active[0].dataset.audioCue==='academy' && active[0].currentTime>0;
},null,{timeout:10000});
tracks=await music();
assert.ok(tracks.find(track=>track.cue==='academy').time>0);
assert.equal(tracks.filter(track=>!track.paused).length,1);
await page.keyboard.press('KeyI'); await page.waitForTimeout(450);
await page.getByRole('button',{name:'Health Potion, quantity 1',exact:true}).dblclick();
await page.waitForFunction(()=>audioReceipt.events.some(event=>event.action==='start'&&event.cues?.includes('drink')));
assert.ok(await page.evaluate(()=>audioReceipt.events.some(event=>event.action==='start'&&event.cues.includes('drink'))));
await page.keyboard.press('Escape');
await page.locator('.player-panels').waitFor({state:'hidden'});
await page.keyboard.press('Escape');
await page.locator('.game-menus[data-menu-screen="pause"]').waitFor();
await page.locator('[data-menu-action="pause-settings"]').click();await page.waitForTimeout(450);


await page.locator('[data-setting="music"]').fill('0');
await page.locator('[data-setting="sound"]').fill('0');
await page.waitForFunction(()=>[...document.querySelectorAll('audio')].every(track=>track.volume===0)&&audioReceipt.gains[0].gain.value===0);
assert.ok((await music()).every(track=>track.volume===0));
assert.equal(await page.evaluate(()=>audioReceipt.gains[0].gain.value),0);
await page.locator('[data-setting="sound"]').fill('73');
await page.locator('[data-setting="music"]').fill('41');
await page.waitForFunction(()=>Math.abs(audioReceipt.gains[0].gain.value-.73)<1e-6&&Math.abs(document.querySelector('audio[data-audio-cue="academy"]').volume-.41)<1e-6);
assert.ok(Math.abs(await page.evaluate(()=>audioReceipt.gains[0].gain.value)-.73)<1e-6);
assert.ok(Math.abs((await music()).find(track=>track.cue==='academy').volume-.41)<1e-6);
assert.deepEqual(errors,[]);
const receipt={passed:true,initialState,errors,hubEntryBeforeReady,hubMusicBeforeSettlement,tracks:await music(),events:await page.evaluate(()=>audioReceipt.events),loadedAudioResources:await page.evaluate(()=>performance.getEntriesByType('resource').filter(entry=>entry.name.includes('/audio/')).length)};
fs.writeFileSync(path.join(output,'audio-browser-receipt.json'),JSON.stringify(receipt,null,2));
await page.screenshot({path:path.join(output,'audio-settings-rust.png')});
console.log(JSON.stringify({passed:true,errors,events:receipt.events.length,loadedAudioResources:receipt.loadedAudioResources}));
}finally{await browser.close()}
})().catch(error=>{console.error(error);process.exitCode=1});
