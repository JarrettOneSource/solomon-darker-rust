"""Private accessibility recovery and input observations; never ship this fixture."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

mode = sys.argv[1]
assert mode in ("raise", "trace-one", "trace-two"), mode
original = Path("tools/smoke-desktop-runtime.py").read_bytes()
assert hashlib.sha256(original).hexdigest() == "7a94572aaebcc84c393738aa334368aa9836d04de83d1bbbd4dc845186a914fa"
source = original.decode()
old = '        tree = re.sub(r"solomondarker://[^\\s\\\"\']+", "", self.app.dump())'
new = '        tree = re.sub(r"solomondarker://[^\\s\\\"\']+", "", read_ui(lambda timeout: self.app.dump()))'
assert source.count(old) == 1
source = source.replace(old, new)
old = "def cleanup():\n    for app in RUNNING:\n"
new = "def cleanup():\n    # Guests need their host alive to finish saving.\n    for app in reversed(RUNNING):\n"
assert source.count(old) == 1
source = source.replace(old, new)
source = source.replace("read_ui", "retry_ui")
source = source.replace("Retry only AXCannotComplete reads while the software renderer is busy.",
                        "Retry transient AXCannotComplete reads or idempotent focus operations.")
source = source.replace("accessibilityReadRetries", "accessibilityRetries")
source = source.replace("Retrying temporary accessibility read:", "Retrying temporary accessibility call:")
old = "        window.activate()\n"
new = "        retry_ui(lambda timeout: window.activate())\n"
assert source.count(old) == 1
source = source.replace(old, new)
fixed_hash = hashlib.sha256(source.encode()).hexdigest()

if mode == "raise":
    old = "        retry_ui(lambda timeout: window.activate())\n"
    new = '''        def raise_window():
            if (self.name == "host" and any(app.name == "guest" for app in RUNNING)
                    and not getattr(self, "private_raise_failed", False)):
                self.private_raise_failed = True
                RECEIPT["injectedWindowRaiseFailures"] = 1
                raise xa11y.PlatformError("Platform error (-25204): AXRaise failed; private one-shot focus check")
            window.activate()
        retry_ui(lambda timeout: raise_window())
'''
    assert source.count(old) == 1
    source = source.replace(old, new)
    proxy = '''
class CaptureReadProbe:
    def __init__(self, app):
        self.app = app
        self.failed = False

    def __getattr__(self, name):
        return getattr(self.app, name)

    def dump(self):
        if not self.failed:
            self.failed = True
            RECEIPT["injectedCaptureReadFailures"] = 1
            raise xa11y.PlatformError("Platform error (-25204): AXUIElementCopyAttributeValue(AXWindows) failed (AXError -25204); private one-shot capture check")
        return self.app.dump()

'''
    source = source.replace("class Application:\n", proxy + "class Application:\n", 1)
    needle = "        self.app = xa11y.App.by_pid(self.process.pid, timeout=30)\n"
    assert source.count(needle) == 1
    source = source.replace(needle, needle + '        if self.name == "guest":\n            self.app = CaptureReadProbe(self.app)\n')

# Record the ordinary hold's exact OS-input interval, without polling a 30-second save.
old = '        app.hold("d", 1.2)\n'
new = '''        app.activate()
        started = time.time()
        INPUT.key_down("d")
        try:
            time.sleep(1.2)
        finally:
            INPUT.key_up("d")
            RECEIPT.setdefault("peerInputIntervals", []).append({
                "name": app.name, "downUnixSeconds": started,
                "upUnixSeconds": time.time(), "baselineX": before_x[app.name],
            })
'''
assert source.count(old) == 1
source = source.replace(old, new)

needle = "    shutil.rmtree(PROFILES)\n"
assert source.count(needle) == 1
source = source.replace(needle, '''    for app in RUNNING:
        record = save_record(app.profile)
        if record:
            write_json(EVIDENCE / f"{app.name}-cleanup-save.json", record[0])
    shutil.rmtree(PROFILES)
''')
out = Path("reports/native-focus") / mode
out.mkdir(parents=True, exist_ok=True)
driver = out / "driver.py"
driver.write_text(source)
env = os.environ.copy()
env["SDR_DESKTOP_EVIDENCE"] = str(out.resolve())
with (out / "driver.log").open("w") as log:
    result = subprocess.run([sys.executable, "-u", str(driver)], env=env, stdout=log, stderr=subprocess.STDOUT)
print((out / "driver.log").read_text(), flush=True)
receipt = json.loads((out / "receipt.json").read_text())
if mode == "raise":
    assert receipt["revision"] == "09e2e9ea90a042bed6d7779bccf2f02650ea7e15"
    assert receipt["executableSha256"] == "223f96c76d9d8507c3b2a69c75e059e81457e7492a70eb1ace2a3a2194e14af7"
    assert receipt["injectedWindowRaiseFailures"] == 1
    assert receipt["injectedCaptureReadFailures"] == 1
    for marker in ("private one-shot focus check", "private one-shot capture check"):
        assert any(marker in row for row in receipt["accessibilityRetries"]), marker
else:
    assert receipt["revision"] == "33ca699899158bae7d880a5fda605c92849e7eef"
assert result.returncode == 0 and receipt["status"] == "ok", receipt
assert len(receipt["exits"]) == 6 and all(row["code"] == 0 for row in receipt["exits"])
assert receipt["cleanup"] == [] and receipt["peer"]["hostInventoryOpened"]
verification = {"mode": mode, "status": "passed", "driverExit": result.returncode,
                "fixedDriverSha256": fixed_hash, "revision": receipt["revision"],
                "executableSha256": receipt["executableSha256"]}
(out / "verification.json").write_text(json.dumps(verification, indent=2) + "\n")
print(json.dumps(verification), flush=True)
