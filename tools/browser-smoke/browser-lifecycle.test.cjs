const test=require('node:test');
const assert=require('node:assert/strict');
const {spawn}=require('node:child_process');
const {once}=require('node:events');
const {releaseStdioAfterExit}=require('./browser-lifecycle.cjs');

test('an inherited stderr holder cannot keep an exited owner waiting for close',async()=>{
  const script=`const {spawn}=require('node:child_process');
    const holder=spawn(process.execPath,['-e','setTimeout(()=>{},30000)'],
      {detached:true,stdio:['ignore','ignore',process.stderr]});
    holder.unref();process.stdout.write(String(holder.pid)+'\\n',()=>process.exit(0));`;
  const owner=spawn(process.execPath,['-e',script],{stdio:['ignore','pipe','pipe']});
  let holder;
  try {
    holder=Number((await once(owner.stdout,'data'))[0].toString().trim());
    assert.ok(Number.isInteger(holder)&&holder>0);
    const closed=once(owner,'close',{signal:AbortSignal.timeout(5000)});
    await once(owner,'exit');
    assert.equal(owner.exitCode,0);
    assert.equal(await Promise.race([closed.then(()=>true),new Promise(resolve=>setTimeout(()=>resolve(false),50))]),
      false,'the real inherited pipe reproduces the wait after process exit');
    releaseStdioAfterExit(owner);
    const result=await closed;
    assert.deepEqual(result,[0,null]);
    assert.doesNotThrow(()=>process.kill(holder,0),'releasing the parent pipe does not kill its holder');
  } finally {
    if(holder)process.kill(holder,'SIGTERM');
    if(owner.exitCode===null)owner.kill('SIGKILL');
  }
});

test('stdio stays intact while its process is alive and the real nonzero exit is preserved',async()=>{
  const owner=spawn(process.execPath,['-e',
    "setTimeout(()=>process.stderr.write('still alive\\n',()=>process.exit(3)),30)"],
  {stdio:['ignore','pipe','pipe']});
  const lines=[];
  owner.stderr.on('data',data=>lines.push(data.toString()));
  releaseStdioAfterExit(owner);
  assert.deepEqual(await once(owner,'close',{signal:AbortSignal.timeout(5000)}),[3,null]);
  assert.equal(lines.join(''),'still alive\n');
});
