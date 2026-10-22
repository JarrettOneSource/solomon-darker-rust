// Passed directly to ElectronApplication.evaluateHandle, so all dependencies stay
// inside this function. Observe the real child without Windows CIM enumeration.
function observeNativeChildren() {
  const channel = process.getBuiltinModule('node:diagnostics_channel');
  const children = [];
  const observe = ({process: child}) => {
    // The channel fires during construction, before pid and spawnargs exist.
    child.once('spawn', () => {
      if (child.spawnargs.includes('--desktop')) children.push(child);
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
      channel.unsubscribe('child_process', observe);
    },
  };
}

module.exports = {observeNativeChildren};
