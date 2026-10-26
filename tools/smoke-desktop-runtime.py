"""Run the packaged game through OS accessibility, keyboard and window APIs.

Storage fault checks hold the real save lock and increment a saved revision to
represent a concurrent writer. The journey never injects game state or replaces
application callbacks. Run in a dedicated desktop session.
"""

import hashlib
from contextlib import contextmanager
import json
import os
from pathlib import Path
import platform
import re
import shutil
import socket
import subprocess
import tempfile
import time

import xa11y


PLATFORM = {"Windows": "win32", "Darwin": "darwin", "Linux": "linux"}[platform.system()]
ARCH = {"x86_64": "x64", "AMD64": "x64", "arm64": "arm64", "aarch64": "arm64"}[
    platform.machine()
]
PACKAGE = Path(os.environ.get(
    "SDR_DESKTOP_APP", f"dist-desktop/Solomon Darker-{PLATFORM}-{ARCH}"
)).resolve()
EVIDENCE = Path(os.environ.get("SDR_DESKTOP_EVIDENCE", "reports/desktop-smoke")).resolve()
MANIFEST = json.loads((PACKAGE / "desktop-package-manifest.json").read_text())
EXECUTABLE = PACKAGE / MANIFEST["executable"]
EVIDENCE.mkdir(parents=True, exist_ok=True)
PROFILES = Path(tempfile.mkdtemp(prefix="solomon-native-smoke-"))
RUNNING = []
RECEIPT = {"platform": PLATFORM, "arch": ARCH, "revision": MANIFEST["revision"],
           "runtime": "native", "exits": [], "cleanup": []}
INPUT = xa11y.input_sim()
xa11y.set_default_timeout(30)


def wait(predicate, description, timeout=120):
    deadline = time.monotonic() + timeout
    while True:
        result = predicate()
        if result:
            return result
        for app in RUNNING:
            if app.process.poll() is not None and not app.closed:
                raise AssertionError(f"{app.name} exited while waiting for {description}: "
                                     f"{app.process.returncode}; see {app.log_path}")
        if time.monotonic() >= deadline:
            raise TimeoutError(f"Timed out waiting for {description}")
        time.sleep(0.1)


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def save_record(profile):
    path = profile / "native-v1/game-save.json"
    if not path.exists():
        return None
    record = json.loads(path.read_text())
    return record, json.loads(record["document"])


def player(document):
    entities = document["continuation"]["simulation"]["playerEntities"]
    player_id = document["continuation"]["summary"]["playerId"]
    index = next(i for i, row in enumerate(entities["identities"]) if row["playerId"] == player_id)
    return {key: rows[index] for key, rows in entities.items() if isinstance(rows, list)}


def identity(document):
    return document["continuation"]["summary"]["character"]


