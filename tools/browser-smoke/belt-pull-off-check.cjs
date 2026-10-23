// Actual pointer input and painted stock burst diagnostics; no game command injection.
const assert = require('node:assert/strict');
const ui = require('./gpu-ui.cjs');

async function checkNativeBeltPullOff(page, slot = 0) {
  const id=`hud:belt:${slot}`;
  const button=await ui.widget(page,id);
  assert.equal(button.enabled,true,'College spells remain removable');
  const projection=JSON.parse(button.value);
  assert.equal(projection.castEligible,false,'the actual College secondary cannot cast');
  const scale=projection.inputScale;
  assert.ok(scale>0);
  const center=ui.center(ui.bounds(button));
  const inputCount=()=>page.evaluate(()=>window.wasmBindings.gameplay_input_state()[8]);
  const before=await inputCount();
  await page.mouse.move(center.x,center.y);await page.mouse.down();
  await page.mouse.move(center.x,center.y-50*scale);await page.mouse.up();
  await ui.widget(page,id);
  assert.equal(await inputCount(),before,'the exact 50-unit boundary keeps the assignment and never casts');

  await page.evaluate(()=>{
    const receipt=[];const seen=new Set();
    window.beltPullOffReceipt=receipt;window.beltPullOffObserving=true;
    const sample=()=>{const read=window.wasmBindings?.gpu_ui_state;if(read){for(const widget of JSON.parse(read()).widgets){if(!widget.id.startsWith('hud:belt-burst:')||seen.has(widget.id))continue;seen.add(widget.id);receipt.push({id:widget.id,...JSON.parse(widget.value)});}}
      if(window.beltPullOffObserving)requestAnimationFrame(sample);
    };requestAnimationFrame(sample);
  });
  try {
    await page.mouse.move(center.x,center.y);await page.mouse.down();
    await page.mouse.move(center.x,center.y-50.25*scale);await page.mouse.up();
    await ui.noWidget(page,id);
    await page.waitForFunction(()=>window.beltPullOffReceipt.length>0);
    const bursts=await page.evaluate(()=>window.beltPullOffReceipt);
    assert.equal(bursts.length,1,'one pull-off paints one cosmetic burst');
    assert.equal(bursts[0].smoke,24);assert.ok([3,4].includes(bursts[0].spokes));
    await ui.noWidget(page,bursts[0].id);
    assert.equal(await inputCount(),before,'unbinding never activates the removed spell');
    return {slot,threshold:50,acceptedBeyond:50.25,bursts};
  } finally {await page.evaluate(()=>{window.beltPullOffObserving=false;});}
}
module.exports={checkNativeBeltPullOff};
