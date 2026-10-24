// Ordinary owned Sack input proves the shared GPU editor using real browser input.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {chromium}=require(path.resolve(process.env.PW));
const {launchOwnedBrowser}=require('./browser-lifecycle.cjs');
const ui=require('./gpu-ui.cjs');
const {startNewGame}=require('./menu-journey.cjs');
const output=path.resolve(process.env.OUT);
const url=process.env.URL||'http://127.0.0.1:4181/';
const field='panel:sack-rename:name';
const receipt={passed:false,artifact:process.env.ARTIFACT,checks:[],errors:[]};
fs.mkdirSync(output,{recursive:true});
async function native(page){return page.evaluate(()=>{
  const input=document.querySelector('[data-gpu-text-input]');
  return {text:input?.value,start:input?.selectionStart,end:input?.selectionEnd,
    direction:input?.selectionDirection,focused:document.activeElement===input};
})}
async function selected(page,start,end){
  await page.waitForFunction(({start,end})=>{
    const input=document.querySelector('[data-gpu-text-input]');
    return input?.selectionStart===start&&input?.selectionEnd===end;
  },{start,end});
  await ui.focused(page,field);
}
async function paintedSelection(page,count){
  return ui.waitForUi(page,state=>state.widgets.filter(widget=>widget.id.startsWith(`${field}/selection:`)).length===count,
    `${count} actual painted native selection rectangles`);
}
(async()=>{
  const owned=await launchOwnedBrowser(chromium,{headless:true,
    executablePath:process.env.CHROME||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    args:['--use-angle=metal','--ignore-gpu-blocklist']});
  const context=await owned.browser.newContext({viewport:{width:1600,height:900},
    permissions:['clipboard-read','clipboard-write']});
  const page=await context.newPage();
  const observed=ui.watchErrors(page);
  const input=await context.newCDPSession(page);
  try{
    await observed.ready;
    await page.addInitScript(()=>{
      const events=[];Object.defineProperty(window,'__nativeEditorEvents',{value:events});
      for(const type of ['compositionstart','compositionupdate','compositionend','beforeinput','input','copy','cut','paste']){
        document.addEventListener(type,event=>{
          if(!event.target?.hasAttribute?.('data-gpu-text-input'))return;
          events.push({type,trusted:event.isTrusted,data:event.data??null,inputType:event.inputType??null,
            composing:event.isComposing??null,value:event.target.value,
            start:event.target.selectionStart,end:event.target.selectionEnd});
        },true);
      }
    });
    await page.goto(url);await ui.ready(page);
    await startNewGame(page,{name:'Editor'});
    const fomentius=await ui.byLabel(page,'Fomentius',{role:'button',prefix:'hud:shortcut:'});
    await ui.click(page,fomentius.id);await ui.widget(page,'panel:service');
    const offer=await ui.byLabel(page,/^Buy Sack for \d+ gold$/,{role:'button',prefix:'panel:shop:'});
    await ui.click(page,offer.id);await ui.click(page,offer.id);
    const sack=await ui.byLabel(page,/^Sack, quantity 1$/,{role:'button',prefix:'panel:item:'});
    await ui.click(page,'panel:close');await ui.noWidget(page,'panel:service');
    await page.keyboard.press('KeyI');await ui.widget(page,'panel:surface:inventory');
    await ui.click(page,sack.id);await ui.click(page,'panel:rename-sack');
    await ui.focused(page,field);
    receipt.checks.push('ordinary paid Sack purchase and real rename-field focus');

    await page.keyboard.press('End');await selected(page,4,4);
    await page.keyboard.press('Tab');await ui.focused(page,'panel:sack-rename:save');
    await page.keyboard.press('Shift+Tab');await ui.focused(page,field);
    const tabSelection=await native(page);
    assert.deepEqual([tabSelection.start,tabSelection.end],[0,4],
      'native Tab entry selects the complete singleline value');
    receipt.checks.push('actual Tab into a singleline field selects all without changing its value');

    await ui.installController(page);await ui.controller(page,[]);await ui.nextFrame(page);
    await ui.controller(page,[0]);await ui.nextFrame(page);
    await ui.focused(page,field);await ui.widget(page,'panel:sack-rename');
    await ui.value(page,field,'Sack');
    await ui.controller(page,[]);await ui.controller(page,[13]);
    await ui.waitForUi(page,state=>state.widgets.some(widget=>widget.focused
      && ['panel:sack-rename:save','panel:sack-rename:cancel'].includes(widget.id)),
      'controller Down leaves the field for a genuine spatial action',{timeout:3000});
    await ui.controller(page,[]);await ui.click(page,field);await ui.focused(page,field);
    receipt.checks.push('controller A clicks without Enter submission; Down moves spatial focus out of the field');

    const joined='تراث 🧑‍💻é';
    await ui.fill(page,field,joined);await ui.value(page,field,joined);
    await page.screenshot({path:path.join(output,'joined-script.png')});
    await page.keyboard.press('ControlOrMeta+Z');await ui.value(page,field,'Sack');
    await page.keyboard.press('ControlOrMeta+Shift+Z');await ui.value(page,field,joined);
    receipt.checks.push('full Arabic/ZWJ/combining text accepts, undo restores original, redo restores full Unicode');

    await ui.fill(page,field,'abאבcd');
    await page.keyboard.press('Home');await page.keyboard.press('ArrowRight');
    await page.keyboard.press('Shift+ArrowRight');await page.keyboard.press('Shift+ArrowRight');
    await selected(page,1,3);
    const bidi=await paintedSelection(page,2);
    const rectangles=bidi.widgets.filter(widget=>widget.id.startsWith(`${field}/selection:`))
      .map(ui.bounds).sort((a,b)=>a.x-b.x);
    assert.ok(rectangles[0].x+rectangles[0].width<rectangles[1].x,'mixed-bidi selection retains its real visual gap');
    await page.screenshot({path:path.join(output,'bidi-selection.png')});
    const hebrew=rectangles[1];
    await page.mouse.click(hebrew.x+hebrew.width-1,hebrew.y+hebrew.height/2);
    await selected(page,2,2);
    const weakFrame=await ui.waitForUi(page,state=>{
      const widget=ui.findWidget(state,`${field}/caret`);if(!widget)return false;
      const caret=ui.bounds(widget);return Math.abs(caret.x+caret.width/2-hebrew.x-hebrew.width)<2;
    },'pointer selection paints the actual weak bidi caret edge');
    const weakCaret=ui.bounds(ui.findWidget(weakFrame,`${field}/caret`));
    await page.keyboard.press('ArrowLeft');await selected(page,1,1);
    const movedFrame=await ui.waitForUi(page,state=>{
      const widget=ui.findWidget(state,`${field}/caret`);return widget&&ui.bounds(widget).x<weakCaret.x;
    },'native ArrowLeft selectionchange paints the moved GPU caret');
    const moved=ui.bounds(ui.findWidget(movedFrame,`${field}/caret`));
    receipt.checks.push({bidiSelection:rectangles,weakCaret,moved});

    await ui.fill(page,field,joined);await page.keyboard.press('ControlOrMeta+A');
    await page.keyboard.press('ControlOrMeta+C');
    assert.equal(await page.evaluate(()=>navigator.clipboard.readText()),joined);
    await page.keyboard.press('ControlOrMeta+X');await ui.value(page,field,'');
    await page.keyboard.press('ControlOrMeta+V');await ui.value(page,field,joined);
    await page.keyboard.press('ControlOrMeta+Z');await ui.value(page,field,'');
    await page.keyboard.press('ControlOrMeta+Z');await ui.value(page,field,joined);
    await selected(page,0,joined.length);
    receipt.checks.push('trusted copy/cut/paste and separate native undo transactions preserve full text and selection');

    await ui.fill(page,field,'');
    await input.send('Input.imeSetComposition',{text:'にほ',selectionStart:0,selectionEnd:0});
    await ui.value(page,field,'にほ');
    const composition=await ui.waitForUi(page,state=>state.widgets.some(widget=>widget.id.startsWith(`${field}/composition:`)),
      'actual GPU composition underline with its caret inside preedit');
    receipt.composition=composition.widgets.filter(widget=>widget.id.startsWith(`${field}/composition:`));
    await page.screenshot({path:path.join(output,'native-ime-preedit.png')});
    await input.send('Input.imeSetComposition',{text:'日本語',selectionStart:3,selectionEnd:3});
    await ui.value(page,field,'日本語');
    await input.send('Input.insertText',{text:'日本語'});
    await ui.value(page,field,'日本語');
    await ui.waitForUi(page,state=>!state.widgets.some(widget=>widget.id.startsWith(`${field}/composition:`)),
      'native composition commit retires its real underline');
    await page.keyboard.press('ControlOrMeta+Z');await ui.value(page,field,'');
    await page.keyboard.press('ControlOrMeta+Shift+Z');await ui.value(page,field,'日本語');
    receipt.checks.push('native browser IME preedit/commit is visible, retains internal caret and undoes as one command');

    await ui.fill(page,field,'a'.repeat(31)+'🧙');await ui.value(page,field,'a'.repeat(31));
    await ui.fill(page,field,joined);await page.keyboard.press('Enter');
    await ui.noWidget(page,'panel:sack-rename');
    await ui.waitForUi(page,state=>ui.findWidget(state,sack.id)?.label===`${joined}, quantity 1`,'one accepted rename from native Enter');
    receipt.checks.push('UTF16 admission avoids surrogate splitting; native Enter submits one genuine rename');
    await ui.assertGpuOnly(page);await ui.settleErrors(page);assert.deepEqual(observed.errors,[]);
    receipt.nativeEvents=await page.evaluate(()=>__nativeEditorEvents);
    assert.ok(receipt.nativeEvents.some(event=>event.type==='compositionstart'&&event.trusted));
    const commit=receipt.nativeEvents.find(event=>event.type==='compositionend'&&event.data==='日本語');
    assert.ok(commit,'the actual browser IME commit ends composition');
    assert.ok(receipt.nativeEvents.some(event=>event.type==='input'&&event.composing&&event.trusted));
    // Unchanged native textarea oracle on this Chrome emits the protocol's end
    // event with isTrusted=false; start/update/input remain trusted. No app event
    // is manufactured, and the final accepted value/underline/undo are asserted.
    receipt.imeQualification={method:'Chromium Input domain',commitEndTrusted:commit.trusted,
      oracle:'native-ime-commit-oracle.json',hardwareCandidateWindow:false};
    receipt.passed=true;
  }catch(error){
    receipt.failure=error.stack;receipt.native=await native(page).catch(failure=>({error:failure.message}));
    receipt.ui=await ui.readUi(page).catch(failure=>({error:failure.message}));
    receipt.nativeEvents=await page.evaluate(()=>window.__nativeEditorEvents||[]).catch(()=>[]);
    await page.screenshot({path:path.join(output,'failure.png')}).catch(()=>{});throw error;
  }finally{
    await ui.settleErrors(page);receipt.errors=observed.errors;
    await input.detach().catch(()=>{});
    try{receipt.cleanup=await owned.close()}catch(error){receipt.cleanup=error.cleanup;throw error}
    finally{fs.writeFileSync(path.join(output,'receipt.json'),JSON.stringify(receipt,null,2));}
  }
  console.log(JSON.stringify(receipt));
})().catch(error=>{console.error(error);process.exitCode=1});
