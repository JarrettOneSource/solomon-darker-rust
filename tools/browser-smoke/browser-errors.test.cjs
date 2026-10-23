const test=require('node:test');
const assert=require('node:assert/strict');
const {residentSuspension,documentRetirement}=require('./browser-errors.cjs');

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
