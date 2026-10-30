"""Observe key delivery and frame sampling in a diagnostic native build."""

import hashlib
import json
from pathlib import Path
import runpy
import subprocess
import sys


REVISION = "6baedc684382e8012a7e4455b0080b49d7b2b376"
EVIDENCE = Path("reports/desktop-smoke")
driver = Path("tools/smoke-desktop-runtime.py")
source = driver.read_text()
assert source == subprocess.check_output(
    ["git", "show", f"{REVISION}:{driver.as_posix()}"], text=True
), "The original package journey must remain unchanged"

hook = '''
def observed_checkpoint(app):
    record = save_record(app.profile)
    if record is None:
        return None
    saved, document = record
    continuation = document["continuation"]
    return {"revision": saved["revision"],
            "documentSha256": hashlib.sha256(saved["document"].encode()).hexdigest(),
            "tick": continuation["simulation"]["tick"],
            "worldKind": continuation["summary"]["worldKind"],
            "locomotion": player(document)["locomotions"]}


original_hold = Application.hold


def observed_hold(app, key, seconds):
    if app.name not in ("host", "guest") or key != "d" or seconds != 3:
        return original_hold(app, key, seconds)
    trace = {"name": app.name, "pid": app.process.pid, "key": key,
             "requestedSeconds": seconds,
             "clockReference": {"monotonicSeconds": time.monotonic(),
                                "unixSeconds": time.time()},
             "beforeActivation": observed_checkpoint(app),
             "activationStartedAt": time.monotonic()}
    RECEIPT.setdefault("peerInputObservations", []).append(trace)
    sampler = None
    sampler_log = None
    try:
        app.activate()
        trace["activationReturnedAt"] = time.monotonic()
        trace["beforeKeyDown"] = observed_checkpoint(app)
        trace["keyDownStartedAt"] = time.monotonic()
        INPUT.key_down(key)
        trace["keyDownReturnedAt"] = time.monotonic()
        try:
            sampler_log = (EVIDENCE / f"{app.name}-input-sampler.log").open("w")
            trace["samplerStartedAt"] = time.monotonic()
            sampler = subprocess.Popen(
                ["sample", str(app.process.pid), "3", "10", "-file",
                 str(EVIDENCE / f"{app.name}-input.sample.txt")],
                stdout=sampler_log, stderr=subprocess.STDOUT,
            )
            time.sleep(seconds)
        finally:
            trace["keyUpStartedAt"] = time.monotonic()
            INPUT.key_up(key)
            trace["keyUpReturnedAt"] = time.monotonic()
        trace["afterKeyUp"] = observed_checkpoint(app)
        if sampler is not None:
            trace["samplerExit"] = sampler.wait(timeout=30)
            assert trace["samplerExit"] == 0, trace
        trace["afterSampling"] = observed_checkpoint(app)
    finally:
        if sampler is not None and sampler.poll() is None:
            sampler.kill()
            sampler.wait(timeout=5)
            trace["samplerForced"] = True
        if sampler_log is not None:
            sampler_log.close()
        trace["finishedAt"] = time.monotonic()
        write_json(EVIDENCE / f"{app.name}-input-observation.json", trace)
        write_json(EVIDENCE / "receipt.json", RECEIPT)


Application.hold = observed_hold
'''

entry = "\ntry:\n    check_package()\n"
assert source.count(entry) == 1
expanded = source.replace(entry, hook + entry)
readiness = "    for app in (host, guest):\n        app.activate()\n"
assert expanded.count(readiness) == 1
expanded = expanded.replace(readiness, '''    RECEIPT["peerReadiness"] = {
        "runId": run_id, "initialTicks": initial_ticks, "beforeX": before_x,
        "playingTicks": {name: document["continuation"]["simulation"]["tick"]
                         for name, document in playing.items()}}
    write_json(EVIDENCE / "receipt.json", RECEIPT)
''' + readiness)
environment = "            cwd=PACKAGE, stdout=self.log, stderr=subprocess.STDOUT,"
assert expanded.count(environment) == 1
expanded = expanded.replace(environment, environment + "\n            env={**os.environ, **({'SDR_DIAGNOSTIC_PEER_INPUT': '1'} if name in ('host', 'guest') else {})},")
compile(expanded, "diagnostic-expanded-driver.py", "exec")
if sys.argv[1:] == ["--check"]:
    print("Original driver matched; expanded observational driver compiles.")
    raise SystemExit(0)
assert not sys.argv[1:]

EVIDENCE.mkdir(parents=True, exist_ok=True)
diagnostic_revision = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
archive = Path("dist-desktop/Solomon-Darker-darwin-x64.zip")
expected, filename = Path(str(archive) + ".sha256").read_text().split()
assert filename == archive.name
with archive.open("rb") as stream:
    archive_sha256 = hashlib.file_digest(stream, "sha256").hexdigest()
assert archive_sha256 == expected
inspector = runpy.run_path("scripts/verify-desktop-archive.py")
inspection = inspector["verify_archive"](archive, Path.cwd(), diagnostic_revision)
binding = {
    "sourceBaseRevision": REVISION, "diagnosticRevision": diagnostic_revision,
    "priorUnchangedPackageObservationRun": 41123880578,
    "archiveSha256": archive_sha256, "inspection": inspection,
    "changedFiles": subprocess.check_output(["git", "diff", "--name-only", REVISION, "HEAD"], text=True).splitlines(),
    "canonicalDriverSha256": hashlib.sha256(source.encode()).hexdigest(),
    "expandedDriverSha256": hashlib.sha256(expanded.encode()).hexdigest(),
    "diagnosticDriverSha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    "experiment": "One unchanged canonical full journey against a diagnostic build. Only host and guest emit KeyD/focus delivery, movement state sampling and frame boundaries. Original three-second holds and movement assertions remain, with the prior read-only samplers and timing observations. Logging, sampling and their overhead are diagnostic conditions. Never publish this package as a release.",
}
(EVIDENCE / "package-binding.json").write_text(json.dumps(binding, indent=2) + "\n")
(EVIDENCE / "diagnostic-expanded-driver.py").write_text(expanded)
exec(compile(expanded, str(driver), "exec"), {"__name__": "__main__", "__file__": str(driver)})
