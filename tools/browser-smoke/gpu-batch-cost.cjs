// Compare actual Renderer submissions from baseline/candidate batch_cost WASM examples.
// Set PW, BASELINE_DIR, CANDIDATE_DIR and OUT; uses the default native GPU backend.
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),http=require('node:http'),crypto=require('node:crypto');
const {chromium}=require(path.resolve(process.env.PW));
const {launchOwnedBrowser}=require('./browser-lifecycle.cjs');
const directories={baseline:process.env.BASELINE_DIR,candidate:process.env.CANDIDATE_DIR};
const files=new Map(),artifacts=[];
for(const [version,directory] of Object.entries(directories))for(const file of ['batch_cost.js','batch_cost_bg.wasm']){
 const bytes=fs.readFileSync(path.join(directory,file));files.set('/'+version+'/'+file,[file.endsWith('.js')?'text/javascript':'application/wasm',bytes]);artifacts.push({version,file,bytes:bytes.length,sha256:crypto.createHash('sha256').update(bytes).digest('hex')});
}
(async()=>{
 const server=http.createServer((request,response)=>{
  const file=request.url==='/'?['text/html','<!doctype html><title>Atlas batch cost</title><link rel="icon" href="data:,">']:files.get(request.url);
  response.writeHead(file?200:404,{'content-type':file?.[0]||'text/plain'});response.end(file?.[1]||'Not found');
 });await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));let owned;
 try{
  owned=await launchOwnedBrowser(chromium,{headless:true,executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'});
  const page=await owned.browser.newPage(),errors=[];page.on('pageerror',error=>errors.push(String(error)));await page.goto(`http://127.0.0.1:${server.address().port}`);
  const result=await page.evaluate(async()=>{
   const probes={},gpu={};
   for(const version of ['baseline','candidate']){
    const module=await import('/'+version+'/batch_cost.js');await module.default();
    const canvas=document.createElement('canvas');canvas.width=canvas.height=512;const gl=canvas.getContext('webgl2',{premultipliedAlpha:false,preserveDrawingBuffer:true});if(!gl)throw Error('WebGL2 missing');
    const ext=gl.getExtension('WEBGL_debug_renderer_info');gpu[version]={renderer:ext?gl.getParameter(ext.UNMASKED_RENDERER_WEBGL):gl.getParameter(gl.RENDERER),version:gl.getParameter(gl.VERSION)};
    module.batch_cost_init(gl);probes[version]={module,gl};
   }
   const rows=[];
   try{
    for(const count of [256,4096])for(const records of [false,true]){
     const setup={};
     for(const [version,{module,gl}] of Object.entries(probes)){
      let draws=0;const draw=gl.drawArrays.bind(gl);gl.drawArrays=(...args)=>{draws++;return draw(...args)};
      module.batch_cost_frame(records,count);gl.drawArrays=draw;
      const pixel=new Uint8Array(4);gl.readPixels(4,507,1,1,gl.RGBA,gl.UNSIGNED_BYTE,pixel);if(pixel[2]<40)throw Error('Invisible cost fixture '+version+JSON.stringify([...pixel]));
      setup[version]={draws,pixel:[...pixel]};for(let i=0;i<6;i++)module.batch_cost_frame(records,count);
     }
     for(let round=0;round<6;round++)for(const version of round%2?['candidate','baseline']:['baseline','candidate']){
      const {module,gl}=probes[version];const started=performance.now();for(let frame=0;frame<8;frame++)module.batch_cost_frame(records,count);const elapsed=performance.now()-started;
      if(gl.getError()!==gl.NO_ERROR)throw Error('Graphics error '+version);
      rows.push({version,count,records,round,frames:8,elapsedMs:elapsed,msPerFrame:elapsed/8,...setup[version]});
     }
    }
   }finally{for(const {module,gl} of Object.values(probes)){module.batch_cost_dispose();gl.getExtension('WEBGL_lose_context')?.loseContext();}}
   return {gpu,rows};
  });
  fs.mkdirSync(process.env.OUT,{recursive:true});fs.writeFileSync(path.join(process.env.OUT,'receipt.json'),JSON.stringify({artifacts,...result,errors},null,2));assert.deepEqual(errors,[]);
  const summaries=[];for(const count of [256,4096])for(const records of [false,true])for(const version of ['baseline','candidate']){
   const rows=result.rows.filter(r=>r.count===count&&r.records===records&&r.version===version),v=rows.map(r=>r.msPerFrame).sort((a,b)=>a-b);summaries.push({version,count,records,draws:rows[0].draws,medianMs:(v[2]+v[3])/2,minMs:v[0],maxMs:v[5]});
  }
  fs.writeFileSync(path.join(process.env.OUT,'summary.json'),JSON.stringify(summaries,null,2));console.log(JSON.stringify({gpu:result.gpu,summaries,errors}));
 }finally{if(owned)await owned.close();await new Promise(resolve=>server.close(resolve));}
})().catch(error=>{console.error(error);process.exitCode=1});
