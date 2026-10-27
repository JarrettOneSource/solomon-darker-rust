"""Observe actual authority input while the fatal recovery screen is visible."""
import json
import os
from pathlib import Path
import subprocess
import sys

mode = sys.argv[1]
assert mode in ("baseline", "released"), mode
source = Path("tools/smoke-desktop-runtime.py").read_text()
needle = "    check_offline()\n    check_peers()\n"
assert source.count(needle) == 1
journey = '''    profile = PROFILES / "fatal-input"
    app = Application("fatal-input", profile)
    app.click("launcher/solo")
    app.new_game("Failure", "ether")
    app.activate()
    INPUT.key_down("d")
    try:
        time.sleep(0.6)
        Path(os.environ["SDR_FATAL_STATUS_TRIGGER"]).touch()
        message = "Native audio output: The stream configuration is no longer valid and must be rebuilt."
        read_ui(app.app.locator("status[name=" + json.dumps(message) + "]").wait_attached)
    finally:
        INPUT.key_up("d")
    time.sleep(4)
    app.capture("fatal-input-visible")
    RECEIPT["saveRequestUnixSeconds"] = time.time()
    app.click("desktop/launcher")
    read_ui(app.control("launcher/solo").wait_enabled, timeout=30)
    saved = save_record(profile)
    assert saved is not None and identity(saved[1])["displayName"] == "Failure"
    write_json(EVIDENCE / "saved-after-failure.json", saved[0])
    app.capture("returned-after-failure")
    app.request_close()
    app.exited()
'''
source = source.replace(needle, journey)
source = source.replace("    shutil.rmtree(PROFILES)\n", '''    for app in RUNNING:
        record = save_record(app.profile)
        if record:
            write_json(EVIDENCE / f"{app.name}-cleanup-save.json", record[0])
    shutil.rmtree(PROFILES)
''')
out = Path("reports/native-fatal-authority") / mode
out.mkdir(parents=True, exist_ok=True)
driver = out / "driver.py"
driver.write_text(source)
env = os.environ.copy()
env["SDR_DESKTOP_EVIDENCE"] = str(out.resolve())
env["SDR_FATAL_STATUS_TRIGGER"] = str((out / "trigger").resolve())
if mode == "released":
    env["PRIVATE_FATAL_RELEASE_INPUT"] = "1"
else:
    env.pop("PRIVATE_FATAL_RELEASE_INPUT", None)
result = subprocess.run([sys.executable, "-u", str(driver)], env=env)
receipt = json.loads((out / "receipt.json").read_text())
assert result.returncode == 0 and receipt["status"] == "ok", receipt
assert receipt["exits"] == [{"name": "fatal-input", "code": 0}] and receipt["cleanup"] == []
lines = (out / "fatal-input.log").read_text().splitlines()
failures = [json.loads(line.removeprefix("PRIVATE_FATAL_FAILURE "))
            for line in lines if line.startswith("PRIVATE_FATAL_FAILURE ")]
assert len(failures) == 1, failures
failed_at = failures[0]["unixSeconds"]
samples = [json.loads(line.removeprefix("PRIVATE_FATAL_AUTHORITY_INPUT "))
           for line in lines if line.startswith("PRIVATE_FATAL_AUTHORITY_INPUT ")]
samples = [row for row in samples
           if failed_at + 0.5 < row["unixSeconds"] < receipt["saveRequestUnixSeconds"]]
assert len(samples) >= 10, "The check must observe real authority ticks after failure and before recovery."
active = [row for row in samples
          if row["movement"] != [0.0, 0.0] or row["primary"] or row["quickbar"] is not None]
verification = {"mode": mode, "failureUnixSeconds": failed_at,
                "saveRequestUnixSeconds": receipt["saveRequestUnixSeconds"],
                "sampleCount": len(samples), "activeInputSamples": len(active),
                "firstSample": samples[0], "lastSample": samples[-1]}
(out / "verification.json").write_text(json.dumps(verification, indent=2) + "\n")
if mode == "baseline":
    assert len(active) >= 10, "The original behavior did not reproduce held input after failure."
else:
    assert not active, "The proposed failure transition did not release authority input."
print(json.dumps(verification), flush=True)
