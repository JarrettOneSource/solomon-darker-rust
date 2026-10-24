// Source c140 menu/navigation.rs: scoped spatial movement and native linear wrap.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {chromium}=require(path.resolve(process.env.PW));
const ui=require('./gpu-ui.cjs');
const {launchOwnedBrowser}=require('./browser-lifecycle.cjs');
const output=process.env.OUT||'/tmp';
fs.mkdirSync(output,{recursive:true});

(async()=>{
 const receipt={checks:[],errors:[],pass:false};
 const owned=await launchOwnedBrowser(chromium,{headless:true,
  executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  args:['--use-angle=metal','--ignore-gpu-blocklist']});
 let page;
 try{
  page=await owned.browser.newPage({viewport:{width:1600,height:900},ignoreHTTPSErrors:process.env.LOCAL_TLS==='1'});
  const observed=ui.watchErrors(page);await observed.ready;receipt.errors=observed.errors;
  await page.goto(process.env.URL||'http://127.0.0.1:4181/');await ui.ready(page);
  if(ui.findWidget(await ui.readUi(page),'prompt-secondary'))await ui.click(page,'prompt-secondary');
  await ui.installController(page);await ui.controller(page);await ui.nextFrame(page);
  await ui.focused(page,'play');
  const step=async(button,id)=>{
   await ui.controller(page,[button]);
   await ui.focused(page,id);
   await ui.controller(page);
  };
  for(const [button,id] of [[12,'hall'],[13,'play'],[14,'hall'],[15,'play'],[4,'hall'],[5,'play']]){
   receipt.pending={button,id};
   await step(button,id);
  }
  receipt.checks.push('all four controller edge directions and bumpers wrap within the Title root');
  await page.screenshot({path:path.join(output,'native-title-wrap.png')});

  // Source MenuNavigation scopes controllers; physical Tab retains normal page order.
  const title=(await ui.readUi(page)).widgets.filter(widget=>widget.enabled&&['play','explore','settings','hall'].includes(widget.id))
   .sort((a,b)=>a.z-b.z).map(widget=>widget.id);
  for(const id of [...title.slice(1),'fullscreen/toggle',title[0]]){
   receipt.pending={keyboard:'Tab',id};
   await page.keyboard.press('Tab');await ui.focused(page,id);
  }
  receipt.checks.push('ordinary physical Tab retains the real fullscreen root and native page wrap');

  await ui.click(page,'settings');await ui.focused(page,'settings-range:sound');
  await ui.controller(page);await ui.nextFrame(page);
  for(const [button,id] of [[12,'settings-back'],[13,'settings-range:sound'],[4,'settings-back'],[5,'settings-range:sound']]){
   receipt.pending={button,id};await step(button,id);
  }
  receipt.checks.push('the active Settings root owns fallback and bumper wrap with the Title disabled behind it');
  await page.screenshot({path:path.join(output,'native-settings-wrap.png')});
  await ui.assertGpuOnly(page);await ui.settleErrors(page);assert.deepEqual(receipt.errors,[]);
  delete receipt.pending;receipt.behaviorPassed=true;
 }catch(error){
  receipt.failure=String(error.stack||error);process.exitCode=1;
  if(page&&!page.isClosed()){
   try{receipt.actual=await ui.readUi(page);await page.screenshot({path:path.join(output,'navigation-failure.png')})}
   catch(observationError){receipt.observationFailure=String(observationError)}
  }
 }finally{
  try{if(page&&!page.isClosed())await ui.settleErrors(page)}
  finally{
   try{receipt.browserCleanup=await owned.close();receipt.pass=receipt.behaviorPassed===true&&receipt.errors.length===0}
   catch(error){receipt.browserCleanup=error.cleanup||{failure:String(error)};process.exitCode=1}
   fs.writeFileSync(path.join(output,'receipt.json'),`${JSON.stringify(receipt,null,2)}\n`);
  }
 }
 console.log(JSON.stringify(receipt));
})().catch(error=>{console.error(error);process.exitCode=1});
