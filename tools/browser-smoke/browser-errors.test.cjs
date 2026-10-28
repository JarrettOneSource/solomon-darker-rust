const test=require('node:test');
const assert=require('node:assert/strict');
const {EventEmitter}=require('node:events');
const {residentSuspension,documentRetirement,watchErrors,settleErrors}=require('./browser-errors.cjs');

test('a completed phase keeps its errors while later failures remain in the live observer',async()=>{
  const page=new EventEmitter();
  page.addInitScript=async()=>{};
  page.frames=()=>[];
  page.mainFrame=()=>null;
  page.url=()=> 'http://127.0.0.1:4181/';
  const observed=watchErrors(page);
  page.emit('requestfailed',{
    url:()=> 'http://127.0.0.1:4181/missing.png',method:()=> 'GET',
    resourceType:()=> 'image',failure:()=>({errorText:'net::ERR_CONNECTION_RESET'}),
  });
  const first=await settleErrors(page);
  assert.equal(first.errors.length,1);
  assert.equal(first.rawFailures.length,1);

  page.emit('pageerror',new Error('a later phase failed'));
  page.emit('requestfailed',{
    url:()=> 'http://127.0.0.1:4181/later.png',method:()=> 'GET',
    resourceType:()=> 'image',failure:()=>({errorText:'net::ERR_CONNECTION_RESET'}),
  });
  const second=await settleErrors(page);
  assert.equal(first.errors.length,1);
  assert.equal(first.rawFailures.length,1);
  assert.equal(second.errors.length,3);
  assert.equal(second.rawFailures.length,2);
  assert.deepEqual(second.errors,observed.errors);
  assert.deepEqual(second.rawFailures,observed.rawFailures);
});

function evidence() {
  const url='http://127.0.0.1:4181/assets/stock/audio/music/death.mp3';
  const channel={id:3,cue:'death',src:url,url,connected:true,hidden:true,controls:false,
    preload:'auto',loads:1,sources:[url],ready:4,network:1,error:null,retired:false,
    events:[{type:'loadstart',trusted:true},{type:'loadeddata',trusted:true},
      {type:'suspend',trusted:true,network:1}]};
  const captured={documentId:123,documentUrl:'http://127.0.0.1:4181/',channels:[channel]};
  return {failure:{url,method:'GET',resourceType:'media',status:200,errorText:'net::ERR_ABORTED'},
    captured,settled:structuredClone(captured)};
}

test('a ready resident channel with directly witnessed browser suspension owns its cancelled transfer',()=>{
  const {failure,captured,settled}=evidence();
  assert.deepEqual(residentSuspension(failure,captured,settled),{cue:'death',channelId:3,
    documentId:123,reason:'ready resident channel suspended by the browser'});
});

function resumedSuspension() {
  const fixture=evidence();
  const channel=fixture.settled.channels[0];
  // Windows Chrome resumed fetching after trusted-gesture music priming. The
  // cancelled preload was already loaded and suspended before this playback.
  Object.assign(channel,{ready:3,network:2,paused:true,seeking:false});
  channel.events.push(
    {type:'playing',trusted:true,url:channel.url,ready:3,paused:false,seeking:false},
    {type:'seeking',trusted:true,paused:true,seeking:true},
    {type:'seeked',trusted:true,paused:true,seeking:false});
  return fixture;
}

test('a suspended preload remains owned when its resident channel later plays and resumes fetching',()=>{
  const {failure,captured,settled}=resumedSuspension();
  assert.deepEqual(residentSuspension(failure,captured,settled),{cue:'death',channelId:3,
    documentId:123,reason:'ready resident channel resumed after browser suspension'});
});

