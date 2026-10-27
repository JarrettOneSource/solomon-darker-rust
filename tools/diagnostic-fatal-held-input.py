"""Compare immediate and delayed saved recovery after the same held-key fault."""
from pathlib import Path
import os
import subprocess
import sys

source = Path("tools/smoke-desktop-runtime.py").read_text()
needle = "    check_offline()\n    check_peers()\n"
assert source.count(needle) == 1
journey = '''    trigger = Path(os.environ["SDR_FATAL_STATUS_TRIGGER"])
    results = []
    for label, delay in (("immediate", 0), ("delayed", 4)):
        trigger.unlink(missing_ok=True)
        profile = PROFILES / label
        app = Application(label, profile)
        app.click("launcher/solo")
        initial = app.new_game("Failure", "ether")[1]
        initial_x = player(initial)["locomotions"]["position"]["x"]
        app.activate()
        pressed = time.monotonic()
        INPUT.key_down("d")
        try:
            time.sleep(0.6)
            trigger.touch()
            message = "Native audio output: The stream configuration is no longer valid and must be rebuilt."
            status = app.app.locator("status[name=" + json.dumps(message) + "]")
            read_ui(status.wait_attached)
            failed = time.monotonic()
        finally:
            INPUT.key_up("d")
        time.sleep(delay)
        requested = time.monotonic()
        app.click("desktop/launcher")
        read_ui(app.control("launcher/solo").wait_enabled, timeout=30)
        saved = save_record(profile)
        assert saved is not None and identity(saved[1])["displayName"] == "Failure"
        final_x = player(saved[1])["locomotions"]["position"]["x"]
        write_json(EVIDENCE / f"{label}-saved-after-failure.json", saved[0])
        app.capture(f"{label}-returned-after-failure")
        app.request_close()
        app.exited()
        results.append({"name": label, "delay": delay, "initialX": initial_x,
                        "finalX": final_x, "distance": final_x - initial_x,
                        "secondsUntilFatalStatus": failed - pressed,
                        "secondsAfterFatalStatus": requested - failed})
        RECEIPT["heldInputAfterFailure"] = results
    drift = results[1]["distance"] - results[0]["distance"]
    RECEIPT["additionalDelayedMovement"] = drift
    assert drift < 50, f"The recovery screen retained held movement: {drift} additional world units"
'''
source = source.replace(needle, journey)
source = source.replace("    shutil.rmtree(PROFILES)\n", '''    for app in RUNNING:
        record = save_record(app.profile)
        if record:
            write_json(EVIDENCE / f"{app.name}-cleanup-save.json", record[0])
    shutil.rmtree(PROFILES)
''')
out = Path("reports/native-fatal-held-input")
out.mkdir(parents=True, exist_ok=True)
driver = out / "driver.py"
driver.write_text(source)
env = os.environ.copy()
env["SDR_DESKTOP_EVIDENCE"] = str((out / "journey").resolve())
env["SDR_FATAL_STATUS_TRIGGER"] = str((out / "trigger").resolve())
raise SystemExit(subprocess.run([sys.executable, "-u", str(driver)], env=env).returncode)
