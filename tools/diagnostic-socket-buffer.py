"""Own the private Windows loopback counterfactual and preserve every outcome."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

assert sys.platform == 'win32'
out = Path('reports/socket-buffer').resolve()
site = out / 'site'
site.mkdir(parents=True)
(site / 'index.html').write_text('''<!doctype html><meta charset="utf-8">
<link rel="icon" href="data:,">
<script>
Promise.allSettled(Array.from({length:5},async (_,i)=>{
  const response=await fetch('asset-'+i+'.bin');
  if(!response.ok)throw new Error('HTTP '+response.status);
  const body=await response.arrayBuffer();
  return body.byteLength;
})).then(result=>{globalThis.probeResult=result});
</script>Loopback resource probe
''', encoding='utf-8')
for index in range(5):
    (site / f'asset-{index}.bin').write_bytes(bytes([index + 1]) * 16384)
files = [Path('tools/diagnostic-socket-buffer.py'), Path('tools/diagnostic-socket-buffer.cjs'),
         Path('tools/browser-smoke/browser-lifecycle.cjs'), Path('scripts/serve-client.py'),
         *sorted(site.iterdir())]
source = {'commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
          'sha256': {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in files},
          'scope': 'Only isolated diagnostic launches change TcpPortRandomizationWin. No game or canonical launch configuration changes.'}
(out / 'source.json').write_text(json.dumps(source, indent=2) + '\n')
server = None
with (out / 'server.log').open('w') as stdout, (out / 'server-error.log').open('w') as stderr:
    try:
        server = subprocess.Popen([sys.executable, 'scripts/serve-client.py', '4181',
                                   '--bind', '127.0.0.1', '--directory', str(site)],
                                  stdout=stdout, stderr=stderr)
        child = subprocess.Popen(['node', 'tools/diagnostic-socket-buffer.cjs'])
        try:
            code = child.wait(timeout=1900)
        except subprocess.TimeoutExpired:
            subprocess.run(['taskkill', '/PID', str(child.pid), '/T', '/F'], check=True)
            child.wait(timeout=15)
            (out / 'timeout.json').write_text(json.dumps({
                'pid': child.pid, 'exit': child.returncode, 'timeoutSeconds': 1900}) + '\n')
            raise
    finally:
        if server is not None:
            if server.poll() is None:
                server.terminate()
            server.wait(timeout=15)
            (out / 'server-cleanup.json').write_text(json.dumps({
                'pid': server.pid, 'exit': server.returncode, 'running': server.poll() is None}) + '\n')
sys.exit(code)
