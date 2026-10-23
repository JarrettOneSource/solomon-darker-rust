// Passed directly to ElectronApplication.evaluateHandle, so all dependencies stay
// inside this function. Observe the real child without Windows CIM enumeration.
function observeNativeChildren({BrowserWindow} = {}, stopJournal) {
  const channel = process.getBuiltinModule('node:diagnostics_channel');
  const children = [];
  const wrapped = [];
  let observing = true;
  const observe = ({process: child}) => {
    // The channel fires during construction, before pid and spawnargs exist.
    child.once('spawn', () => {
      const native = child.spawnargs.includes('--desktop');
      if (native) children.push(child);
      const peer = child.spawnargs.some(value => /run-peer-service\.mjs$/.test(value));
      if (!stopJournal || (!native && !peer)) return;
      const kill = child.kill;
      const observedKill = function(signal) {
        if (observing && signal !== 0) {
          const window = BrowserWindow.getAllWindows()[0];
          const url = window && !window.webContents.isDestroyed()
            ? new URL(window.webContents.getURL()) : null;
          process.getBuiltinModule('node:fs').appendFileSync(stopJournal, `${JSON.stringify({
            pid: child.pid, kind: native ? 'native' : 'peer', signal: signal ?? 'SIGTERM',
            gameDocumentLive: url?.protocol === 'solomon-darker:' && url.host === 'app'
              && (url.pathname === '/game' || url.pathname.startsWith('/game/')),
            document: url ? {protocol: url.protocol, path: url.pathname} : null,
          })}\n`);
        }
        return Reflect.apply(kill, this, [signal]);
      };
      child.kill = observedKill;
      wrapped.push({child, kill, observedKill});
    });
  };
  channel.subscribe('child_process', observe);
  return {
    parentPid: process.pid,
    nativeChildren() {
      return children.filter(child => child.exitCode === null && child.signalCode === null)
        .map(child => ({pid: child.pid, executable: child.spawnfile, arguments: [...child.spawnargs]}));
    },
    close() {
      observing = false;
      channel.unsubscribe('child_process', observe);
      for (const {child, kill, observedKill} of wrapped) {
        if (child.kill === observedKill) child.kill = kill;
      }
    },
  };
}

module.exports = {observeNativeChildren};
