"""Own the unchanged server/browser with private request and native CPU observations."""
from pathlib import Path
import importlib.util
import json
import os
import subprocess
import sys
import threading
import time
import urllib.request

root = Path.cwd(); output = root / os.environ.get('OBSERVE_SERVER_OUT', 'reports/browser-server'); output.mkdir(parents=True, exist_ok=True)
stop = threading.Event()
spec = importlib.util.spec_from_file_location('native_counters', root / 'tools/browser-smoke/observe-windows-processes.py')
native = importlib.util.module_from_spec(spec); spec.loader.exec_module(native)
first = native.counters()
assert any(p['pid'] == os.getpid() for p in first['processes']), 'Native process counters did not include the sampler'
(output / 'counter-preflight.json').write_text(json.dumps(first, indent=2)+'\n')
sampler = native.start(output / 'processes.jsonl', stop)

def probe():
    with (output / 'head-probes.jsonl').open('w') as log:
        while not stop.is_set():
            start = time.monotonic(); row = {'time': time.time()}
            try:
                request = urllib.request.Request('http://127.0.0.1:4181/assets/stock/loader-logo.png', method='HEAD')
                with urllib.request.urlopen(request, timeout=10) as response:
                    row['status'] = response.status
            except Exception as error:
                row['error'] = str(error)
            row['seconds'] = time.monotonic() - start
            log.write(json.dumps(row)+'\n'); log.flush(); stop.wait(15)

receipt = {'pass': False}
env = os.environ.copy(); env['SERVER_OBSERVATION'] = str(output)
with (output / 'server.log').open('w') as log:
    server = subprocess.Popen([sys.executable, 'tools/browser-smoke/observe-server.py', '4181', '--bind', '127.0.0.1', '--directory', 'web/dist'], env=env, stdout=log, stderr=subprocess.STDOUT)
    prober = None
    try:
        deadline = time.monotonic()+30
        while True:
            if server.poll() is not None:
                raise RuntimeError(f'Server exited before readiness: {server.returncode}')
            try:
                with urllib.request.urlopen('http://127.0.0.1:4181/', timeout=1) as response:
                    assert response.status == 200
                break
            except OSError:
                if time.monotonic() >= deadline:
                    raise
                time.sleep(.1)
        prober = threading.Thread(target=probe, daemon=True); prober.start()
        env.update(PW=str(root/'node_modules/playwright-core'), CHROME=str(Path(os.environ['ProgramFiles'])/'Google/Chrome/Application/chrome.exe'), URL='http://127.0.0.1:4181/', OUT=str(output/'browser'), OBSERVE_LOADING_JOURNEY='./observe-reload.cjs')
        result = subprocess.run(['node', 'tools/browser-smoke/observe-loading.cjs'], env=env)
        receipt['browserDriverExit'] = result.returncode; receipt['pass'] = result.returncode == 0
    finally:
        stop.set()
        receipt['serverExitedBeforeCleanup'] = server.poll() is not None
        if server.poll() is None:
            server.terminate()
        receipt['serverExit'] = server.wait(timeout=15)
        sampler.join(timeout=10)
        if prober:
            prober.join(timeout=12)
        (output/'driver-receipt.json').write_text(json.dumps(receipt, indent=2)+'\n')
sys.exit(0 if receipt['pass'] else 1)