function capturedRewind() {
  const fixture=evidence();
  const before=fixture.captured.channels[0];
  // Windows Chrome captured the priming rewind before delivering its seeking
  // event. The retained preload and later native events establish its lifetime.
  Object.assign(before,{ready:1,network:1,paused:false,currentTime:0,seeking:true});
  Object.assign(before.events[1],{ready:4});
  const after=structuredClone(before);
  Object.assign(after,{ready:4,paused:true,seeking:false});
  after.events.push(
    {type:'seeking',trusted:true,url:after.url,ready:1,paused:false,seeking:true,currentTime:0},
    {type:'seeked',trusted:true,url:after.url,ready:3,paused:false,seeking:false},
    {type:'playing',trusted:true,url:after.url,ready:3,paused:false,seeking:false},
    {type:'seeking',trusted:true,url:after.url,ready:1,paused:true,seeking:true,currentTime:0},
    {type:'seeked',trusted:true,url:after.url,ready:4,paused:true,seeking:false});
  fixture.settled.channels=[after];
  return fixture;
}

test('a loaded suspended channel can complete its captured priming rewind and play',()=>{
  const {failure,captured,settled}=capturedRewind();
  assert.deepEqual(residentSuspension(failure,captured,settled),{cue:'death',channelId:3,
    documentId:123,reason:'resident channel completed a captured rewind after browser suspension'});
});

test('a ready channel with a completed rewind can later play after its cancelled preload',()=>{
  const {failure,captured,settled}=capturedRewind();
  const before=captured.channels[0],after=settled.channels[0];
  after.events.splice(6);
  after.events[3].paused=true;
  after.events[4].paused=true;
  before.events=structuredClone(after.events.slice(0,5));
  Object.assign(before,{ready:3,network:2,paused:true,seeking:false});
  Object.assign(after,{paused:false,currentTime:1.7});
  assert.deepEqual(residentSuspension(failure,captured,settled),{cue:'death',channelId:3,
    documentId:123,reason:'resident channel completed a captured rewind after browser suspension'});
});

for(const [name,change] of [
  ['no earlier loaded data',f=>{f.captured.channels[0].events[1].ready=1}],
  ['no earlier suspension',f=>{f.captured.channels[0].events[2].trusted=false}],
  ['a different captured seek',f=>{f.captured.channels[0].currentTime=12}],
  ['no captured rewind',f=>{f.captured.channels[0].seeking=false}],
  ['an unfinished seek',f=>{f.settled.channels[0].seeking=true}],
  ['no trusted rewind start',f=>{f.settled.channels[0].events[3].trusted=false}],
  ['a rewind to another position',f=>{f.settled.channels[0].events[3].currentTime=12}],
  ['no trusted seek completion',f=>{f.settled.channels[0].events[4].trusted=false}],
  ['seek completion of another source',f=>{f.settled.channels[0].events[4].url+='?other'}],
  ['playback preceding seek completion',f=>{
    const events=f.settled.channels[0].events;[events[4],events[5]]=[events[5],events[4]];
  }],
  ['no trusted playback',f=>{f.settled.channels[0].events[5].trusted=false}],
  ['playback of another source',f=>{f.settled.channels[0].events[5].url+='?other'}],
  ['lost future data',f=>{f.settled.channels[0].ready=2}],
  ['an empty network state',f=>{f.settled.channels[0].network=0}],
  ['a later media error',f=>{f.settled.channels[0].error=3}],
  ['a media abort',f=>{f.settled.channels[0].events.push({type:'abort',trusted:true})}],
  ['a replaced source',f=>{f.settled.channels[0].sources.push('https://example.com/other.mp3')}],
  ['a replaced owner',f=>{f.settled.channels[0].id=4}],
  ['a replaced document',f=>{f.settled.documentId=124}],
  ['an actual network failure',f=>{f.failure.errorText='net::ERR_CONNECTION_RESET'}],
])test(`a captured rewind does not excuse ${name}`,()=>{
  const fixture=capturedRewind();change(fixture);
  assert.equal(residentSuspension(fixture.failure,fixture.captured,fixture.settled),null);
});

