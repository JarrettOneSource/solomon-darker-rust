"""Private candidate proof: complete admission after real preferences contention."""

import hashlib
import json
from pathlib import Path
import runpy
import subprocess


PACKAGE_REVISION = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
CANDIDATE_REVISION = "3625b85904af56526e3ba18c3f95485e49347d0b"
evidence = Path("reports/desktop-smoke")
evidence.mkdir(parents=True, exist_ok=True)
archive = Path("dist-desktop/Solomon-Darker-darwin-x64.zip")
expected, filename = Path(str(archive) + ".sha256").read_text().split()
assert filename == archive.name
with archive.open("rb") as stream:
    archive_hash = hashlib.file_digest(stream, "sha256").hexdigest()
assert archive_hash == expected, "built package checksum differs"
inspector = runpy.run_path("scripts/verify-desktop-archive.py")
inspection = inspector["verify_archive"](archive, Path.cwd(), PACKAGE_REVISION)
subprocess.run(["ditto", "-x", "-k", str(archive), "dist-desktop"], check=True)
package = Path("dist-desktop/Solomon Darker-darwin-x64")
manifest = json.loads((package / "desktop-package-manifest.json").read_text())
assert manifest["revision"] == PACKAGE_REVISION
with (package / manifest["executable"]).open("rb") as stream:
    assert hashlib.file_digest(stream, "sha256").hexdigest() == inspection["nativeSha256"]

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
        sampler = None
        sample_output = None
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
                    while "Game remembering resume" not in app.log_path.read_text():
                        if time.monotonic() >= acquired + 6.5:
                            raise TimeoutError("local acceptance did not reach the held preferences lock")
                        time.sleep(0.002)
                    sample = EVIDENCE / "contention-blocked.sample.txt"
                    sample_output = (EVIDENCE / "contention-sampler.log").open("w")
                    sampler = subprocess.Popen(
                        ["sample", str(app.process.pid), "2", "10", "-file", str(sample)],
                        stdout=sample_output, stderr=subprocess.STDOUT,
                    )
                    time.sleep(max(0, acquired + 6.5 - time.monotonic()))
                    contention["logBeforeRelease"] = app.log_path.read_text()
                finally:
                    fcntl.flock(lock, fcntl.LOCK_UN)
                    contention["releasedAt"] = time.monotonic()
            # Symbolication can outlive collection. It must not extend the lock hold.
            if sampler is not None:
                contention["sampleExitCode"] = sampler.wait(timeout=30)
                assert contention["sampleExitCode"] == 0, contention
        except Exception as error:
            contention["injectionError"] = str(error)
        finally:
            if sampler is not None and sampler.poll() is None:
                sampler.kill()
                sampler.wait(timeout=5)
            if sample_output is not None:
                sample_output.close()

    holder = threading.Thread(target=hold_preferences, name="preferences-contention")
    holder.start()
    try:
        app.click("continue")
    finally:
        holder.join(timeout=60)
    assert not holder.is_alive(), "preferences lock owner did not finish"
    assert "injectionError" not in contention, contention
    contention["heldSeconds"] = contention["releasedAt"] - contention["acquiredAt"]
    assert contention["heldSeconds"] >= 6.5, contention
    retry_ui(app.control("hud:inventory").wait_enabled, timeout=120)
    retry_ui(app.control("match-loading").wait_detached, timeout=120)
    wait(lambda: " welcome accepted " in app.log_path.read_text(),
         "the post-contention admission result", timeout=20)
    trace = app.log_path.read_text()
    markers = {
        "decoded": " welcome decoded ",
        "remembering": " Game remembering resume",
        "remembered": " Game remembered resume",
        "accepted": " welcome accepted ",
    }
    timings = {}
    for name, marker in markers.items():
        matches = [line for line in trace.splitlines()
                   if line.startswith("ADMISSION_DIAGNOSTIC ") and marker in line]
        assert len(matches) == 1, (name, matches)
        timings[name] = float(matches[0].split()[1])
    contention["traceMilliseconds"] = timings
    assert (timings["decoded"] <= timings["remembering"]
            < timings["remembered"] <= timings["accepted"]), timings
    assert timings["remembered"] - timings["decoded"] > 5000, timings
    assert "Game remembered resume" not in contention["logBeforeRelease"]
    assert " fail reason=" not in trace, trace
    assert " rejected " not in trace, trace
    assert "handshake timed out" not in trace, trace
    assert "remember_resume" in (EVIDENCE / "contention-blocked.sample.txt").read_text()
    # Compare input with this live session's checkpoint. Courtyard collision can
    # move an idle wizard while credentials and the sampled frame are pending.
    admitted_revision = save_record(profile)[0]["revision"]
    baseline_record, baseline = wait(
        lambda: (record if (record := save_record(profile))[0]["revision"]
                 > admitted_revision else None),
        "a fresh checkpoint after delayed admission", timeout=65,
    )
    write_json(EVIDENCE / "contention-before-input-save.json", baseline_record)
    app.capture("contention-before-input")
    # Match the established software-renderer peer journey's hold duration.
    contention["inputHoldSeconds"] = 3
    contention["inputStartedAt"] = time.monotonic()
    app.hold("d", contention["inputHoldSeconds"])
    contention["inputFinishedAt"] = time.monotonic()
    app.capture("contention-continued")
    continued_save = app.close()[1]
    contention["inputTrace"] = [line for line in app.log_path.read_text().splitlines()
                                if line.startswith("INPUT_DIAGNOSTIC ")]
    assert identity(continued_save) == identity(before), "delayed admission changed the wizard"
    before_position = player(baseline)["locomotions"]["position"]
    continued_position = player(continued_save)["locomotions"]["position"]
    assert continued_position["x"] > before_position["x"] + 10, (before_position, continued_position)
    assert player(continued_save)["locomotions"]["footstepTick"] > baseline["continuation"]["simulation"]["tick"], "no accepted walking after the observed baseline"
    contention["movementBaseline"] = {
        "position": before_position,
        "tick": baseline["continuation"]["simulation"]["tick"],
        "revision": baseline_record["revision"],
    }
    contention["result"] = "admitted after delayed durable storage; movement and save succeeded"
    contention["continuedPosition"] = continued_position

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
         Path("crates/web/src/network/connection.rs"), Path("crates/web/src/game/native.rs"),
         Path("crates/web/src/network.rs"), Path("crates/web/src/game.rs"),
         Path("crates/web/src/input.rs"), Path("crates/web/src/input/controls.rs"),
         Path("crates/web/src/gpu_ui/input/native.rs"),
         Path("crates/launcher/src/host/hosted/socket.rs"),
         Path("crates/launcher/src/host/input.rs")]
(evidence / "source.json").write_text(json.dumps({
    "driverRevision": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
    "packageRevision": PACKAGE_REVISION,
    "candidateRevision": CANDIDATE_REVISION,
    "archiveSha256": archive_hash,
    "archiveInspection": inspection,
    "files": {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in paths},
    "executedDriverSha256": hashlib.sha256(instrumented.encode()).hexdigest(),
    "scope": "Ordinary offline actions, one 6.5-second preferences-lock hold, then normal recovery. "
             "Candidate runtime with private admission and input traces; admission must wait for durable credentials, "
             "then allow movement, save and fresh continuation. The original stall source remains unknown.",
}, indent=2) + "\n")
exec(compile(instrumented, str(driver), "exec"), {"__name__": "__main__", "__file__": str(driver)})
