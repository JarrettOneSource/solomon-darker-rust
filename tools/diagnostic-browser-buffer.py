"""Private Windows observation of the unchanged browser acceptance sequence."""
import collections
import ctypes
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
import time

out = Path('reports/browser-buffer').resolve()
out.mkdir(parents=True, exist_ok=True)
assert sys.platform == 'win32'

class MemoryStatus(ctypes.Structure):
    _fields_ = [('length', ctypes.c_ulong), ('load', ctypes.c_ulong)] + [
        (name, ctypes.c_ulonglong) for name in (
            'physicalTotal', 'physicalAvailable', 'pageFileTotal', 'pageFileAvailable',
            'virtualTotal', 'virtualAvailable', 'extendedVirtualAvailable')]

kernel = ctypes.WinDLL('kernel32', use_last_error=True)
kernel.GlobalMemoryStatusEx.argtypes = [ctypes.POINTER(MemoryStatus)]
kernel.GlobalMemoryStatusEx.restype = ctypes.c_int
stop = threading.Event()
phase = 'setup'

def sample():
    with (out / 'resources.jsonl').open('w', encoding='utf-8') as stream:
        while not stop.is_set():
            memory = MemoryStatus()
            memory.length = ctypes.sizeof(memory)
            ok = kernel.GlobalMemoryStatusEx(ctypes.byref(memory))
            result = subprocess.run(['netstat', '-ano', '-p', 'tcp'], capture_output=True, text=True)
            rows = [line.split() for line in result.stdout.splitlines() if line.lstrip().startswith('TCP ')]
            states = collections.Counter(row[3] for row in rows if len(row) == 5)
            local = [row for row in rows if len(row) == 5 and any(value.endswith(':4181') for value in row[1:3])]
            record = {'time': time.time(), 'phase': phase, 'memoryRead': bool(ok),
                      'memory': {name: getattr(memory, name) for name, _ in MemoryStatus._fields_[1:]},
                      'netstatExit': result.returncode, 'states': dict(states), 'acceptanceConnections': local,
                      'localEndpoints': len({row[1] for row in rows if len(row) == 5})}
            if result.returncode: record['netstatError'] = result.stderr
            stream.write(json.dumps(record) + '\n')
            stream.flush()
            stop.wait(2)

lifecycle = Path('tools/browser-smoke/browser-lifecycle.cjs')
original = lifecycle.read_text(encoding='utf-8')
needle = "  const server=await chromium.launchServer({...options,host:'127.0.0.1'});"
assert original.count(needle) == 1
instrumented = original.replace(needle, "  options={...options,args:[...(options.args||[]),`--log-net-log=${require('node:path').join(globalThis.process.env.OUT,'network.json')}`]};\n" + needle)
lifecycle.write_text(instrumented, encoding='utf-8')
provenance = {'source': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
              'originalLifecycleSha256': hashlib.sha256(original.encode()).hexdigest(),
              'instrumentedLifecycleSha256': hashlib.sha256(instrumented.encode()).hexdigest(),
              'change': 'Add default-mode Chromium network logging only; no application, priority, server, assertions or input changes.',
              'artifactRun': 40466361015,
              'browserFiles': {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(Path('web/dist').iterdir()) if p.is_file()}}
(out / 'source.json').write_text(json.dumps(provenance, indent=2) + '\n')
thread = threading.Thread(target=sample)
thread.start()
results = []
server = None
try:
    with (out / 'server.log').open('w') as stdout, (out / 'server-error.log').open('w') as stderr:
        server = subprocess.Popen([sys.executable, 'scripts/serve-client.py', '4181', '--bind', '127.0.0.1', '--directory', 'web/dist'], stdout=stdout, stderr=stderr)
        for iteration in (1, 2):
            for name, script in [('gameplay', 'smoke.cjs'), ('settings', 'settings-parity.cjs'), ('image-load-failure', 'image-load-failure.cjs')]:
                phase = f'{iteration}-{name}'
                target = out / phase
                target.mkdir()
                env = dict(os.environ, OUT=str(target), URL='http://127.0.0.1:4181/')
                started = time.time()
                with (target / 'driver.log').open('w', encoding='utf-8') as log:
                    child = subprocess.Popen(['node', 'tools/browser-smoke/' + script], env=env, stdout=log, stderr=subprocess.STDOUT)
                    try:
                        code = child.wait(timeout=900)
                        timed_out = False
                    except subprocess.TimeoutExpired:
                        subprocess.run(['taskkill', '/PID', str(child.pid), '/T', '/F'], check=True)
                        code = child.wait(timeout=15)
                        timed_out = True
                results.append({'iteration': iteration, 'journey': name, 'exit': code, 'timedOut': timed_out, 'start': started, 'finish': time.time()})
                (out / 'journeys.json').write_text(json.dumps(results, indent=2) + '\n')
                print(json.dumps(results[-1]), flush=True)
finally:
    phase = 'cleanup'
    if server is not None:
        if server.poll() is None: server.terminate()
        server.wait(timeout=10)
        (out / 'server-cleanup.json').write_text(json.dumps({'pid': server.pid, 'exit': server.returncode, 'running': server.poll() is None}) + '\n')
    stop.set()
    thread.join(timeout=15)
    lifecycle.write_text(original, encoding='utf-8')
    assert not thread.is_alive(), 'Resource observer did not stop'
assert len(results) == 6 and all(row['exit'] == 0 and not row['timedOut'] for row in results), results
