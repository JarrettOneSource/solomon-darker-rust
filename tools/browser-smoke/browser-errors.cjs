// Browser failures are fatal unless their actual owner supplies complete evidence.
const fs=require('node:fs');
const path=require('node:path');
const catalog=fs.readFileSync(path.resolve(__dirname,'../../crates/sim/src/audio/catalog.rs'),'utf8');
const musicPaths=new Map([...catalog.matchAll(/id: "([^"]+)",\s*path: "([^"]+)",\s*kind: AudioKind::Music/g)]
  .map(match=>[match[1],match[2]]));
const observers=new WeakMap();
let nextObserver=0;

function residentSuspension(failure,captured,settled) {
  if(failure.errorText!=='net::ERR_ABORTED'||failure.method!=='GET'
    ||failure.resourceType!=='media'||![200,206].includes(failure.status)
    ||!captured||!settled||captured.documentId!==settled.documentId)return null;
  const matches=captured.channels.filter(channel=>channel.url===failure.url);
  if(matches.length!==1)return null;
  const before=matches[0];
  const remaining=settled.channels.filter(channel=>channel.url===failure.url);
  if(remaining.length!==1)return null;
  const after=remaining[0];
  if(after.id!==before.id)return null;
  const stock=musicPaths.get(before.cue);
  if(!stock||new URL(`assets/stock/audio/${stock}`,captured.baseUrl??captured.documentUrl).href!==failure.url)return null;
  for(const channel of [before,after]) {
    if(!channel||channel.cue!==before.cue||channel.url!==failure.url||channel.src!==failure.url
      ||!channel.connected||!channel.hidden||channel.controls||channel.preload!=='auto'
      ||channel.retired||channel.loads!==1||channel.sources.length!==1
      ||channel.sources[0]!==failure.url||channel.error!==null||channel.ready<2||channel.network!==1)return null;
    const events=channel.events;
    const loaded=events.findIndex(event=>event.type==='loadeddata'&&event.trusted);
    if(loaded<0||events.filter(event=>event.type==='loadstart').length!==1
      ||!events.some(event=>event.type==='suspend'&&event.trusted&&event.network===1)
      ||events.some((event,index)=>['error','abort'].includes(event.type)
        ||(event.type==='emptied'&&index>loaded)))return null;
  }
  return {cue:before.cue,channelId:before.id,documentId:captured.documentId,
    reason:'ready resident channel suspended by the browser'};
}

// All wrappers forward the original native call unchanged. There are no setters
// or synthetic media events; this records the browser's actual channel lifecycle.
function observeResidentMedia() {
  const channels=new Map();
  let nextChannel=0;
  const describe=media=>({cue:media.dataset.audioCue||null,src:media.src,url:media.currentSrc,
    connected:media.isConnected,hidden:media.hidden,controls:media.controls,preload:media.preload,
    ready:media.readyState,network:media.networkState,error:media.error?.code??null});
  const own=media=>{
    if(!channels.has(media))channels.set(media,{id:++nextChannel,loads:0,sources:[],events:[],retired:false});
    return channels.get(media);
  };
  const src=Object.getOwnPropertyDescriptor(HTMLMediaElement.prototype,'src');
  Object.defineProperty(HTMLMediaElement.prototype,'src',{...src,set(value){
    Reflect.apply(src.set,this,[value]);
    own(this).sources.push(this.src);
  }});
  const setAttribute=Element.prototype.setAttribute;
  Element.prototype.setAttribute=function(name,value){
    const result=Reflect.apply(setAttribute,this,[name,value]);
    if(this instanceof HTMLMediaElement&&name.toLowerCase()==='src')own(this).sources.push(this.src);
    return result;
  };
  const load=HTMLMediaElement.prototype.load;
  HTMLMediaElement.prototype.load=function(...args){
    own(this).loads+=1;
    return Reflect.apply(load,this,args);
  };
  for(const type of ['loadstart','loadeddata','suspend','abort','emptied','error','playing']) {
    document.addEventListener(type,event=>{
      if(event.target instanceof HTMLMediaElement)own(event.target).events.push({type,
        trusted:event.isTrusted,time:performance.now(),...describe(event.target)});
    },true);
  }
  new MutationObserver(records=>{
    for(const record of records)for(const removed of record.removedNodes) {
      for(const [media,channel] of channels)if(removed===media||removed.contains(media))channel.retired=true;
    }
  }).observe(document,{subtree:true,childList:true});
  Object.defineProperty(window,'__solomonResidentMedia',{value:()=>({
    documentId:performance.timeOrigin,documentUrl:location.href,baseUrl:document.baseURI,
    channels:[...channels].map(([media,channel])=>({...channel,...describe(media)})),
  })});
}

