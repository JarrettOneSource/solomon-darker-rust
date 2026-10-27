"""Private Windows regression and replay of retained native media events."""
from pathlib import Path
import hashlib
import json
import re
import subprocess
import sys

assert sys.platform == 'win32'
phase = sys.argv[1]
assert phase in ('red', 'green')
out = Path('reports/media-seek')
out.mkdir(parents=True, exist_ok=True)

def run(name, command):
    result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                            text=True, encoding='utf-8', timeout=180)
    (out / name).write_text(result.stdout, encoding='utf-8')
    print(json.dumps({'stage': name, 'exit': result.returncode}), flush=True)
    return result

result = run('browser-support-' + phase + '.log', ['node', '--test',
    'tools/browser-smoke/gpu-ui-contract.test.cjs',
    'tools/browser-smoke/browser-errors.test.cjs',
    'tools/browser-smoke/browser-lifecycle.test.cjs'])
if phase == 'red':
    assert result.returncode == 1 and '# fail 1' in result.stdout, result.stdout
    assert re.search(r'not ok \d+ - a loaded suspended channel can complete', result.stdout), result.stdout
else:
    assert result.returncode == 0, result.stdout

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
replayed = run('replay-' + phase + '.log', ['node', '-e', replay,
    'tools/media-seek-evidence.json', str(out / ('replay-' + phase + '.json')), phase])
assert replayed.returncode == 0, replayed.stdout
(out / ('regression-' + phase + '.json')).write_text(json.dumps({
    'source': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
    'phase': phase, 'testExit': result.returncode, 'replayExit': replayed.returncode,
    'classifierSha256': hashlib.sha256(Path('tools/browser-smoke/browser-errors.cjs').read_bytes()).hexdigest(),
    'fixtureSha256': hashlib.sha256(Path('tools/media-seek-evidence.json').read_bytes()).hexdigest(),
}, indent=2) + '\n')
