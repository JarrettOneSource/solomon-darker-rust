"""Private diagnostic: exercise unmodified offline actions with connection tracing."""

import hashlib
import json
from pathlib import Path
import subprocess


driver = Path("tools/smoke-desktop-runtime.py")
source = driver.read_text()
original = "    check_offline()\n    check_peers()\n"
replacement = '''    check_offline()
    for attempt in range(8):
        profile = PROFILES / "solo"
        before = save_record(profile)[1]
        app = Application(f"repeat-{attempt + 1}", profile)
        app.click("launcher/solo")
        app.continue_game()
        app.capture(f"continued-{attempt + 1}")
        after = app.close()[1]
        assert identity(after) == identity(before), "continuation changed the wizard"
        RECEIPT.setdefault("repeatedContinuations", []).append({
            "attempt": attempt + 1,
            "character": identity(after),
            "beforeX": player(before)["locomotions"]["position"]["x"],
            "afterX": player(after)["locomotions"]["position"]["x"],
        })
'''
assert source.count(original) == 1
instrumented = source.replace(original, replacement)
evidence = Path("reports/desktop-smoke")
evidence.mkdir(parents=True, exist_ok=True)
paths = [driver, Path(__file__), Path("crates/web/src/network/native.rs"),
         Path("crates/web/src/network/connection.rs"), Path("crates/web/src/game/native.rs")]
(evidence / "source.json").write_text(json.dumps({
    "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
    "baseRevision": "3c42d3f1ed6cc07a070e6fb481c32d1e91621071",
    "files": {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in paths},
    "executedDriverSha256": hashlib.sha256(instrumented.encode()).hexdigest(),
    "scope": "Ordinary offline recovery followed by eight fresh-process continuations; no peer qualification.",
}, indent=2) + "\n")
exec(compile(instrumented, str(driver), "exec"), {"__name__": "__main__", "__file__": str(driver)})
