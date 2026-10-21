import { loadKernel, writeFixture } from './oracle.mjs'

const { GameStringSender, GameStringReceiver } = await loadKernel('protocol/game-string-transfer.ts')
const units = text => Array.from({length:text.length},(_,index)=>text.charCodeAt(index))
const runs = text => {
  const result=[]
  for(const unit of units(text)) {
    const last=result.at(-1)
    if(last?.[0]===unit)last[1]++
    else result.push([unit,1])
  }
  return result
}
const encode = chunk => ({...chunk,data:runs(chunk.data)})
const senders=[]
function sender(name,data,steps) {
  const chunks=[],results=[]
  let current
  try {current=new GameStringSender(data,chunk=>chunks.push(encode(chunk)))}
  catch(error) {senders.push({name,data:runs(data),error:error.message});return}
  for(const step of steps) {
    const before=chunks.length
    try {
      let status=null
      if(step.kind==='start')current.start()
      if(step.kind==='ack')status=current.acknowledge(step.offset)
      if(step.kind==='close')current.close()
      results.push({status,chunks:chunks.slice(before)})
    } catch(error) {results.push({error:error.message,chunks:chunks.slice(before)})}
  }
  senders.push({name,data:runs(data),steps,results})
}
sender('empty','',[])
sender('small','a😀z',[{kind:'start'},{kind:'ack',offset:4},{kind:'ack',offset:4},{kind:'start'}])
sender('window-and-cumulative','q'.repeat(8192*8+7),[
  {kind:'start'},{kind:'start'},{kind:'ack',offset:0},{kind:'ack',offset:1},
  {kind:'ack',offset:8192},{kind:'ack',offset:24576},{kind:'ack',offset:8192},
  {kind:'ack',offset:57344},{kind:'ack',offset:65543},{kind:'ack',offset:65544},
])
sender('split-surrogate','x'.repeat(8191)+'😀'+'z'.repeat(8192*4),[
  {kind:'start'},{kind:'ack',offset:32768},{kind:'ack',offset:40961},
])
sender('explicit-close','z'.repeat(8192*6),[
  {kind:'start'},{kind:'close'},{kind:'ack',offset:8192},{kind:'start'},
])
const receivers=[]
function receiver(name,maximumBytes,chunks,steps=chunks.map(()=>false)) {
  const current=new GameStringReceiver(maximumBytes,'fixture'),results=[]
  for(const [index,chunk] of chunks.entries()) {
    try {
      if(steps[index])current.close()
      const value=current.accept(chunk)
      results.push({value:value===null?null:runs(value)})
    }catch(error){results.push({error:error.message})}
  }
  receivers.push({name,maximumBytes,chunks:chunks.map(encode),closeBefore:steps,results})
}
receiver('assembled-surrogate',10000,[
  {data:'x'.repeat(8191)+'\uD83D',offset:0,totalLength:8193},
  {data:'\uDE00',offset:8192,totalLength:8193},
])
receiver('missing-first',10,[{data:'a',offset:1,totalLength:2}])
receiver('empty-data',10,[{data:'',offset:0,totalLength:2}])
receiver('empty-total',10,[{data:'a',offset:0,totalLength:0}])
receiver('too-long-total',1,[{data:'a',offset:0,totalLength:2}])
receiver('too-long-chunk',10000,[{data:'x'.repeat(8193),offset:0,totalLength:8193}])
receiver('mismatch-retains-original',10,[
  {data:'a',offset:0,totalLength:3},{data:'b',offset:1,totalLength:4},
  {data:'c',offset:2,totalLength:3},{data:'bc',offset:1,totalLength:3},
])
receiver('overrun',3,[{data:'ab',offset:0,totalLength:1}])
receiver('utf8-byte-bound',3,[{data:'ééé',offset:0,totalLength:3}])
receiver('isolated-surrogate-bound',2,[{data:'\uD83D',offset:0,totalLength:1}])
receiver('isolated-surrogate-retained',3,[{data:'\uD83D',offset:0,totalLength:1}])
receiver('reset-cancels-prefix',10,[
  {data:'a',offset:0,totalLength:3},{data:'xy',offset:0,totalLength:2},
],[false,true])
writeFixture('string-transfer-goldens.json','string-transfer-goldens.mjs',
  'current protocol/game-string-transfer.ts; lossless UTF-16 run encoding',{senders,receivers})
