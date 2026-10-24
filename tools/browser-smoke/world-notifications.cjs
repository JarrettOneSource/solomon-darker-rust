// Observe the actual painted notice and native speech. Callers retain the accepted
// local event that owns its key; this helper never creates or dispatches an event.
const assert=require('node:assert/strict');
const ui=require('./gpu-ui.cjs');

async function assertWorldNotification(page,{key,text,afterAnnouncement=0}) {
  const id=`world-notification:${key}`;
  const snapshot=await ui.waitForUi(page,state=>{
    const row=ui.findWidget(state,id);
    if(!row||row.label!==text)return false;
    const value=JSON.parse(row.value);
    return value.offset>=0&&value.alpha>0;
  },`the accepted ${key} notification becomes fully visible`);
  const notice=ui.findWidget(snapshot,id),presentation=JSON.parse(notice.value);
  assert.equal(notice.role,'text');assert.equal(notice.enabled,false);
  assert.equal(presentation.key,key);
  assert.equal(presentation.clipTop,50);
  assert.equal(presentation.baseline,67+Math.round(presentation.offset));
  const unrounded=Math.max(0,Math.fround(1-Math.max(presentation.offset,0)/250));
  assert.equal(presentation.scale,Math.fround(Number(unrounded.toFixed(2))));
  assert.ok(presentation.alpha>0&&presentation.alpha<=1);
  assert.ok(notice.rect[1]>=presentation.clipTop&&notice.rect[2]>0&&notice.rect[3]>0);
  await ui.announced(page,text,{afterIndex:afterAnnouncement});
  const speech=(await ui.announcements(page)).slice(afterAnnouncement)
    .filter(event=>event.text.split('\n').includes(text));
  assert.equal(speech.length,1,'one accepted local notice announces its actual text once');
  return {frame:snapshot.frame,notice,presentation,speech};
}

module.exports={assertWorldNotification};
