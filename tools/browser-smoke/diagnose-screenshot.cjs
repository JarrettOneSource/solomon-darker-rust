// Disposable Windows capture diagnostic. This file is not a release change.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {performance}=require('node:perf_hooks');
const out=path.resolve(process.env.OUT);
fs.mkdirSync(out,{recursive:true});
const started=performance.now();
const budget=Number(process.env.CAPTURE_TIMEOUT_MS);
assert.ok(Number.isFinite(budget)&&budget>0);
globalThis.__solomonScreenshotTrace=event=>{
  const row={elapsedMs:Math.round(performance.now()-started),...event};
  fs.appendFileSync(path.join(out,'capture-timing.jsonl'),JSON.stringify(row)+'\n');
  console.log(JSON.stringify(row));
};
const trace=globalThis.__solomonScreenshotTrace;

// Instrument the pinned Playwright implementation around its two Chrome calls.
// The command arguments and results pass through unchanged.
const bundle=path.join(path.resolve(process.env.PW),'lib/coreBundle.js');
const original=fs.readFileSync(bundle,'utf8');
let instrumented=original;
for(const [needle,before,after] of [
  ['const { visualViewport, contentSize, cssContentSize } = await progress2.race(this._mainFrameSession._client.send("Page.getLayoutMetrics"));',
    'layout-start','layout-complete'],
  ['const result2 = await progress2.race(this._mainFrameSession._client.send("Page.captureScreenshot", { format: format2, quality, clip, captureBeyondViewport: !fitsViewport }));',
    'capture-start','capture-complete'],
]) {
  assert.equal(instrumented.split(needle).length,2,'the pinned Chrome call must occur exactly once');
  instrumented=instrumented.replace(needle,
    `globalThis.__solomonScreenshotTrace?.({event:${JSON.stringify(before)}});\n${needle}\nglobalThis.__solomonScreenshotTrace?.({event:${JSON.stringify(after)}});`);
}
fs.writeFileSync(bundle,instrumented);
process.on('exit',()=>fs.writeFileSync(bundle,original));

const lifecycle=require('./browser-lifecycle.cjs');
const launch=lifecycle.launchOwnedBrowser;
lifecycle.launchOwnedBrowser=async(...args)=>{
  const owned=await launch(...args);
  trace({event:'browser-launched',version:owned.browser.version(),budget});
  const pages=[];
  const newPage=owned.browser.newPage.bind(owned.browser);
  owned.browser.newPage=async(...pageArgs)=>{
    const page=await newPage(...pageArgs);
    pages.push(page);
    page.on('crash',()=>trace({event:'page-crashed'}));
    page.__solomonDiagnosticEarly=true;
    const evaluate=page.evaluate.bind(page);
    page.evaluate=async(...evaluateArgs)=>{
      const source=String(evaluateArgs[0]);
      const kind=source.includes('wasmBindings?.gpu_ui_state')?'gpu-ui-read'
        :source.includes('requestAnimationFrame')?'animation-frame-wait':null;
      if(!kind||!page.__solomonDiagnosticEarly)return evaluate(...evaluateArgs);
      const operationStarted=performance.now();
      trace({event:'evaluation-start',kind});
      try {
        const result=await evaluate(...evaluateArgs);
        const state=kind==='gpu-ui-read'&&result?{
          frame:result.frame,ready:result.ready,surface:result.surface,error:result.error,
          widgets:result.widgets.filter(w=>['settings','prompt-secondary'].includes(w.id))
            .map(w=>({id:w.id,enabled:w.enabled,rect:w.rect,z:w.z})),
        }:undefined;
        trace({event:'evaluation-complete',kind,durationMs:Math.round(performance.now()-operationStarted),state});
        return result;
      } catch(error) {
        trace({event:'evaluation-failed',kind,durationMs:Math.round(performance.now()-operationStarted),message:error.message});
        throw error;
      }
    };
    const screenshot=page.screenshot.bind(page);
    page.screenshot=async options=>{
      const captureStarted=performance.now();
      trace({event:'screenshot-start',file:path.basename(options.path),budget});
      try {
        const result=await screenshot({...options,timeout:budget});
        trace({event:'screenshot-complete',durationMs:Math.round(performance.now()-captureStarted)});
        return result;
      } catch(error) {
        trace({event:'screenshot-failed',durationMs:Math.round(performance.now()-captureStarted),message:error.message});
        throw error;
      }
    };
    return page;
  };
  const close=owned.close.bind(owned);
  owned.close=async()=>{
    for(const page of pages)if(!page.isClosed()) {
      let timer;
      try {
        await Promise.race([
          require('./gpu-ui.cjs').settleErrors(page),
          new Promise((_,reject)=>{timer=setTimeout(()=>reject(new Error('Diagnostic media observation timed out')),20000)}),
        ]);
      } catch(error) {
        trace({event:'media-observation-failed',message:error.message});
      } finally {
        clearTimeout(timer);
      }
    }
    const result=await close();
    trace({event:'browser-closed',result});
    return result;
  };
  return owned;
};
const gpu=require('./gpu-ui.cjs');
const click=gpu.click;
gpu.click=async(page,id,...args)=>{
  const operationStarted=performance.now();
  trace({event:'click-start',id});
  try {
    const result=await click(page,id,...args);
    trace({event:'click-complete',id,durationMs:Math.round(performance.now()-operationStarted)});
    if(id==='settings')page.__solomonDiagnosticEarly=false;
    return result;
  } catch(error) {
    trace({event:'click-failed',id,durationMs:Math.round(performance.now()-operationStarted),message:error.message});
    throw error;
  }
};
const journey=process.env.CAPTURE_JOURNEY;
assert.ok(['smoke','settings-parity'].includes(journey));
require(`./${journey}.cjs`);
