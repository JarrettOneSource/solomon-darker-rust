// Private observer: timestamp admission and transfer events without recording credentials.
const fs = require('node:fs');
const path = require('node:path');
exports.install = async (page, output) => {
  const record = value => fs.appendFileSync(path.join(output,'admission-trace.jsonl'),
    JSON.stringify({receivedAt:Date.now(),...value})+'\n');
  page.on('console', message => {
    const prefix = 'ADMISSION_TRACE ';
    if (message.text().startsWith(prefix)) record(JSON.parse(message.text().slice(prefix.length)));
  });
  let socketId = 0;
  page.on('websocket', socket => {
    const id = ++socketId;
    record({side:'node',kind:'socket',id});
    for (const event of ['framesent','framereceived']) socket.on(event, ({payload}) => {
      const data = JSON.parse(payload.toString());
      if (['hello','inspect_session','welcome_chunk_ack','welcome_chunk','welcome',
        'session_status','error','save_before_leave','save_committed'].includes(data.type)) {
        record({side:'node',kind:event,id,type:data.type,offset:data.offset,nextOffset:data.next_offset,
          total:data.total_length,resume:!!data.resume,bytes:payload.length});
      }
    });
    socket.on('close',()=>record({side:'node',kind:'close',id}));
    socket.on('socketerror',error=>record({side:'node',kind:'error',id,error:String(error)}));
  });
  await page.addInitScript(() => {
    const Original = window.WebSocket;
    let nextId = 0;
    const record = value => console.debug('ADMISSION_TRACE '+JSON.stringify({
      side:'page',at:Date.now(),time:performance.now(),document:performance.timeOrigin,...value}));
    window.WebSocket = class ObservedSocket extends Original {
      constructor(...args) {
        super(...args);
        this.observedId = ++nextId;
        record({kind:'construct',id:this.observedId});
        for (const kind of ['open','close','error']) this.addEventListener(kind,event =>
          record({kind,id:this.observedId,code:event.code,reason:event.reason}));
        this.addEventListener('message',event => {
          if (typeof event.data !== 'string') return;
          const data = JSON.parse(event.data);
          if (['welcome_chunk','welcome','session_status','error','save_committed'].includes(data.type)) {
            record({kind:'message',id:this.observedId,type:data.type,offset:data.offset,
              total:data.total_length,bytes:event.data.length});
          }
        });
        setTimeout(()=>record({kind:'five-second-observation',id:this.observedId,state:this.readyState}),5000);
      }
      send(text) {
        if (typeof text === 'string') {
          const data = JSON.parse(text);
          if (['hello','inspect_session','welcome_chunk_ack','save_before_leave'].includes(data.type)) {
            record({kind:'send',id:this.observedId,type:data.type,resume:!!data.resume,
              nextOffset:data.next_offset,bytes:text.length});
          }
        }
        return super.send(text);
      }
    };
  });
};
