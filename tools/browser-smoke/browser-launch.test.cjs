const test=require('node:test');
const assert=require('node:assert/strict');
const {chromium}=require('playwright-core');
const {launchOwnedBrowser,releaseStdioAfterExit}=require('./browser-lifecycle.cjs');

// Run with CHROME pointing to the installed browser. The control launch reads
// Playwright's real defaults, so dependency upgrades cannot silently drop one.
test('the owned browser preserves launch features and disables Windows port randomization',async()=>{
  const options={headless:true,executablePath:process.env.CHROME,args:['--window-size=800,600']};
  const control=await chromium.launchServer({...options,host:'127.0.0.1'});
  releaseStdioAfterExit(control.process());
  let defaults;
  try {
    const prefix='--disable-features=';
    const switches=control.process().spawnargs.filter(arg=>arg.startsWith(prefix));
    assert.equal(switches.length,1);
    defaults=switches[0].slice(prefix.length).split(',');
  } finally {
    try {await control.close()}
    finally {if(control.process().exitCode===null)await control.kill()}
  }
  assert.equal(control.process().exitCode,0);
  const owned=await launchOwnedBrowser(chromium,options);
  try {
    const cdp=await owned.browser.newBrowserCDPSession();
    let args;
    try {({arguments:args}=await cdp.send('Browser.getBrowserCommandLine'))}
    finally {await cdp.detach()}
    const prefix='--disable-features=';
    const switches=args.filter(arg=>arg.startsWith(prefix));
    assert.equal(switches.length,1,'the effective command line has one feature list');
    const expected=new Set(defaults);
    if(process.platform==='win32')expected.add('TcpPortRandomizationWin');
    assert.deepEqual(new Set(switches[0].slice(prefix.length).split(',')),expected);
    assert.ok(args.includes('--window-size=800,600'),'caller arguments survive');
  } finally {
    const cleanup=await owned.close();
    assert.equal(cleanup.mode,'graceful');
    assert.equal(cleanup.exitCode,0);
  }
});
