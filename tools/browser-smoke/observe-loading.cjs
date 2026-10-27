// Private read-only request and renderer measurements around the unchanged settings journey.
const fs = require('node:fs');
const path = require('node:path');
const lifecycle = require('./browser-lifecycle.cjs');
const gpu = require('./gpu-ui.cjs');
const output = path.resolve(process.env.OUT);
fs.mkdirSync(output,{recursive:true});
const record = (kind,value) => fs.appendFileSync(path.join(output,'loading-trace.jsonl'),
  JSON.stringify({time:Date.now(),kind,...value})+'\n');
const originalLaunch = lifecycle.launchOwnedBrowser;
lifecycle.launchOwnedBrowser = async (...args) => {
  const owner = await originalLaunch(...args);
  const create = owner.browser.newPage.bind(owner.browser);
  const samplers = [];
  owner.browser.newPage = async (...pageArgs) => {
    const page = await create(...pageArgs);
    const cdp = await page.context().newCDPSession(page);
    await cdp.send('Performance.enable');
    for (const event of ['request','requestfinished','requestfailed']) page.on(event,request => {
      record(event,{url:request.url(),type:request.resourceType(),timing:request.timing(),failure:request.failure()});
    });
    page.on('response',response => record('response',{url:response.url(),status:response.status()}));
    page.on('pageerror',error => record('pageerror',{error:String(error)}));
    let busy = false;
    const timer = setInterval(async () => {
      if (busy || page.isClosed()) return;
      busy = true;
      const start = Date.now();
      try {
        const [state,metrics] = await Promise.all([gpu.readUi(page),cdp.send('Performance.getMetrics')]);
        record('sample',{duration:Date.now()-start,state:state&&{frame:state.frame,ready:state.ready,
          surface:state.surface,loading:state.loading,error:state.error,draw:state.draw},metrics});
      } catch(error) {record('sample-error',{duration:Date.now()-start,error:String(error)});}
      finally {busy=false;}
    },10000);
    samplers.push(timer);
    return page;
  };
  const close = owner.close;
  owner.close = async () => {
    for (const timer of samplers) clearInterval(timer);
    return close();
  };
  return owner;
};
const originalReady = gpu.ready;
gpu.ready = async (...args) => {
  const start = Date.now();
  record('ready-start',{});
  try {const result=await originalReady(...args);record('ready-end',{duration:Date.now()-start});return result;}
  catch (error) {
    record('ready-failed',{duration:Date.now()-start,error:String(error)});
    // Preserve the original failure while observing two additional bounded minutes of progress.
    try {await originalReady(args[0],{timeout:120000});record('post-failure-ready',{});}
    catch (later) {record('post-failure-not-ready',{error:String(later)});}
    throw error;
  }
};
require('./settings-parity.cjs');
