// Browser failures are fatal unless their actual owner supplies complete evidence.
const fs=require('node:fs');
const path=require('node:path');
const catalog=fs.readFileSync(path.resolve(__dirname,'../../crates/sim/src/audio/catalog.rs'),'utf8');
const musicPaths=new Map([...catalog.matchAll(/id: "([^"]+)",\s*path: "([^"]+)",\s*kind: AudioKind::Music/g)]
  .map(match=>[match[1],match[2]]));
const observers=new WeakMap();
let nextObserver=0;

function loadedResidentChannel(failure,snapshot) {
  if(failure.errorText!=='net::ERR_ABORTED'||failure.method!=='GET'
    ||failure.resourceType!=='media'||![200,206].includes(failure.status)||!snapshot)return null;
  const matches=snapshot.channels.filter(channel=>channel.url===failure.url);
  if(matches.length!==1)return null;
  const channel=matches[0];
  const stock=musicPaths.get(channel.cue);
  if(!stock||new URL(`assets/stock/audio/${stock}`,snapshot.baseUrl??snapshot.documentUrl).href!==failure.url)return null;
  if(channel.src!==failure.url||!channel.connected||!channel.hidden||channel.controls
    ||channel.preload!=='auto'||channel.retired||channel.loads!==1||channel.sources.length!==1
    ||channel.sources[0]!==failure.url||channel.error!==null)return null;
  const events=channel.events;
  const loaded=events.findIndex(event=>event.type==='loadeddata'&&event.trusted);
  if(loaded<0||events.filter(event=>event.type==='loadstart').length!==1
    ||events.some((event,index)=>['error','abort'].includes(event.type)
      ||(event.type==='emptied'&&index>loaded)))return null;
  return channel;
}

function suspendedResidentChannel(channel) {
  return channel.ready>=2&&channel.network===1
    &&channel.events.some(event=>event.type==='suspend'&&event.trusted&&event.network===1);
}

function residentSuspension(failure,captured,settled) {
  if(!captured||!settled||captured.documentId!==settled.documentId)return null;
  const before=loadedResidentChannel(failure,captured);
  const after=loadedResidentChannel(failure,settled);
  if(!before||!after||after.id!==before.id||after.cue!==before.cue
    ||![before,after].every(suspendedResidentChannel))return null;
  return {cue:before.cue,channelId:before.id,documentId:captured.documentId,
    reason:'ready resident channel suspended by the browser'};
}

function documentRetirement(failure,retirement) {
  if(!retirement?.pendingRequest||!retirement.completed)return null;
  const removal=retirement.kind==='frame-removal';
  if(removal) {
    if(!retirement.detached||retirement.detachedTargetId!==retirement.beforeTargetId)return null;
  } else if(retirement.beforeTargetId===retirement.afterTargetId)return null;
  const channel=loadedResidentChannel(failure,retirement.before);
  if(!channel)return null;
  const action=removal?'frame removal':'document replacement';
  let reason=`ready resident channel retired by the observed ${action}`;
  if(!suspendedResidentChannel(channel)) {
    // Rewinding a loaded, paused song can temporarily leave only metadata while
    // its range request is pending. Only the observed document retirement owns
    // that cancellation; the same state in a live document remains a failure.
    const loaded=channel.events.findIndex(event=>event.type==='loadeddata'&&event.trusted);
    const lastSeek=channel.events.findLastIndex(event=>['seeking','seeked'].includes(event.type));
    const seek=channel.events[lastSeek];
    if(!channel.paused||channel.currentTime!==0||!channel.seeking||channel.ready<1
      ||![1,2].includes(channel.network)||lastSeek<=loaded||seek.type!=='seeking'
      ||!seek.trusted||!seek.paused||seek.currentTime!==0||!seek.seeking)return null;
    reason=`loaded resident channel rewinding during the observed ${action}`;
  }
  return {cue:channel.cue,channelId:channel.id,documentId:retirement.before.documentId,reason};
}