class Application:
    def __init__(self, name, profile):
        self.name, self.profile, self.closed = name, profile, False
        self.log_path = EVIDENCE / f"{name}.log"
        self.log = self.log_path.open("wb")
        self.process = subprocess.Popen(
            [str(EXECUTABLE), "--data-dir", str(profile)],
            cwd=PACKAGE, stdout=self.log, stderr=subprocess.STDOUT,
        )
        RUNNING.append(self)
        self.app = xa11y.App.by_pid(self.process.pid, timeout=30)
        self.control("launcher/solo").wait_enabled(timeout=60)
        self.activate()

    def control(self, identifier):
        return self.app.locator(f"[stable_id='{identifier}']")

    def activate(self):
        self.app.locator("window").first().activate()
        if PLATFORM == "darwin":
            # macOS runners can expose the window tree while the system-wide
            # AXFocusedApplication is empty. Claim and verify the native process
            # slot as xa11y's own CI harness does before synthesizing key events.
            def frontmost():
                result = subprocess.run([
                    "osascript", "-e",
                    'tell application "System Events"\n'
                    f'set frontmost of (first process whose unix id is {self.process.pid}) to true\n'
                    'return unix id of first application process whose frontmost is true\n'
                    'end tell',
                ], capture_output=True, text=True, timeout=5)
                if result.returncode:
                    raise RuntimeError(f"Cannot activate {self.name}: {result.stderr.strip()}")
                return result.stdout.strip() == str(self.process.pid)
            wait(frontmost, f"{self.name} in the foreground", 10)
        else:
            wait(lambda: xa11y.App.foreground(timeout=1).pid == self.process.pid,
                 f"{self.name} in the foreground", 10)

    def click(self, identifier):
        self.control(identifier).wait_enabled()
        self.control(identifier).press()

    def fill(self, identifier, value):
        self.control(identifier).wait_enabled()
        self.control(identifier).set_value(value)
        wait(lambda: self.control(identifier).element().value == value,
             f"{identifier} accepting its edit", 10)

    def key(self, key):
        self.activate()
        INPUT.press(key)

    def hold(self, key, seconds):
        self.activate()
        INPUT.key_down(key)
        try:
            time.sleep(seconds)
        finally:
            INPUT.key_up(key)

    def capture(self, name, activate=True):
        if activate:
            self.activate()
        xa11y.screenshot(element=self.app.locator("window").first().element()).save_png(
            EVIDENCE / f"{name}.png"
        )

    def title(self, fresh=False):
        wait(lambda: self.control("play").exists() or self.control("prompt-secondary").exists(),
             f"{self.name} title")
        if self.control("prompt-secondary").exists():
            assert fresh, "an established profile must retain its tutorial preference"
            self.click("prompt-secondary")
        self.control("play").wait_enabled()

    def continue_game(self):
        self.title()
        self.click("play")
        self.click("continue")
        self.control("hud:inventory").wait_enabled(timeout=120)

    def new_game(self, name, element):
        self.title(fresh=True)
        self.click("play")
        self.click("new-game")
        wait(lambda: self.control("create-name").exists()
             or self.control("panel:dialogue-next").exists()
             or self.control("prompt-primary").exists(), "new wizard admission")
        if self.control("prompt-primary").exists():
            self.click("prompt-primary")
        wait(lambda: self.control("create-name").exists()
             or self.control("panel:dialogue-next").exists(), "College introduction")
        if not self.control("create-name").exists():
            self.click("panel:dialogue-next")
            self.click("panel:dialogue-choice:0")
            self.key("Escape")
            self.control("panel:dialogue-next").wait_detached()
            # Walk around the Office desk's left edge, then through the south door.
            # Its authored bounds are x402.5..638.5, y644.5..741.5; the introduction
            # finishes near (420,415).
            self.hold("a", 1.1)
            self.hold("s", 3.6)
            self.hold("d", 1.9)
            self.activate()
            INPUT.key_down("s")
            try:
                self.control("create-name").wait_attached(timeout=15)
            finally:
                INPUT.key_up("s")
        self.fill("create-name", name)
        self.click(f"element:{element}")
        self.click("discipline:arcane")
        self.control("create-name").wait_detached()
        self.control("hud:map").wait_attached(timeout=120)
        return wait(lambda: self.saved_as(name, element), f"saved {name}/{element}/arcane")

    def saved_as(self, name, element):
        record = save_record(self.profile)
        if record and identity(record[1]) == {"displayName": name, "element": element,
                                             "discipline": "arcane"}:
            return record
        return None

    def request_close(self):
        self.app.locator("window").first().close()

    def exited(self):
        code = self.process.wait(timeout=30)
        self.closed = True
        self.log.close()
        RECEIPT["exits"].append({"name": self.name, "code": code})
        assert code == 0, f"{self.name} failed; see {self.log_path}"
        record = save_record(self.profile)
        if record:
            write_json(EVIDENCE / f"{self.name}-save.json", record[0])
        return record

    def close(self):
        self.request_close()
        return self.exited()


def check_package():
    assert MANIFEST["runtime"] == "native" and MANIFEST["schemaVersion"] == 2
    assert (MANIFEST["platform"], MANIFEST["arch"]) == (PLATFORM, ARCH)
    expected = MANIFEST["files"][MANIFEST["executable"]]
    assert hashlib.sha256(EXECUTABLE.read_bytes()).hexdigest() == expected
    RECEIPT["executableSha256"] = expected


def settings(app, verify=False):
    app.click("settings")
    slider = app.control("settings-range:music")
    if not verify:
        app.activate()
        slider.focus()
        INPUT.press("Home")
        INPUT.press("ArrowRight")
    wait(lambda: slider.element().value == "1" or slider.element().numeric_value == 1,
         "persisted music volume", 10)
    app.click("settings-page:controls")
    reverse = app.control("settings-toggle:reverse-hotbar")
    if not verify:
        app.click("settings-toggle:reverse-hotbar")
    wait(lambda: reverse.element().checked == "on", "persisted hotbar preference", 10)
    app.click("settings-back")
    app.click("settings-back")


