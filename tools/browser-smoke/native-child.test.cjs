const assert = require('node:assert/strict');
const {spawn} = require('node:child_process');
const {once} = require('node:events');
const test = require('node:test');
const {observeNativeChildren} = require('./native-child.cjs');

test('the observer retains the actual native child identity and removes exited children', async () => {
  const observer = observeNativeChildren();
  const children = [];
  const launch = async args => {
    const child = spawn(process.execPath, ['-e', 'process.stdin.resume()', '--', ...args], {
      stdio: ['pipe', 'ignore', 'ignore'],
    });
    children.push(child);
    await once(child, 'spawn');
    return child;
  };
  try {
    assert.equal(observer.parentPid, process.pid);
    assert.deepEqual(observer.nativeChildren(), []);
    await launch(['peer']);
    assert.deepEqual(observer.nativeChildren(), []);
    const native = await launch(['--desktop']);
    assert.deepEqual(observer.nativeChildren(), [{
      pid: native.pid, executable: process.execPath, arguments: native.spawnargs,
    }]);
    process.kill(native.pid, 0);
    const exited = once(native, 'exit');
    native.kill();
    await exited;
    assert.deepEqual(observer.nativeChildren(), []);
    observer.close();
    await launch(['--desktop']);
    assert.deepEqual(observer.nativeChildren(), []);
  } finally {
    observer.close();
    await Promise.all(children.filter(child => child.exitCode === null && child.signalCode === null)
      .map(async child => {
        const exited = once(child, 'exit');
        child.kill();
        await exited;
      }));
  }
});