// All wrappers forward the original native call unchanged. There are no setters
// or synthetic media events; this records the browser's actual channel lifecycle.
function observeResidentMedia() {
  const channels=new Map();
  let nextChannel=0;
  const describe=media=>({cue:media.dataset.audioCue||null,src:media.src,url:media.currentSrc,
    connected:media.isConnected,hidden:media.hidden,controls:media.controls,preload:media.preload,
    paused:media.paused,currentTime:media.currentTime,seeking:media.seeking,
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
  for(const type of ['loadstart','loadeddata','suspend','abort','emptied','error','playing','seeking','seeked']) {
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
    mediaEvents:new Set(),activeMedia:new Map(),retiringRequests:new Map(),replacements:[],
    nextRequest:0,id:++nextObserver};
  observed.ready=page.addInitScript(observeResidentMedia);
  observers.set(page,observed);
  page.on('request',request=>{
    if(request.resourceType()==='media')observed.activeMedia.set(request,{
      id:++observed.nextRequest,owner:request.frame(),status:null});
  });
  page.on('response',response=>{
    const row=observed.activeMedia.get(response.request());
    if(row)row.status=response.status();
  });
  page.on('requestfinished',request=>observed.activeMedia.delete(request));
  page.on('requestfailed',request=>{
    const tracked=observed.activeMedia.get(request);
    const retirement=observed.retiringRequests.get(request);
    observed.activeMedia.delete(request);
    const failure={type:'requestfailed',url:request.url(),method:request.method(),
      resourceType:request.resourceType(),failure:request.failure(),requestId:tracked?.id};
    errors.push(failure);observed.rawFailures.push(failure);
    if(failure.resourceType!=='media'||failure.failure?.errorText!=='net::ERR_ABORTED')return;
    const owner=tracked?.owner??request.frame();
    const status=tracked?.status??null;
    const pending=owner.evaluate(()=>window.__solomonResidentMedia?.()??null)
      .then(captured=>({failure,status,captured,owner,retirement}),
        error=>({failure,status,owner,retirement,observationError:error.message}));
    observed.pending.push(pending);
  });
  return {errors,warnings,rawFailures:observed.rawFailures,
    expectedSuspensions:observed.expectedSuspensions,ready:observed.ready};
}

async function beginRetirement(page,frame,kind) {
  const observed=observers.get(page);
  if(!observed)throw new Error('document retirement requires the page error observer');
  await settleErrors(page);
  const snapshots=new Map(await Promise.all(page.frames().map(async owner=>[owner,
    await owner.evaluate(()=>window.__solomonResidentMedia?.()??null)])));
  const target=snapshots.get(frame);
  if(!target)throw new Error('the retiring document has no lifecycle observation');
  const transition={kind,beforeTargetId:target.documentId,beforeUrl:target.documentUrl,completed:false,
    pendingRequests:[],navigations:[]};
  observed.replacements.push(transition);
  for(const [request,tracked] of observed.activeMedia) {
    let within=false;
    for(let ancestor=tracked.owner;ancestor;ancestor=ancestor.parentFrame())if(ancestor===frame)within=true;
    if(!within)continue;
    const before=snapshots.get(tracked.owner);
    const evidence={requestId:tracked.id,url:request.url(),method:request.method(),
      resourceType:request.resourceType(),status:tracked.status,before};
    transition.pendingRequests.push(evidence);
    // The exact still-pending Request instance is the membership key. A new
    // request for the same URL can never inherit this retirement observation.
    observed.retiringRequests.set(request,{transition,before,pendingRequest:true});
  }
  return {target,transition};
}

async function observeFrameRemoval(page,action,{frame,timeout=45000}) {
  if(!frame||frame===page.mainFrame()||frame.isDetached())
    throw new Error('frame removal requires the actual attached child frame');
  const {target,transition}=await beginRetirement(page,frame,'frame-removal');
  let resolveRemoval;
  const removed=new Promise(resolve=>{resolveRemoval=resolve});
  const onRemoval=owner=>{if(owner===frame)resolveRemoval(true)};
  page.on('framedetached',onRemoval);
  const timer=setTimeout(()=>resolveRemoval(false),timeout);
  try {
    const result=await action();
    if(!await removed||!frame.isDetached()||page.frames().includes(frame))
      throw new Error('the actual action did not remove its owned frame');
    transition.detached=true;
    transition.detachedTargetId=target.documentId;
    transition.completed=true;
    return result;
  } catch(error) {
    transition.failure=error.message;
    throw error;
  } finally {
    clearTimeout(timer);
    page.off('framedetached',onRemoval);
  }
}

async function observeDocumentReplacement(page,action,{frame=page.mainFrame(),timeout=45000}={}) {
  const {target,transition}=await beginRetirement(page,frame,'document-replacement');
  let resolveNavigation;
  const navigated=new Promise(resolve=>{resolveNavigation=resolve});
  const onNavigation=async owner=>{
    if(owner!==frame)return;
    const event={url:owner.url()};
    transition.navigations.push(event);
    try {
      const after=await owner.evaluate(()=>({documentId:performance.timeOrigin,url:location.href}));
      event.documentId=after.documentId;
      // A Website router may first pushState into its game route, then its
      // ordinary bridge replaces the document. The intermediate SPA event is
      // recorded, but supplies no document-retirement evidence.
      if(after.documentId!==target.documentId)resolveNavigation(after);
    } catch(error) {
      event.observationError=error.message;
    }
  };
  page.on('framenavigated',onNavigation);
  const timer=setTimeout(()=>resolveNavigation(null),timeout);
  try {
    const result=await action();
    const after=await navigated;
    if(!after)throw new Error('the actual action did not replace its document');
    transition.afterTargetId=after.documentId;
    transition.afterUrl=after.url;
    transition.completed=true;
    return result;
  } catch(error) {
    transition.failure=error.message;
    throw error;
  } finally {
    clearTimeout(timer);
    page.off('framenavigated',onNavigation);
  }
}

function watchErrors(page,{errors=[],warnings=[]}={}) {
  const observed=watchRequestFailures(page,{errors,warnings});
  page.on('pageerror',error=>errors.push({type:'pageerror',message:error.message,stack:error.stack}));
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
    const {owner,retirement,...evidence}=row;
    const settled=snapshots.get(owner)??null;
    const failure={...row.failure,status:row.status,errorText:row.failure.failure?.errorText};
    const retired=retirement?{...retirement.transition,before:retirement.before,
      pendingRequest:retirement.pendingRequest}:null;
    const classification=residentSuspension(failure,row.captured,settled)
      ??documentRetirement(failure,retired);
    observed.mediaFailures.push({...evidence,retirement:retired,settled,classification});
    if(classification) {
      const index=observed.errors.indexOf(row.failure);
      if(index>=0)observed.errors.splice(index,1);
      observed.expectedSuspensions.push({failure:row.failure,classification,
        captured:row.captured,settled,retirement:retired});
    }
  }
  // A saved phase must not acquire failures or classifications from later work.
  const report=structuredClone({url:page.url(),rawFailures:observed.rawFailures,
    expectedSuspensions:observed.expectedSuspensions,mediaFailures:observed.mediaFailures,
    replacements:observed.replacements,
    media:snapshots.get(page.mainFrame()),frames:[...snapshots.values()].filter(Boolean),
    errors:observed.errors,warnings:observed.warnings});
  if(process.env.OUT) {
    fs.mkdirSync(process.env.OUT,{recursive:true});
    fs.writeFileSync(path.join(process.env.OUT,`browser-errors-${observed.id}.json`),
      `${JSON.stringify(report,null,2)}\n`);
  }
  return report;
}

module.exports={watchErrors,watchRequestFailures,settleErrors,residentSuspension,
  documentRetirement,observeDocumentReplacement,observeFrameRemoval};