def check_failed_close(app):
    path = app.profile / "native-v1/game-save.json"
    previous = json.loads(path.read_text())
    previous["revision"] += 1
    # Commit another writer's revision atomically, keeping its exact document.
    replacement = path.with_suffix(".concurrent")
    write_json(replacement, previous)
    os.replace(replacement, path)
    expected = path.read_bytes()
    app.request_close()
    app.control("desktop/keep-playing").wait_enabled()
    app.capture("save-conflict")
    assert path.read_bytes() == expected, "failed close overwrote the concurrent record"
    app.click("desktop/keep-playing")
    app.control("desktop/keep-playing").wait_detached()
    assert app.process.poll() is None, "Keep playing must retain the application"
    app.request_close()
    app.click("desktop/close-unsaved")
    app.exited()
    assert path.read_bytes() == expected, "Close without saving wrote a checkpoint"
    return {"retainedConcurrentRecord": True, "keepPlaying": True, "closeWithoutSaving": True}


@contextmanager
def held_save_lock(profile):
    with (profile / "native-v1/game-save.json.lock").open("r+b") as lock:
        if PLATFORM == "win32":
            import msvcrt
            msvcrt.locking(lock.fileno(), msvcrt.LK_LOCK, 1)
        else:
            import fcntl
            fcntl.flock(lock, fcntl.LOCK_EX)
        try:
            yield
        finally:
            if PLATFORM == "win32":
                msvcrt.locking(lock.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                fcntl.flock(lock, fcntl.LOCK_UN)


def check_delayed_close(app):
    with held_save_lock(app.profile):
        before = save_record(app.profile)[0]["revision"]
        app.hold("a", 0.8)
        app.request_close()
        app.control("desktop/keep-playing").wait_enabled(timeout=35)
        app.capture("save-timeout")
        app.click("desktop/keep-playing")
        app.control("desktop/keep-playing").wait_detached()
    wait(lambda: save_record(app.profile)[0]["revision"] > before,
         "the delayed checkpoint finishing after Keep playing")
    app.key("Escape")
    app.control("hud:inventory").wait_enabled()
    before_x = player(save_record(app.profile)[1])["locomotions"]["position"]["x"]
    app.hold("d", 1.2)
    final = app.close()[1]
    after_x = player(final)["locomotions"]["position"]["x"]
    assert after_x > before_x + 10, "Keep playing did not restore gameplay after the save timeout"
    return {"lateSaveCommitted": True, "beforeX": before_x, "afterX": after_x}


def check_discarded_session(app):
    path = app.profile / "native-v1/game-save.json"
    with held_save_lock(app.profile):
        expected = path.read_bytes()
        app.hold("a", 0.8)
        app.key("Escape")
        app.click("desktop/launcher")
        app.control("desktop/close-unsaved").wait_enabled(timeout=35)
        app.click("desktop/close-unsaved")
        app.control("launcher/solo").wait_enabled()
    # A discarded blocking write used to wake here and replace the durable slot.
    for _ in range(20):
        time.sleep(0.1)
        assert path.read_bytes() == expected, "the retired session committed after returning"
    app.capture("discarded-session-launcher")
    return {"pendingWriteCancelled": True, "durableSavePreserved": True}


def check_offline():
    profile = PROFILES / "solo"
    solo = Application("solo", profile)
    solo.capture("launcher")
    solo.fill("launcher/host", "")
    solo.click("launcher/start-host")
    wait(lambda: solo.control("launcher/status").exists(), "host validation", 10)
    solo.click("launcher/join")
    wait(lambda: "invite" in (solo.control("launcher/status").element().name or "").lower(),
         "empty invite validation", 10)
    solo.click("launcher/solo")
    solo.title(fresh=True)
    settings(solo)
    initial = solo.new_game("NativeOffline", "fire")[1]
    solo.capture("solo")
    start_x = player(initial)["locomotions"]["position"]["x"]
    solo.hold("d", 1.2)
    moved = solo.close()[1]
    moved_x = player(moved)["locomotions"]["position"]["x"]
    assert moved_x > start_x + 10, "held OS keyboard input did not move the wizard"
    resumed = Application("resumed", profile)
    second = subprocess.run([str(EXECUTABLE), "--data-dir", str(profile)], cwd=PACKAGE,
                            capture_output=True, timeout=15)
    assert second.returncode == 0 and resumed.process.poll() is None
    assert save_record(profile)[1] == moved, "second launch replaced the saved document"
    resumed.click("launcher/solo")
    resumed.title()
    settings(resumed, verify=True)
    resumed.continue_game()
    resumed.capture("resumed")
    RECEIPT["offline"] = {"beforeX": start_x, "savedX": moved_x,
                          "character": identity(moved), "secondLaunchFocused": True}
    RECEIPT["delayedClose"] = check_delayed_close(resumed)
    conflicted = Application("conflicted", profile)
    conflicted.click("launcher/solo")
    conflicted.continue_game()
    RECEIPT["failedClose"] = check_failed_close(conflicted)
    returned = Application("returned", profile)
    returned.click("launcher/solo")
    returned.continue_game()
    returned.key("Escape")
    returned.click("desktop/launcher")
    returned.control("launcher/solo").wait_enabled(timeout=120)
    returned.capture("returned-to-launcher")
    RECEIPT["savedReturnToLauncher"] = True
    returned.click("launcher/solo")
    returned.continue_game()
    RECEIPT["discardedSession"] = check_discarded_session(returned)
    returned.close()


def check_peers():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    host = Application("host", PROFILES / "host")
    host.fill("launcher/host", "127.0.0.1")
    host.fill("launcher/port", str(port))
    host.click("launcher/start-host")
    host.new_game("NativeHost", "ether")
    host.key("Escape")
    host.click("desktop/copy-invite")
    host.key("Escape")
    guest = Application("guest", PROFILES / "guest")
    guest.activate()
    guest.control("launcher/invite").focus()
    INPUT.chord("v", held=["Meta" if PLATFORM == "darwin" else "Ctrl"])
    invite = guest.control("launcher/invite")
    wait(lambda: (invite.element().value or "").startswith("solomondarker://"),
         "native clipboard invite", 10)
    guest.click("launcher/join")
    guest.new_game("NativeGuest", "fire")
    assert not guest.control("hud:map").element().enabled, "the guest may not start the match"
    host.activate()
    host.click("hud:map")

    def shared_boneyard():
        saved = [save_record(app.profile) for app in (host, guest)]
        if any(record is None for record in saved):
            return None
        documents = [record[1] for record in saved]
        if not all(document["continuation"]["summary"]["activeRun"] for document in documents):
            return None
        run_ids = [document["continuation"]["simulation"]["world"]["runId"]
                   for document in documents]
        return run_ids[0] if run_ids[0] == run_ids[1] else None

    run_id = wait(shared_boneyard, "both genuine peer Boneyard checkpoints")
    host.capture("peer-host-boneyard")
    guest.capture("peer-guest-boneyard")
    guest_save = guest.close()[1]
    host_save = host.close()[1]
    assert identity(host_save)["displayName"] == "NativeHost"
    assert identity(guest_save)["displayName"] == "NativeGuest"
    assert all(document["continuation"]["simulation"]["world"]["runId"] == run_id
               for document in (host_save, guest_save))
    RECEIPT["peer"] = {"runId": run_id, "host": identity(host_save),
                       "guest": identity(guest_save), "guestMatchStartDisabled": True}


def cleanup():
    for app in RUNNING:
        if app.process.poll() is None:
            try:
                app.request_close()
                app.process.wait(timeout=5)
            except Exception as error:
                RECEIPT["cleanup"].append({"name": app.name, "closeError": str(error)})
                app.process.terminate()
                try:
                    app.process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    app.process.kill()
                    app.process.wait(timeout=5)
                RECEIPT["cleanup"].append({"name": app.name, "forced": True})
        app.closed = True
        app.log.close()
    shutil.rmtree(PROFILES)


try:
    check_package()
    check_offline()
    check_peers()
    RECEIPT["status"] = "ok"
except Exception:
    RECEIPT["status"] = "failed"
    for app in RUNNING:
        if app.process.poll() is None:
            try:
                tree = re.sub(r"solomondarker://[^\s\"']+", "", app.app.dump())
                (EVIDENCE / f"{app.name}-failure.txt").write_text(tree)
                app.capture(f"{app.name}-failure", activate=False)
            except Exception as error:
                RECEIPT.setdefault("diagnosticErrors", []).append(str(error))
    raise
finally:
    cleanup()
    write_json(EVIDENCE / "receipt.json", RECEIPT)
    print(json.dumps(RECEIPT, indent=2))
