const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const os=require('node:os');
const {chromium}=require('playwright-core');
const {launchOwnedBrowser}=require('./browser-lifecycle.cjs');
(async()=>{
  const receipt={pass:false};
  const owned=await launchOwnedBrowser(chromium,{headless:true,executablePath:process.env.CHROME,
    args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
  try {
    receipt.pages=owned.browser.contexts().flatMap(context=>context.pages()).length;
    assert.equal(receipt.pages,0,'GPU scheduling is established before the first page');
    const cdp=await owned.browser.newBrowserCDPSession();
    try {
      const {processInfo}=await cdp.send('SystemInfo.getProcessInfo');
      receipt.gpu=processInfo.filter(process=>process.type.toLowerCase()==='gpu')
        .map(process=>({...process,priority:os.getPriority(process.id)}));
      assert.equal(receipt.gpu.length,1);
      assert.equal(receipt.gpu[0].priority,os.constants.priority.PRIORITY_NORMAL);
      receipt.pass=true;
    } finally {await cdp.detach()}
  } finally {
    receipt.cleanup=await owned.close();
    fs.mkdirSync(process.env.OUT,{recursive:true});
    fs.writeFileSync(path.join(process.env.OUT,'gpu-preflight.json'),JSON.stringify(receipt,null,2)+'\n');
  }
})().catch(error=>{console.error(error);process.exitCode=1});