for(const [name,change] of [
  ['no later playback',f=>{f.settled.channels[0].events.splice(3,1)}],
  ['playback only before capture',f=>{
    f.captured=structuredClone(f.settled);f.captured.channels[0].network=1;
  }],
  ['a fabricated playing event',f=>{f.settled.channels[0].events[3].trusted=false}],
  ['playback of another resource',f=>{f.settled.channels[0].events[3].url+='?other'}],
  ['an unfinished seek',f=>{f.settled.channels[0].seeking=true}],
  ['lost future data',f=>{f.settled.channels[0].ready=2}],
  ['a later media error',f=>{f.settled.channels[0].error=3}],
  ['a changed source',f=>{f.settled.channels[0].sources.push('https://example.com/other.mp3')}],
])test(`resumed fetching does not excuse ${name}`,()=>{
  const fixture=resumedSuspension();change(fixture);
  assert.equal(residentSuspension(fixture.failure,fixture.captured,fixture.settled),null);
});

test('URL and ERR_ABORTED alone never classify a failed request',()=>{
  const {failure}=evidence();
  assert.equal(residentSuspension(failure,null,null),null);
});

test('a mounted editor document resolves stock music from its actual baseURI',()=>{
  const fixture=evidence();
  for(const snapshot of [fixture.captured,fixture.settled]) {
    snapshot.documentUrl='https://example.com/game-stg/editor-test/';
    snapshot.baseUrl='https://example.com/game-stg/';
    const channel=snapshot.channels[0];
    channel.url=channel.src='https://example.com/game-stg/assets/stock/audio/music/death.mp3';
    channel.sources=[channel.url];
  }
  fixture.failure.url=fixture.captured.channels[0].url;
  assert.equal(residentSuspension(fixture.failure,fixture.captured,fixture.settled).cue,'death');
  assert.equal(fixture.captured.documentUrl,'https://example.com/game-stg/editor-test/');
});

for(const [name,change] of [
  ['a real network failure',(f)=>{f.failure.errorText='net::ERR_CONNECTION_RESET'}],
  ['an image request',(f)=>{f.failure.resourceType='image'}],
  ['an HTTP failure',(f)=>{f.failure.status=503}],
  ['a different resource',(f)=>{f.failure.url=f.failure.url.replace('death.mp3','other.mp3')}],
  ['missing loadeddata',(f)=>{f.captured.channels[0].events.shift();f.captured.channels[0].events.shift()}],
  ['no native suspend',(f)=>{f.captured.channels[0].events.pop()}],
  ['a fabricated suspend',(f)=>{f.captured.channels[0].events[2].trusted=false}],
  ['a channel still loading',(f)=>{f.settled.channels[0].network=2}],
  ['readiness lost',(f)=>{f.settled.channels[0].ready=1}],
  ['a media error after the network callback',(f)=>{f.settled.channels[0].error=2}],
  ['a media abort event',(f)=>{f.settled.channels[0].events.push({type:'abort',trusted:true})}],
  ['a repeated load',(f)=>{f.settled.channels[0].loads=2}],
  ['a changed source',(f)=>{f.settled.channels[0].sources.push('https://example.com/other.mp3')}],
  ['a reused owner',(f)=>{f.settled.channels[0].id=4}],
  ['a removed owner',(f)=>{f.settled.channels[0].connected=false}],
  ['a retired and reattached owner',(f)=>{f.settled.channels[0].retired=true}],
  ['a replaced document',(f)=>{f.settled.documentId=124}],
  ['multiple owners for the failed resource',(f)=>{f.captured.channels.push({...f.captured.channels[0],id:4})}],
  ['a second owner added before classification',(f)=>{f.settled.channels.push({...f.settled.channels[0],id:4})}],
])test(`${name} remains fatal`,()=>{
  const fixture=evidence();change(fixture);
  assert.equal(residentSuspension(fixture.failure,fixture.captured,fixture.settled),null);
});

test('an exact pending request retired by a proved replacement keeps its old channel evidence',()=>{
  const {failure,captured}=evidence();
  const retirement={pendingRequest:true,completed:true,before:captured,
    beforeTargetId:123,afterTargetId:124,afterUrl:'http://127.0.0.1:4181/editor/'};
  assert.equal(documentRetirement(failure,retirement).reason,
    'ready resident channel retired by the observed document replacement');
});

