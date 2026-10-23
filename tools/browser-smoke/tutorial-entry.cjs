// Ordinary Title YES admission and the real first Tutorial lesson. Later lesson
// progression belongs to its own combat/inventory acceptance, never fabricated state.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {chromium}=require(path.resolve(process.env.PW));
const ui=require('./gpu-ui.cjs');
const {launchOwnedBrowser}=require('./browser-lifecycle.cjs');
const {waitForSave,localParticipant}=require('./saved-game.cjs');
const {assertTutorialTextBounds}=require('./tutorial-text-bounds.cjs');
const output=path.resolve(process.env.OUT||'/tmp/tutorial-entry');
const url=process.env.URL||'http://127.0.0.1:4181/';
fs.mkdirSync(output,{recursive:true});
const receipt={passed:false,url,cases:[],errors:[]};
const position=page=>page.evaluate(()=>Array.from(wasmBindings.wizard_state()).slice(0,2));

(async()=>{
  const owned=await launchOwnedBrowser(chromium,{headless:true,
    executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
  const {browser}=owned;
  try {
    for(const touch of [false,true]){
      const name=touch?'touch':'keyboard';
      const context=await browser.newContext({viewport:{width:1600,height:900},
        hasTouch:touch,isMobile:touch,deviceScaleFactor:1});
      const page=await context.newPage();
      const observed=ui.watchErrors(page);
      const result={name,layouts:[]};
      try {
        await observed.ready;
        await ui.observeGpu(page);
        await ui.observeAnnouncements(page);
        await page.goto(url,{waitUntil:'domcontentloaded'});
        await ui.ready(page);
        await ui.surface(page,'title');
        await ui.widget(page,'prompt-primary',{enabled:true});
        await(touch?ui.tap:ui.click)(page,'prompt-primary');
        const record=await waitForSave(page,value=>value.checkpoint.boneyard?.tutorial);
        result.runId=record.checkpoint.run.run_id;
        result.intro=record.checkpoint.boneyard.tutorial;
        assert.equal(localParticipant(record).player.selection.name,'Sirmin');
        await page.screenshot({path:path.join(output,`${name}-intro.png`)});
        const heading=touch?'USE THE LEFT JOYSTICK\nTO MOVE THE WIZARD'
          :'USE YOUR KEYBOARD\nTO MOVE THE WIZARD';
        for(const size of [{width:1600,height:900},{width:844,height:390}]){
          await page.setViewportSize(size);
          const subheading=touch?'Find and confront Solomon Dark':'Move with W, A, S, and D';
          await ui.waitForUi(page,snapshot=>snapshot.canvas.width===size.width
            &&snapshot.canvas.height===size.height
            &&ui.findWidget(snapshot,'tutorial:text:heading')?.label===heading
            &&ui.findWidget(snapshot,'tutorial:text:subheading')?.label===subheading,
          `actual ${name} Movement lesson at ${size.width}x${size.height}`,{timeout:30000});
          const bounds=await assertTutorialTextBounds(page);
          await ui.assertGpuOnly(page);
          result.layouts.push(bounds);
          await page.screenshot({path:path.join(output,`${name}-${size.width}x${size.height}-lesson.png`)});
        }
        const before=await position(page);
        let input;
        if(touch){
          const joystick=ui.bounds(await ui.widget(page,'hud:joystick:movement'));
          const center=ui.center(joystick);
          input=await context.newCDPSession(page);
          await input.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{...center,id:1}]});
          await input.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{
            x:center.x+joystick.width*.4,y:center.y,id:1}]});
        }else await page.keyboard.down('KeyD');
        try {
          await page.waitForFunction(x=>wasmBindings.wizard_state()[0]>x+8,before[0]);
          await ui.noWidget(page,'tutorial:text:heading');
          await ui.noWidget(page,'tutorial:text:subheading');
        }finally{
          if(touch){
            await input.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});
            await input.detach();
          }else await page.keyboard.up('KeyD');
        }
        result.movement={before,after:await position(page)};
        result.announcements=await ui.announcements(page);
        assert.ok(result.announcements.some(value=>value.text.includes('Solomon')
          ||value.text.includes('Sirmin')),'actual Tutorial narration reaches the native announcement adapter');
        await page.screenshot({path:path.join(output,`${name}-movement-acknowledged.png`)});
        await ui.assertGpuOnly(page);
        await ui.settleErrors(page);
        assert.deepEqual(observed.errors,[]);
        receipt.cases.push(result);
      }catch(error){
        receipt.failure=error.stack;
        receipt.failedCase=name;
        receipt.ui=await ui.readUi(page);
        await page.screenshot({path:path.join(output,`${name}-failure.png`)});
        throw error;
      }finally{
        await ui.settleErrors(page);
        receipt.errors.push(...observed.errors);
        await context.close();
      }
    }
    receipt.passed=true;
  }finally{
    try {receipt.cleanup=await owned.close();}
    catch(error){receipt.passed=false;receipt.cleanup=error.cleanup||{error:error.stack};throw error;}
    finally{fs.writeFileSync(path.join(output,'receipt.json'),JSON.stringify(receipt,null,2));}
  }
  console.log(JSON.stringify(receipt));
})().catch(error=>{console.error(error);process.exitCode=1;});
