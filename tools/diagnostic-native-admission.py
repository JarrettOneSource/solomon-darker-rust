"""Private diagnostic: hold the real preferences lock after socket startup."""

import hashlib
import json
from pathlib import Path
import runpy
import subprocess


PACKAGE_REVISION = "4d8069218db920701003559771177466448106fd"
EXECUTABLE_SHA256 = "328d42ee2e59d87a92915e7b51714b48faa55da11db65a18b5a2ed4b19acbf58"
evidence = Path("reports/desktop-smoke")
evidence.mkdir(parents=True, exist_ok=True)
archive = Path("diagnostic-package/Solomon-Darker-darwin-x64.zip")
expected, filename = Path(str(archive) + ".sha256").read_text().split()
assert filename == archive.name
with archive.open("rb") as stream:
    archive_hash = hashlib.file_digest(stream, "sha256").hexdigest()
assert archive_hash == expected, "downloaded package checksum differs"
inspector = runpy.run_path("scripts/verify-desktop-archive.py")
inspection = inspector["verify_archive"](archive, Path.cwd(), PACKAGE_REVISION)
assert inspection["nativeSha256"] == EXECUTABLE_SHA256
subprocess.run(["ditto", "-x", "-k", str(archive), "dist-desktop"], check=True)

driver = Path("tools/smoke-desktop-runtime.py")
source = driver.read_text()
original = "    check_offline()\n    check_peers()\n"
replacement = '''    check_offline()
    import fcntl
    import threading

    profile = PROFILES / "solo"
    before = save_record(profile)[1]
    app = Application("contention", profile)
    app.click("launcher/solo")
    app.title()
    app.click("play")
    contention = {"holdSecondsRequested": 6.5, "lock": "native-v1/preferences.json.lock"}
    RECEIPT["contention"] = contention

    def hold_preferences():
        try:
            deadline = time.monotonic() + 15
            while "connecting welcome=true" not in app.log_path.read_text():
                if time.monotonic() >= deadline:
                    raise TimeoutError("no new connection before preferences-lock injection")
                time.sleep(0.002)
            contention["connectionObservedAt"] = time.monotonic()
            with (profile / contention["lock"]).open("r+b") as lock:
                while True:
                    try:
                        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                        break
                    except BlockingIOError:
                        if time.monotonic() >= deadline:
                            raise TimeoutError("could not acquire the preferences lock")
                        time.sleep(0.002)
                try:
                    acquired = time.monotonic()
                    contention["acquiredAt"] = acquired
                    contention["logAtAcquire"] = app.log_path.read_text()
                    time.sleep(1)
                    sample = EVIDENCE / "contention-blocked.sample.txt"
                    subprocess.run(
                        ["sample", str(app.process.pid), "2", "10", "-file", str(sample)],
                        capture_output=True, text=True, timeout=6, check=True,
                    )
                    time.sleep(max(0, acquired + 6.5 - time.monotonic()))
                    contention["logBeforeRelease"] = app.log_path.read_text()
                finally:
                    fcntl.flock(lock, fcntl.LOCK_UN)
                    contention["releasedAt"] = time.monotonic()
        except Exception as error:
            contention["injectionError"] = str(error)

    holder = threading.Thread(target=hold_preferences, name="preferences-contention")
    holder.start()
    try:
        app.click("continue")
    finally:
        holder.join(timeout=30)
    assert not holder.is_alive(), "preferences lock owner did not finish"
    assert "injectionError" not in contention, contention
    contention["heldSeconds"] = contention["releasedAt"] - contention["acquiredAt"]
    assert contention["heldSeconds"] >= 6.5, contention
    wait(lambda: "rejected command=accept_welcome" in app.log_path.read_text(),
         "the post-contention admission result", timeout=20)
    trace = app.log_path.read_text()
    markers = {
        "decoded": " welcome decoded ",
        "remembering": " Game remembering resume",
        "failed": " fail reason=The game server handshake timed out.",
        "remembered": " Game remembered resume",
        "rejected": " rejected command=accept_welcome",
    }
    timings = {}
    for name, marker in markers.items():
        matches = [line for line in trace.splitlines()
                   if line.startswith("ADMISSION_DIAGNOSTIC ") and marker in line]
        assert len(matches) == 1, (name, matches)
        timings[name] = float(matches[0].split()[1])
    contention["traceMilliseconds"] = timings
    assert (timings["decoded"] <= timings["remembering"] < timings["failed"]
            < timings["remembered"] <= timings["rejected"]), timings
    assert "Game remembered resume" not in contention["logBeforeRelease"]
    assert "fail reason=The game server handshake timed out." in contention["logBeforeRelease"]
    retry_ui(app.app.locator("group[name='The game host is not connected']").wait_attached,
             timeout=30)
    app.capture("contention-failure")
    failed_save = app.close()[1]
    assert identity(failed_save) == identity(before), "failed admission changed the wizard"
    contention["result"] = "local persistence stall reproduced timeout and masked failure"

    recovered = Application("recovered", profile)
    recovered.click("launcher/solo")
    recovered.continue_game()
    recovered.capture("after-contention")
    after = recovered.close()[1]
    assert identity(after) == identity(before), "recovered continuation changed the wizard"
    contention["recoveredWithoutLock"] = True
    contention["character"] = identity(after)
    contention["beforeX"] = player(before)["locomotions"]["position"]["x"]
    contention["afterX"] = player(after)["locomotions"]["position"]["x"]
'''
assert source.count(original) == 1
instrumented = source.replace(original, replacement)
paths = [driver, Path(__file__), Path("crates/web/src/network/native.rs"),
         Path("crates/web/src/network/connection.rs"), Path("crates/web/src/game/native.rs")]
(evidence / "source.json").write_text(json.dumps({
    "driverRevision": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
    "packageRevision": PACKAGE_REVISION,
    "packageSourceRun": 40527892687,
    "archiveSha256": archive_hash,
    "archiveInspection": inspection,
    "files": {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in paths},
    "executedDriverSha256": hashlib.sha256(instrumented.encode()).hexdigest(),
    "scope": "Ordinary offline actions, one 6.5-second preferences-lock hold, then normal recovery. "
             "Same reviewed executable and unchanged transport deadlines; controlled fault evidence "
             "does not establish the cause of the original intermittent failure.",
}, indent=2) + "\n")
exec(compile(instrumented, str(driver), "exec"), {"__name__": "__main__", "__file__": str(driver)})
