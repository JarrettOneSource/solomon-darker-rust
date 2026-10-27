"""Private before/after proof of the real Windows launch contract."""
from pathlib import Path
import json
import os
import subprocess

out=Path('reports/browser-network')
out.mkdir(parents=True,exist_ok=True)
p=Path('tools/browser-smoke/browser-lifecycle.cjs')
candidate=p.read_bytes()
baseline=subprocess.check_output(['git','show','65b1ac8755ad7c0ecfb806485f0cecc0c53bf9f8:tools/browser-smoke/browser-lifecycle.cjs'])
results={}
try:
 for label,source in [('red',baseline),('green',candidate)]:
  p.write_bytes(source)
  result=subprocess.run(['node','--test','tools/browser-smoke/browser-launch.test.cjs'],capture_output=True,text=True,timeout=90)
  (out/f'launch-regression-{label}.log').write_text(result.stdout+result.stderr)
  results[label]={'exit':result.returncode,'expected':1 if label=='red' else 0}
  (out/'launch-regression.json').write_text(json.dumps(results,indent=2)+'\n')
  assert result.returncode==results[label]['expected'], (label,result.stdout,result.stderr)
  if label=='red':assert 'TcpPortRandomizationWin' in result.stdout, result.stdout
  print(label,result.returncode,flush=True)
finally:
 p.write_bytes(candidate)
