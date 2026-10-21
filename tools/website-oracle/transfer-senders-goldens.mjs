import { loadKernel,writeFixture } from './oracle.mjs'
const {GameWelcomeSender}=await loadKernel('protocol/game-welcome-transfer.ts')
const {GameSaveCheckpointSender}=await loadKernel('protocol/game-save-checkpoint-transfer.ts')
function runs(text) {
  const output=[]
  for(let index=0;index<text.length;index++) {
    const unit=text.charCodeAt(index),last=output.at(-1)
    if(last?.[0]===unit)last[1]++
    else output.push([unit,1])
  }
  return output
}
const chunk=message=>({kind:'chunk',data:runs(message.data),offset:message.offset,totalLength:message.totalLength})
const welcomes=[]
const originalTimeout=globalThis.setTimeout,originalClear=globalThis.clearTimeout
try {
  for(const [name,length,steps] of [
    ['ordered-prefix',73729,[{kind:'start'},{kind:'defer',value:'first'},{kind:'ack',offset:32768},
      {kind:'ack',offset:32768},{kind:'advance',ms:4999},{kind:'ack',offset:65536},
      {kind:'defer',value:'second'},{kind:'ack',offset:73729},{kind:'advance',ms:6000}]],
    ['only-progress-refreshes',73729,[{kind:'start'},{kind:'advance',ms:4000},
      {kind:'ack',offset:8192},{kind:'advance',ms:4000},{kind:'ack',offset:8192},{kind:'advance',ms:1000}]],
    ['cancelled-prefix',73729,[{kind:'start'},{kind:'defer',value:'discarded'},{kind:'close'},{kind:'advance',ms:10000}]],
    ['invalid-ack',73729,[{kind:'start'},{kind:'ack',offset:1},{kind:'advance',ms:5000}]],
  ]) {
    let now=0,next=0
    const timers=new Map(),events=[],results=[]
    globalThis.setTimeout=(fn,delay)=>{const id=++next;timers.set(id,{at:now+delay,fn});return id}
    globalThis.clearTimeout=id=>timers.delete(id)
    const sender=new GameWelcomeSender('w'.repeat(length),payload=>{
      if(payload.startsWith('{'))events.push(chunk(JSON.parse(payload)))
      else events.push({kind:'sideband',payload})
    },length=>events.push({kind:'complete',length}),()=>events.push({kind:'timeout'}))
    for(const step of steps) {
      const before=events.length
      let error=null
      try {
        if(step.kind==='start')sender.start()
        if(step.kind==='defer')sender.defer(step.value)
        if(step.kind==='ack')sender.acknowledge(step.offset)
        if(step.kind==='close')sender.close()
        if(step.kind==='advance') {
          now+=step.ms
          for(const [id,timer] of [...timers])if(timer.at<=now){timers.delete(id);timer.fn()}
        }
      }catch(reason){error=reason.message}
      results.push({error,events:events.slice(before),pending:timers.size>0})
    }
    sender.close()
    welcomes.push({name,length,steps,results})
  }
} finally {globalThis.setTimeout=originalTimeout;globalThis.clearTimeout=originalClear}
const saves=[]
for(const [name,steps] of [
  ['newest-pending-and-atomic',[
    {kind:'publish',sequence:1,length:73729,char:'a',mode:'background'},
    {kind:'publish',sequence:2,length:73729,char:'b',mode:'background'},
    {kind:'publish',sequence:3,length:73729,char:'c',mode:'background'},
    {kind:'ack',sequence:2,offset:8192},{kind:'ack',sequence:4,offset:8192},
    {kind:'ack',sequence:1,offset:32768},{kind:'ack',sequence:1,offset:65536},{kind:'ack',sequence:1,offset:73729},
    {kind:'ack',sequence:1,offset:73729},
    {kind:'publish',sequence:4,length:73729,char:'d',mode:'atomic'},
    {kind:'ack',sequence:3,offset:8192},{kind:'publish',sequence:4,length:2,char:'e',mode:'background'},
  ]],
  ['short-progress-replaces-stream',[
    {kind:'publish',sequence:1,length:73729,char:'a',mode:'background'},
    {kind:'publish',sequence:2,length:73729,char:'b',mode:'background'},
    {kind:'publish',sequence:3,length:10,char:'c',mode:'background'},
    {kind:'ack',sequence:1,offset:8192},{kind:'ack',sequence:4,offset:8192},
  ]],
  ['threshold-counts-units',[
    {kind:'publish',sequence:1,length:65536,char:'é',mode:'background'},
    {kind:'publish',sequence:2,length:65537,char:'é',mode:'background'},
    {kind:'close'},{kind:'ack',sequence:2,offset:8192},
  ]],
]) {
  const events=[],results=[]
  const sender=new GameSaveCheckpointSender(message=>events.push(message.type==='server-save-checkpoint'
    ?{kind:'complete',save:runs(message.save),reason:message.reason,sequence:message.sequence}
    :{...chunk(message),reason:message.reason,sequence:message.sequence}))
  for(const step of steps) {
    const before=events.length
    let error=null
    try {
      if(step.kind==='publish')sender.publish({type:'server-save-checkpoint',save:step.char.repeat(step.length),reason:'progress',sequence:step.sequence},step.mode)
      if(step.kind==='ack')sender.acknowledge({sequence:step.sequence,nextOffset:step.offset})
      if(step.kind==='close')sender.close()
    }catch(reason){error=reason.message}
    results.push({error,events:events.slice(before)})
  }
  sender.close();saves.push({name,steps,results})
}
writeFixture('transfer-senders-goldens.json','transfer-senders-goldens.mjs',
  'current game-welcome-transfer.ts + game-save-checkpoint-transfer.ts; actual source timers and send callbacks',{welcomes,saves})
