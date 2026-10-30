"""Check observed peer movement against the exact original Intel package."""

import hashlib
import json
from pathlib import Path
import runpy
import subprocess
import sys


REVISION = "6baedc684382e8012a7e4455b0080b49d7b2b376"
NATIVE_SHA256 = "364c1f50cf96601a7c4729409ba72d4a97601fdef5f74f9cc81258a2909e703d"
EVIDENCE = Path("reports/desktop-smoke")
driver = Path("tools/smoke-desktop-runtime.py")
source = driver.read_text()
assert source == subprocess.check_output(
    ["git", "show", f"{REVISION}:{driver.as_posix()}"], text=True
), "The canonical driver must match the original package's source"

before = '''        # Two software-rendered peers can exceed two seconds per frame.
        # Keep the key held long enough for gameplay to sample it.
        app.hold("d", 3)
'''
after = '''        before_step = player(save_record(app.profile)[1])["locomotions"]["footstepTick"]

        def saved_movement():
            _, document = save_record(app.profile)
            simulation = document["continuation"]["simulation"]
            locomotion = player(document)["locomotions"]
            if (simulation["world"]["runId"] == run_id
                    and locomotion["position"]["x"] > before_x[app.name] + 10
                    and locomotion["footstepTick"] > before_step):
                return document
            return None

        # A software-rendered frame can outlast a fixed key hold. Keep ordinary
        # input held until the real save records movement, within the same deadline.
        started = time.monotonic()
        INPUT.key_down("d")
        try:
            moved = wait(saved_movement, f"{app.name} saved peer movement")
        finally:
            INPUT.key_up("d")
        RECEIPT.setdefault("peerMovementCheckpoints", {})[app.name] = {
            "heldSeconds": time.monotonic() - started,
            "tick": moved["continuation"]["simulation"]["tick"],
            "beforeFootstepTick": before_step,
            "locomotion": player(moved)["locomotions"],
        }
'''
assert source.count(before) == 1
expanded = source.replace(before, after)
compile(expanded, "diagnostic-expanded-driver.py", "exec")
if sys.argv[1:] == ["--check"]:
    print("Original driver matched; proposed movement observation compiles.")
    raise SystemExit(0)
assert not sys.argv[1:]

EVIDENCE.mkdir(parents=True, exist_ok=True)
archive = Path("input/Solomon-Darker-darwin-x64.zip")
expected, filename = Path(str(archive) + ".sha256").read_text().split()
assert filename == archive.name
with archive.open("rb") as stream:
    archive_sha256 = hashlib.file_digest(stream, "sha256").hexdigest()
assert archive_sha256 == expected
inspector = runpy.run_path("scripts/verify-desktop-archive.py")
inspection = inspector["verify_archive"](archive, Path.cwd(), REVISION)
assert inspection["nativeSha256"] == NATIVE_SHA256
subprocess.run(["ditto", "-x", "-k", str(archive), "dist-desktop"], check=True)
binding = {
    "sourceRevision": REVISION, "sourceRun": 41114455330, "sourceAttempt": 1,
    "sourceArtifactId": 13009232047, "archiveSha256": archive_sha256,
    "inspection": inspection,
    "canonicalDriverSha256": hashlib.sha256(source.encode()).hexdigest(),
    "expandedDriverSha256": hashlib.sha256(expanded.encode()).hexdigest(),
    "diagnosticDriverSha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    "experiment": "One full journey on the exact original failed package. Replace only the fixed three-second peer hold with ordinary held input until a real saved checkpoint records eastward movement and a later footstep tick. The existing 120-second observation bound, final movement assertions, other journeys, save failures and cleanup remain. No game instrumentation, state injection, process sampler or product change.",
}
(EVIDENCE / "package-binding.json").write_text(json.dumps(binding, indent=2) + "\n")
(EVIDENCE / "diagnostic-expanded-driver.py").write_text(expanded)
exec(compile(expanded, str(driver), "exec"), {"__name__": "__main__", "__file__": str(driver)})
