// Actual glyph layout bounds keep wrapping and the phone font floor observable.
const assert=require('node:assert/strict');
const ui=require('./gpu-ui.cjs');

function rectangle(rect) {
  assert.ok(Array.isArray(rect)&&rect.length===4,'unclipped GPU paint geometry is retained');
  const [left,top,width,height]=rect;
  return {left,top,width,height,right:left+width,bottom:top+height};
}

async function assertTutorialTextBounds(page) {
  await ui.widget(page,'tutorial:text:heading');
  await ui.widget(page,'tutorial:text:subheading');
  const snapshot=await ui.readUi(page);
  const {width,height}=snapshot.canvas;
  const coarse=await page.evaluate(()=>matchMedia('(pointer:coarse)').matches);
  const selected=snapshot.widgets.filter(widget=>/^hud:skill-binding:(12|16|20)$/.test(widget.id))
    .map(widget=>rectangle(widget.rect));
  const controls=snapshot.widgets.filter(widget=>/^hud:belt:/.test(widget.id)
    ||['hud:inventory','hud:skills'].includes(widget.id)
    ||(widget.id==='hud:joystick:movement'&&widget.enabled))
    .map(widget=>rectangle(widget.rect)).filter(box=>box.top>=height/2);
  // Hit bounds are clipped for input. Full native glyph bounds must independently fit.
  const heading=rectangle(ui.findWidget(snapshot,'tutorial:text:heading').paintRect);
  const subheading=rectangle(ui.findWidget(snapshot,'tutorial:text:subheading').paintRect);
  const bounds={width,height,coarse,selected,controls,heading,subheading};
  for(const name of ['heading','subheading']) {
    const box=bounds[name];
    assert.ok(box.top>=0&&box.bottom<=height,`${name} stays vertically in the viewport`);
    assert.ok(box.left>=0&&box.right<=width,`${name} stays horizontally in the viewport`);
  }
  assert.ok(subheading.top>=heading.bottom+4,'subheading clears every wrapped heading line');
  if(coarse&&width<1000) {
    for(const name of ['heading','subheading']) {
      for(const control of controls) {
        const text=bounds[name];
        const clear=text.bottom<=control.top||text.top>=control.bottom
          ||text.right<=control.left||text.left>=control.right;
        assert.ok(clear,`${name} clears the actual lower controls`);
      }
    }
    for(const button of selected) {
      assert.ok(button.width>=43.99&&button.height>=43.99,'selectors retain a real finger-sized target');
      assert.ok(button.top>=0&&button.left>=0&&button.right<=width,'selector targets stay in the viewport');
      assert.ok(heading.top>=button.bottom+7,'lesson clears the visible selector strip');
    }
  }
  return bounds;
}

module.exports={assertTutorialTextBounds};
