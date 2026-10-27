"""Private Windows red/green and retained-trace replay for resident music."""
from pathlib import Path
import hashlib
import json
import re
import subprocess
import sys

assert sys.platform == 'win32'
out = Path('reports/browser-buffer')
out.mkdir(parents=True, exist_ok=True)

def run(name, command):
    result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                            text=True, encoding='utf-8', timeout=180)
    (out / name).write_text(result.stdout, encoding='utf-8')
    print(json.dumps({'stage': name, 'exit': result.returncode}), flush=True)
    return result

subprocess.run(['git', 'apply', 'tools/media-resume-test.patch'], check=True)
red = run('regression-red.log', ['node', '--test', 'tools/browser-smoke/browser-errors.test.cjs'])
assert red.returncode == 1 and '# fail 1' in red.stdout, red.stdout
assert re.search(r'not ok \d+ - a suspended preload remains owned', red.stdout), red.stdout

replay = r"""
const fs=require('node:fs');
const assert=require('node:assert/strict');
const observer=require('./tools/browser-smoke/browser-errors.cjs');
const [fixture,output,phase]=process.argv.slice(1);
const rows=JSON.parse(fs.readFileSync(fixture,'utf8'));
const result=rows.map(row=>{
  const actual=observer.residentSuspension(row.failure,row.captured,row.settled)
    ??observer.documentRetirement(row.failure,row.retirement);
  const expected=phase==='red'&&row.newRegression?null:row.expected;
  assert.deepEqual(actual,expected,`${row.source} request ${row.failure.requestId}`);
  return {source:row.source,requestId:row.failure.requestId,newRegression:row.newRegression,actual};
});
fs.writeFileSync(output,JSON.stringify({phase,rows:result},null,2)+'\n');
console.log(JSON.stringify({phase,checked:result.length,newRegressions:result.filter(x=>x.newRegression).length}));
"""

def replay_phase(phase):
    result = run('replay-' + phase + '.log', ['node', '-e', replay,
        'tools/media-resume-evidence.json', str(out / ('replay-' + phase + '.json')), phase])
    assert result.returncode == 0, result.stdout

replay_phase('red')
subprocess.run(['git', 'apply', 'tools/media-resume-fix.patch'], check=True)
green = run('browser-support-green.log', ['node', '--test',
    'tools/browser-smoke/gpu-ui-contract.test.cjs',
    'tools/browser-smoke/browser-errors.test.cjs',
    'tools/browser-smoke/browser-lifecycle.test.cjs'])
assert green.returncode == 0, green.stdout
replay_phase('green')
diff = subprocess.check_output(['git', 'diff', '--', 'tools/browser-smoke/browser-errors.cjs',
                                'tools/browser-smoke/browser-errors.test.cjs'])
(out / 'verified-source.patch').write_bytes(diff)
(out / 'regression.json').write_text(json.dumps({
    'source': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
    'redExit': red.returncode, 'greenExit': green.returncode,
    'verifiedPatchSha256': hashlib.sha256(diff).hexdigest(),
    'classifierSha256': hashlib.sha256(Path('tools/browser-smoke/browser-errors.cjs').read_bytes()).hexdigest(),
    'fixtureSha256': hashlib.sha256(Path('tools/media-resume-evidence.json').read_bytes()).hexdigest(),
}, indent=2) + '\n')
subprocess.run([sys.executable, 'tools/diagnostic-browser-buffer.py'], check=True)