function watchRequestFailures(page,{errors=[],warnings=[]}={}) {
  if(observers.has(page))throw new Error('one browser error observer owns each page');
  const observed={errors,warnings,rawFailures:[],expectedSuspensions:[],mediaFailures:[],pending:[],
    mediaEvents:new Set(),id:++nextObserver};
  observed.ready=page.addInitScript(observeResidentMedia);
  observers.set(page,observed);
  page.on('requestfailed',request=>{
    const failure={type:'requestfailed',url:request.url(),method:request.method(),
      resourceType:request.resourceType(),failure:request.failure()};
    errors.push(failure);observed.rawFailures.push(failure);
    if(failure.resourceType!=='media'||failure.failure?.errorText!=='net::ERR_ABORTED')return;
    const pending=(async()=>{
      const owner=request.frame();
      const [response,captured]=await Promise.all([request.response(),
        owner.evaluate(()=>window.__solomonResidentMedia?.()??null)]);
      return {failure,status:response?.status()??null,captured,owner};
    })().catch(error=>({failure,observationError:error.message}));
    observed.pending.push(pending);
  });
  return {errors,warnings,rawFailures:observed.rawFailures,
    expectedSuspensions:observed.expectedSuspensions,ready:observed.ready};
}

function watchErrors(page,{errors=[],warnings=[]}={}) {
  const observed=watchRequestFailures(page,{errors,warnings});
  page.on('pageerror',error=>errors.push({type:'pageerror',message:error.message}));
  page.on('console',message=>{
    if(message.type()==='error')errors.push({type:'console',message:message.text(),location:message.location()});
    else if(message.type()==='warning')warnings.push(message.text());
  });
  page.on('response',response=>{
    if(response.status()>=400)errors.push({type:'response',status:response.status(),url:response.url()});
  });
  return observed;
}

async function settleErrors(page) {
  const observed=observers.get(page);
  if(!observed)throw new Error('the page has no browser error observer');
  await observed.ready;
  // Let already queued native media events (including error/abort) run before
  // deciding whether a completed request failure belongs to idle suspension.
  const frames=page.frames();
  await Promise.all(frames.map(frame=>frame.evaluate(()=>new Promise(resolve=>setTimeout(resolve,0)))));
  const pending=await Promise.all(observed.pending.splice(0));
  const snapshots=new Map(await Promise.all(frames.map(async frame=>[frame,
    await frame.evaluate(()=>window.__solomonResidentMedia?.()??null)])));
  for(const snapshot of snapshots.values())for(const channel of snapshot?.channels??[])for(const event of channel.events) {
    if(!['error','abort'].includes(event.type))continue;
    const key=`${snapshot.documentId}:${channel.id}:${event.type}:${event.time}`;
    if(observed.mediaEvents.has(key))continue;
    observed.mediaEvents.add(key);
    observed.errors.push({type:'media',event:event.type,url:event.url,cue:event.cue,
      code:event.error,time:event.time});
  }
  for(const row of pending) {
    const {owner,...evidence}=row;
    const settled=snapshots.get(owner)??null;
    const classification=residentSuspension({...row.failure,status:row.status,
      errorText:row.failure.failure?.errorText},row.captured,settled);
    observed.mediaFailures.push({...evidence,settled,classification});
    if(classification) {
      const index=observed.errors.indexOf(row.failure);
      if(index>=0)observed.errors.splice(index,1);
      observed.expectedSuspensions.push({failure:row.failure,classification,
        captured:row.captured,settled});
    }
  }
  const report={url:page.url(),rawFailures:observed.rawFailures,
    expectedSuspensions:observed.expectedSuspensions,mediaFailures:observed.mediaFailures,
    media:snapshots.get(page.mainFrame()),frames:[...snapshots.values()].filter(Boolean),
    errors:observed.errors,warnings:observed.warnings};
  if(process.env.OUT) {
    fs.mkdirSync(process.env.OUT,{recursive:true});
    fs.writeFileSync(path.join(process.env.OUT,`browser-errors-${observed.id}.json`),
      `${JSON.stringify(report,null,2)}\n`);
  }
  return report;
}

module.exports={watchErrors,watchRequestFailures,settleErrors,residentSuspension};
