"""Observe direct and selector-based window queries without changing game bytes."""
from pathlib import Path
import hashlib
import json
import os
import subprocess

root=Path('reports/desktop-smoke')
root.mkdir(parents=True,exist_ok=True)
p=Path('tools/smoke-desktop-runtime.py')
source=p.read_text()
needle="try:\n    check_package()"
assert source.count(needle)==1
observation='''original_window = Application.window

def observe_window(self):
    started = time.monotonic()
    window = original_window(self)
    direct_ms = (time.monotonic() - started) * 1000
    started = time.monotonic()
    row = {"app": self.name, "pid": self.process.pid, "directMs": direct_ms,
           "directRole": window.role, "directName": window.name}
    try:
        old = self.app.locator("window").first().wait_attached(timeout=0)
        row["selectorMatched"] = True
        row["selectorRole"] = old.role
        row["selectorName"] = old.name
    except xa11y.XA11yError as error:
        row["selectorMatched"] = False
        row["selectorError"] = type(error).__name__
        if isinstance(error, xa11y.TimeoutError):
            row["selectorObservation"] = error.last_observed
    row["selectorMs"] = (time.monotonic() - started) * 1000
    RECEIPT.setdefault("windowQueries", []).append(row)
    write_json(EVIDENCE / "window-queries.json", RECEIPT["windowQueries"])
    return window

Application.window = observe_window

'''
instrumented=source.replace(needle,observation+needle)
p.write_text(instrumented)
(root/'source.json').write_text(json.dumps({
 'source':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),
 'candidateDriverSha256':hashlib.sha256(source.encode()).hexdigest(),
 'instrumentedDriverSha256':hashlib.sha256(instrumented.encode()).hexdigest(),
 'packageRun':40497735544,
 'packageRevision':'74e2887f71d0fbcf23f3f2d654ca2b2c521efb46',
 'failedPrPackageAvailable':False,
 'scope':'One original Intel Mac native journey, using the same-source direct package because the failed PR package upload was cancelled. Direct window API candidate plus read-only comparison against one original selector attempt after each successful direct query.'
},indent=2)+'\n')
try:
 subprocess.run([os.sys.executable,'-u',str(p)],check=True)
finally:
 p.write_text(source)
