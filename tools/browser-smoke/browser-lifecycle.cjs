// The public Playwright server owns Chrome; the client owns only its connection.
// Keep both close results, including a timeout, fatal even if termination succeeds.
const os=require('node:os');

async function prepareSoftwareRenderer(browser,options) {
  if(os.platform()!=='win32'||!options.args?.includes('--use-angle=swiftshader'))return;
  const cdp=await browser.newBrowserCDPSession();
  try {
    await cdp.send('SystemInfo.getInfo');
    const {processInfo}=await cdp.send('SystemInfo.getProcessInfo');
    const gpu=processInfo.filter(process=>process.type.toLowerCase()==='gpu');
    if(gpu.length!==1)throw new Error('Expected one owned software GPU process');
    // Chrome promotes its Windows GPU process above normal. SwiftShader runs
    // on the CPU and can then starve the local server and browser driver.
    os.setPriority(gpu[0].id,os.constants.priority.PRIORITY_NORMAL);
  } finally {await cdp.detach()}
}

async function bounded(promise,label,milliseconds) {
  let timer;
  try {
    return await Promise.race([promise,new Promise((resolve,reject)=>{
      timer=setTimeout(()=>reject(new Error(`${label} timed out`)),milliseconds);
    })]);
  } finally {clearTimeout(timer)}
}

function releaseStdioAfterExit(child) {
  // Crashpad/updater descendants may inherit stderr after Chrome has exited.
  // Node's close event waits for those pipes; no live browser owns them now.
  const release=()=>{for(const stream of child.stdio)if(stream)stream.destroy()};
  if(child.exitCode!==null||child.signalCode!==null)release();
  else child.once('exit',release);
}

async function launchOwnedBrowser(chromium,options) {
  const server=await chromium.launchServer({...options,host:'127.0.0.1'});
  releaseStdioAfterExit(server.process());
  let browser;
  try {browser=await chromium.connect(server.wsEndpoint())}
  catch(error) {
    try {await bounded(server.kill(),'unconnected owned Chrome termination',5000)}
    catch(cleanupError) {throw new AggregateError([error,cleanupError],'Browser connection and cleanup failed')}
    throw error;
  }
  const process=server.process();
  let stderrTail='';
  process.stderr?.on('data',chunk=>{stderrTail=(stderrTail+chunk.toString()).slice(-16384)});
  async function close() {
    const report={pid:process.pid,connectedBefore:browser.isConnected(),errors:[]};
    try {await bounded(browser.close(),'Playwright connection close',5000)}
    catch(error) {report.errors.push(error.message)}
    try {await bounded(server.close(),'owned Chrome close',15000);report.mode='graceful'}
    catch(error) {
      report.errors.push(error.message);report.mode='forced';
      try {await bounded(server.kill(),'owned Chrome termination',5000)}
      catch(terminationError) {report.errors.push(terminationError.message)}
    }
    report.connectedAfter=browser.isConnected();
    report.exitCode=process.exitCode;report.signalCode=process.signalCode;
    if(report.connectedAfter)report.errors.push('Playwright connection remains open');
    if(report.exitCode===null&&report.signalCode===null)report.errors.push('owned Chrome remains running');
    else if(report.exitCode!==0||report.signalCode!==null)report.errors.push('owned Chrome did not exit successfully');
    if(report.errors.length) {
      report.stderrTail=stderrTail;
      const error=new Error(`Browser cleanup failed: ${report.errors.join('; ')}`);
      error.cleanup=report;throw error;
    }
    return report;
  }
  try {await bounded(prepareSoftwareRenderer(browser,options),'software renderer setup',15000)}
  catch(error) {
    try {await close()}
    catch(cleanupError) {throw new AggregateError([error,cleanupError],'Browser setup and cleanup failed')}
    throw error;
  }
  return {browser,close};
}

module.exports={launchOwnedBrowser,releaseStdioAfterExit};
