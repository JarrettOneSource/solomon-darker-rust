// Private, bounded A/B/A observation; network failures are measurements, not passes.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const os=require('node:os');
const path=require('node:path');
const {spawnSync}=require('node:child_process');
const {chromium}=require(path.resolve(process.env.PW));
const {launchOwnedBrowser}=require('./browser-smoke/browser-lifecycle.cjs');

assert.equal(process.platform,'win32');
const root=path.resolve('reports/socket-buffer');
const url='http://127.0.0.1:4181/';
const feature='TcpPortRandomizationWin';
const phases=[{name:'default-before',pages:3000},{name:'disabled',pages:8000},
  {name:'default-restored',pages:3000}];
const results=[];
let defaultDisabledFeatures;

function write(file,value) {
  fs.writeFileSync(file,JSON.stringify(value,null,2)+'\n');
}

function resources() {
  const result={time:Date.now(),availablePhysicalBytes:os.freemem(),
    totalPhysicalBytes:os.totalmem()};
  const tcp=spawnSync('netstat',['-ano','-p','tcp'],{encoding:'utf8',timeout:15000});
  const rows=(tcp.stdout||'').split(/\r?\n/).map(line=>line.trim().split(/\s+/))
    .filter(row=>row[0]==='TCP'&&row.length===5);
  return {...result,netstatExit:tcp.status,netstatError:tcp.error?.message||tcp.stderr,
    localEndpoints:new Set(rows.map(row=>row[1])).size,
    states:rows.reduce((counts,row)=>{counts[row[3]]=(counts[row[3]]||0)+1;return counts},{}),
    acceptanceConnections:rows.filter(row=>row[1].endsWith(':4181')||row[2].endsWith(':4181'))};
}

async function observePhase(phase) {
  const out=path.join(root,phase.name);
  fs.mkdirSync(out,{recursive:true});
  const args=['--use-angle=swiftshader','--enable-unsafe-swiftshader',
    '--ignore-gpu-blocklist',`--log-net-log=${path.join(out,'network.json')}`];
  const options={headless:true,executablePath:process.env.CHROME,args};
  if(phase.name==='disabled') {
    assert.ok(defaultDisabledFeatures);
    options.ignoreDefaultArgs=[defaultDisabledFeatures];
    args.push(`${defaultDisabledFeatures},${feature}`);
  }
  const report={phase:phase.name,pageLimit:phase.pages,concurrency:4,
    durationLimitSeconds:600,start:Date.now(),attempted:0,passed:0,failed:0,
    failures:[],before:resources()};
  console.log(JSON.stringify({phase:phase.name,event:'start',time:report.start}));
  const owned=await launchOwnedBrowser(chromium,options);
  try {
    const session=await owned.browser.newBrowserCDPSession();
    try {report.chrome=await session.send('SystemInfo.getInfo')}
    finally {await session.detach()}
    report.version=owned.browser.version();
    const switches=report.chrome.commandLine.match(/--disable-features=[^"\s]+/g)||[];
    assert.equal(switches.length,1,'one effective disabled-feature list');
    if(phase.name==='default-before') {
      defaultDisabledFeatures=switches[0];
      assert.ok(!defaultDisabledFeatures.includes(feature));
    } else {
      assert.equal(switches[0],defaultDisabledFeatures+(phase.name==='disabled'?','+feature:''),
        'the controlled phase changes only the named disabled feature');
    }
    write(path.join(out,'launch.json'),{options,version:report.version,chrome:report.chrome});
    const deadline=Date.now()+report.durationLimitSeconds*1000;
    let next=0,stop=false;
    async function worker() {
      while(!stop&&next<phase.pages&&Date.now()<deadline) {
        const index=next++;
        const record={index,start:Date.now(),requestsFailed:[],pageErrors:[],badResponses:[]};
        let context;
        try {
          context=await owned.browser.newContext();
          const page=await context.newPage();
          page.on('requestfailed',request=>record.requestsFailed.push({url:request.url(),
            method:request.method(),type:request.resourceType(),error:request.failure()}));
          page.on('pageerror',error=>record.pageErrors.push(error.message));
          page.on('response',response=>{if(response.status()>=400)
            record.badResponses.push({url:response.url(),status:response.status()})});
          const response=await page.goto(url+'?page='+index,{waitUntil:'domcontentloaded',timeout:15000});
          assert.equal(response.status(),200);
          await page.waitForFunction(()=>globalThis.probeResult!==undefined,{},{timeout:15000});
          record.payloads=await page.evaluate(()=>globalThis.probeResult);
          assert.equal(record.payloads.length,5);
          assert.ok(record.payloads.every(item=>item.status==='fulfilled'&&item.value===16384),
            'all five real asset bodies arrived');
          assert.deepEqual(record.requestsFailed,[]);
          assert.deepEqual(record.pageErrors,[]);
          assert.deepEqual(record.badResponses,[]);
          record.pass=true;
        } catch(error) {
          record.pass=false;
          record.error=error.stack;
          stop=true;
          if(!report.failureResources) {
            report.failureResources=resources();
            write(path.join(out,'failure-resources.json'),report.failureResources);
          }
        } finally {
          if(context)await context.close();
        }
        record.finish=Date.now();
        report.attempted++;
        if(record.pass)report.passed++;
        else {report.failed++;report.failures.push(record)}
        fs.appendFileSync(path.join(out,'pages.jsonl'),JSON.stringify(record)+'\n');
        if(report.attempted%250===0||!record.pass)
          console.log(JSON.stringify({phase:phase.name,attempted:report.attempted,
            failed:report.failed,time:record.finish}));
      }
    }
    const workers=await Promise.allSettled(Array.from({length:4},worker));
    for(const result of workers)if(result.status==='rejected')throw result.reason;
    report.stopReason=stop?'observed failure':next===phase.pages?'page limit':'time limit';
  } finally {
    try {report.browserCleanup=await owned.close()}
    catch(error) {report.browserCleanup=error.cleanup||{error:error.message};throw error}
    finally {
      report.finish=Date.now();report.after=resources();
      write(path.join(out,'report.json'),report);
    }
  }
  results.push(report);
  write(path.join(root,'comparison.json'),{results});
  console.log(JSON.stringify({phase:phase.name,event:'complete',attempted:report.attempted,
    passed:report.passed,failed:report.failed,stopReason:report.stopReason}));
}

(async()=>{
  const deadline=Date.now()+15000;
  let ready=false;
  while(Date.now()<deadline) {
    try {ready=(await fetch(url,{signal:AbortSignal.timeout(1000)})).ok}
    catch(error) {if(Date.now()>=deadline)throw error}
    if(ready)break;
    await new Promise(resolve=>setTimeout(resolve,100));
  }
  assert.ok(ready,'owned HTTP server ready');
  write(path.join(root,'system.json'),{platform:os.platform(),release:os.release(),
    version:os.version(),hostname:os.hostname(),node:process.version});
  for(const phase of phases)await observePhase(phase);
  assert.equal(results.length,3);
  write(path.join(root,'comparison.json'),{complete:true,
    meaning:'A/B/A measurements; a completed diagnostic is not an acceptance pass.',results});
})().catch(error=>{
  write(path.join(root,'diagnostic-error.json'),{message:error.message,stack:error.stack});
  console.error(error);process.exitCode=1;
});