function rewindingRetirement() {
  const {failure,captured}=evidence();
  failure.status=206;
  const channel=captured.channels[0];
  Object.assign(channel,{paused:true,currentTime:0,seeking:true,ready:1,network:2});
  channel.events.push({type:'seeking',trusted:true,paused:true,currentTime:0,seeking:true});
  return {failure,retirement:{kind:'document-replacement',pendingRequest:true,
    completed:true,before:captured,beforeTargetId:123,afterTargetId:124}};
}

test('replacement retires a loaded channel whose paused rewind is still seeking',()=>{
  const {failure,retirement}=rewindingRetirement();
  assert.deepEqual(documentRetirement(failure,retirement),{cue:'death',channelId:3,
    documentId:123,reason:'loaded resident channel rewinding during the observed document replacement'});
  // An unfinished seek in the current document is not an idle suspension.
  assert.equal(residentSuspension(failure,retirement.before,retirement.before),null);
});

for(const [name,change] of [
  ['an unfinished replacement',f=>{f.retirement.completed=false}],
  ['a request outside the pending set',f=>{f.retirement.pendingRequest=false}],
  ['a real network failure',f=>{f.failure.errorText='net::ERR_CONNECTION_RESET'}],
  ['a failed HTTP response',f=>{f.failure.status=503}],
  ['a channel that never loaded',f=>{f.retirement.before.channels[0].events.splice(1,1)}],
  ['an unobserved seek',f=>{f.retirement.before.channels[0].events.pop()}],
  ['a fabricated seek',f=>{f.retirement.before.channels[0].events.at(-1).trusted=false}],
  ['a completed seek',f=>{f.retirement.before.channels[0].events.push({type:'seeked',trusted:true})}],
  ['a media error',f=>{f.retirement.before.channels[0].error=3}],
  ['a media abort',f=>{f.retirement.before.channels[0].events.push({type:'abort',trusted:true})}],
])test(`a retired rewind does not excuse ${name}`,()=>{
  const fixture=rewindingRetirement();change(fixture);
  assert.equal(documentRetirement(fixture.failure,fixture.retirement),null);
});

for(const [name,change] of [
  ['failed navigation',r=>{r.completed=false}],
  ['same-document navigation',r=>{r.afterTargetId=r.beforeTargetId}],
  ['already-failed request',r=>{r.pendingRequest=false}],
  ['wrong resource owner',r=>{r.before.channels[0].url+='?other'}],
  ['media error',r=>{r.before.channels[0].error=2}],
  ['media abort',r=>{r.before.channels[0].events.push({type:'abort',trusted:true})}],
  ['channel reload',r=>{r.before.channels[0].loads=2}],
])test(`document replacement does not excuse ${name}`,()=>{
  const {failure,captured}=evidence();
  const retirement={pendingRequest:true,completed:true,before:captured,
    beforeTargetId:123,afterTargetId:124,afterUrl:'http://127.0.0.1:4181/'};
  change(retirement);
  assert.equal(documentRetirement(failure,retirement),null);
});

test('an exact pending resident request can retire with its actually removed child frame',()=>{
  const {failure,captured}=evidence();
  const retirement={kind:'frame-removal',pendingRequest:true,completed:true,before:captured,
    beforeTargetId:123,detachedTargetId:123,detached:true};
  assert.equal(documentRetirement(failure,retirement)?.reason,
    'ready resident channel retired by the observed frame removal');
});

for(const [name,change] of [
  ['an attached frame',r=>{r.detached=false}],
  ['another removed document',r=>{r.detachedTargetId=124}],
  ['an incomplete action',r=>{r.completed=false}],
  ['a request outside the pending set',r=>{r.pendingRequest=false}],
  ['a channel that never became ready',r=>{r.before.channels[0].ready=1}],
  ['an actual media error',r=>{r.before.channels[0].error=3}],
  ['a replaced media source',r=>{r.before.channels[0].sources.push('https://example.com/other.mp3')}],
])test(`frame removal does not excuse ${name}`,()=>{
  const {failure,captured}=evidence();
  const retirement={kind:'frame-removal',pendingRequest:true,completed:true,before:captured,
    beforeTargetId:123,detachedTargetId:123,detached:true};
  change(retirement);
  assert.equal(documentRetirement(failure,retirement),null);
});
