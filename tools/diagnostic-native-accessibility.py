"""Private acceptance-driver checks against the unchanged retained native package."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

mode = sys.argv[1]
assert mode in ("capture", "cleanup", "ordinary", "observed"), mode
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
fixed_hash = hashlib.sha256(source.encode()).hexdigest()

if mode in ("capture", "observed"):
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
elif mode == "cleanup":
    needle = "    # Closing inventory starts the ordinary two-second shared resume hold.\n"
    assert source.count(needle) == 1
    source = source.replace(needle, '    raise RuntimeError("PRIVATE_DIAGNOSTIC_PEER_CAPTURE_FAILURE")\n' + needle)

if mode in ("capture", "observed"):
    probe = """
def peer_input_probe(app, baseline_x):
    app.activate()
    started = time.monotonic()
    samples = []
    row = {"name": app.name, "baselineX": baseline_x, "mode": PEER_INPUT_MODE,
           "samples": samples}
    RECEIPT.setdefault("peerInputProbe", []).append(row)
    INPUT.key_down("d")
    try:
        while True:
            document = save_record(app.profile)[1]
            location = player(document)["locomotions"]
            elapsed = time.monotonic() - started
            samples.append({"elapsed": elapsed,
                            "tick": document["continuation"]["simulation"]["tick"],
                            "position": location["position"],
                            "heading": location["headingDegrees"],
                            "gait": location["gaitDegrees"]})
            if PEER_INPUT_MODE == "capture" and elapsed >= 1.2:
                break
            if PEER_INPUT_MODE == "observed" and location["position"]["x"] > baseline_x + 10:
                break
            if elapsed >= 10:
                raise TimeoutError(f"No durable movement for {app.name} during the held key")
            time.sleep(0.1)
    finally:
        INPUT.key_up("d")
        row["heldSeconds"] = time.monotonic() - started

"""
    source = source.replace("class Application:\n", "PEER_INPUT_MODE = " + repr(mode) + "\n" + probe + "class Application:\n", 1)
    needle = '        app.hold("d", 1.2)\n'
    assert source.count(needle) == 1
    source = source.replace(needle, '        peer_input_probe(app, before_x[app.name])\n')

# Keep the final durable records before the ordinary driver removes its profiles.
needle = "    shutil.rmtree(PROFILES)\n"
assert source.count(needle) == 1
source = source.replace(needle, '''    for app in RUNNING:
        record = save_record(app.profile)
        if record:
            write_json(EVIDENCE / f"{app.name}-cleanup-save.json", record[0])
    shutil.rmtree(PROFILES)
''')
out = Path("reports/native-accessibility") / mode
out.mkdir(parents=True, exist_ok=True)
driver = out / "driver.py"
driver.write_text(source)
env = os.environ.copy()
env["SDR_DESKTOP_EVIDENCE"] = str(out.resolve())
with (out / "driver.log").open("w") as log:
    result = subprocess.run([sys.executable, "-u", str(driver)], env=env, stdout=log, stderr=subprocess.STDOUT)
log = (out / "driver.log").read_text()
print(log, flush=True)
receipt = json.loads((out / "receipt.json").read_text())
assert receipt["revision"] == "33ca699899158bae7d880a5fda605c92849e7eef"
assert receipt["executableSha256"] == "a0936ba4defb005abc55299338f1d342ad771e5376d58bacfd822a0002415036"
assert all(row["code"] == 0 for row in receipt["exits"])
if mode == "cleanup":
    assert result.returncode == 1 and receipt["status"] == "failed", receipt
    assert "RuntimeError: PRIVATE_DIAGNOSTIC_PEER_CAPTURE_FAILURE" in log
    assert [row["name"] for row in receipt["cleanup"]] == ["guest", "host"], receipt
    assert all(row["code"] == 0 and not row.get("forced") and not row.get("closeError") for row in receipt["cleanup"]), receipt
    documents = {}
    for name in ("guest", "host"):
        record = json.loads((out / f"{name}-cleanup-save.json").read_text())
        documents[name] = json.loads(record["document"])
        assert documents[name]["continuation"]["summary"]["character"]["displayName"] == "Native" + name.title()
        assert documents[name]["continuation"]["summary"]["activeRun"]
    assert documents["guest"]["continuation"]["simulation"]["world"]["runId"] == documents["host"]["continuation"]["simulation"]["world"]["runId"]
    assert "The game host is not connected" not in (out / "guest.log").read_text()
else:
    assert result.returncode == 0 and receipt["status"] == "ok", receipt
    assert len(receipt["exits"]) == 6 and receipt["cleanup"] == [], receipt
    assert receipt["peer"]["hostInventoryOpened"]
    assert (out / "peer-guest-boneyard.txt").is_file()
    if mode in ("capture", "observed"):
        assert receipt["injectedCaptureReadFailures"] == 1, receipt
        assert any("private one-shot capture check" in row for row in receipt["accessibilityReadRetries"]), receipt
verification = {"mode": mode, "status": "passed", "driverExit": result.returncode,
                "fixedDriverSha256": fixed_hash, "revision": receipt["revision"],
                "executableSha256": receipt["executableSha256"]}
(out / "verification.json").write_text(json.dumps(verification, indent=2) + "\n")
print(json.dumps(verification), flush=True)
