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

# xa11y's AT-SPI stable_id is the D-Bus object path, while its AX and UIA
# backends expose AccessKit's author ID. Use the actual Linux roles and labels.
LINUX_CONTROLS = {
    "launcher/solo": "button[name='Play offline']",
    "launcher/host": "text_field[name='Your reachable IP or hostname']",
    "launcher/port": "text_field[name='TCP port']",
    "launcher/start-host": "button[name='Host game']",
    "launcher/invite": "text_area[name='Multiplayer invite']",
    "launcher/join": "button[name='Join game']",
    "play": "button[name='Play']",
    "continue": "button[name='Last game']",
    "new-game": "button[name='New game']",
    "prompt-primary": "button[name='YES']",
    "prompt-secondary": "button[name='NO']",
    "create-name": "text_field[name='Wizard name']",
    "panel:dialogue-next": "button[name='Skip'], button[name='Done']",
    "panel:dialogue-choice:0": "button[name='Solomon Dark?']",
    "hud:inventory": "button[name^='Inventory,']",
    "hud:map": "button[name='Enter the Boneyard']",
    "panel:close": "button[name='Open or close inventory']",
    "match-loading": "status[name='Loading game']",
    "settings": "button[name='Settings']",
    "settings-range:music": "slider[name='MUSIC VOL:']",
    "settings-page:controls": "button[name='CUSTOMIZE KEYBOARD']",
    "settings-toggle:reverse-hotbar": "check_box[name='REVERSE HOTBAR CYCLING']",
    "settings-back": "button[name='BACK'], button[name='DONE']",
    "desktop/launcher": "button[name='Save and return to launcher']",
    "desktop/copy-invite": "button[name='Copy multiplayer invite']",
    "desktop/keep-playing": "button[name='Keep playing']",
    "desktop/close-unsaved": "button[name='Close without saving']",
}


def retry_ui(operation, *args, timeout=30):
    """Retry transient AXCannotComplete reads or idempotent focus operations."""
    deadline = time.monotonic() + timeout
    while True:
        try:
            return operation(*args, timeout=max(0, deadline - time.monotonic()))
        except xa11y.PlatformError as error:
            if (PLATFORM != "darwin" or not str(error).startswith("Platform error (-25204):")
                    or time.monotonic() >= deadline):
                raise
            RECEIPT.setdefault("accessibilityRetries", []).append(str(error))
            print(f"Retrying temporary accessibility call: {error}", flush=True)
            time.sleep(0.1)


def wait(predicate, description, timeout=120):
    deadline = time.monotonic() + timeout
    while True:
        result = retry_ui(lambda timeout: predicate(), timeout=max(0, deadline - time.monotonic()))
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
        print(f"Launching {name}", flush=True)
        self.name, self.profile, self.closed = name, profile, False
        self.log_path = EVIDENCE / f"{name}.log"
        self.log = self.log_path.open("wb")
        self.process = subprocess.Popen(
            [str(EXECUTABLE), "--data-dir", str(profile)],
            cwd=PACKAGE, stdout=self.log, stderr=subprocess.STDOUT,
        )
        RUNNING.append(self)
        self.app = xa11y.App.by_pid(self.process.pid, timeout=30)
        retry_ui(self.control("launcher/solo").wait_enabled, timeout=60)
        self.activate()
        print(f"{name}: launcher ready", flush=True)

    def control(self, *identifiers):
        selectors = []
        for identifier in identifiers:
            if identifier == "launcher/status":
                selectors.append(
                    "static_text[name^='Enter your reachable'], static_text[name^='Paste your friend']"
                )
            elif PLATFORM == "linux":
                if identifier.startswith(("element:", "discipline:")):
                    selectors.append(f"button[name={json.dumps(identifier.split(':', 1)[1])}]")
                else:
                    selectors.append(LINUX_CONTROLS[identifier])
            else:
                selectors.append(f"[stable_id='{identifier}']")
        return self.app.locator(", ".join(selectors))

    def wait_ready(self, *identifiers):
        controls = self.control(*identifiers)
        # A modal can retain a disabled matching control beneath its own choices.
        return wait(lambda: next((item for item in controls.elements() if item.enabled), None),
                    f"{self.name} ready control from {identifiers}")

    def activate(self):
        window = retry_ui(self.app.locator("window").first().wait_attached)
        if PLATFORM == "linux":
            # The CI display is 1280x1024. Keep both launcher and game windows
            # fully visible. AT-SPI can reuse a retired window's object path.
            bounds = window.bounds
            if bounds is None or (bounds.width, bounds.height) != (1180, 800):
                window.resize_to(1180, 800)
                retry_ui(self.app.locator("window").first().wait_until,
                    lambda item: item is not None and item.bounds is not None
                    and (item.bounds.width, item.bounds.height) == (1180, 800), timeout=10)
            if bounds is None or (bounds.x, bounds.y) != (40, 40):
                window.move_to(40, 40)
        elif PLATFORM == "win32" and not window.maximized:
            # The runner's desktop can be smaller than the initial game window.
            window.maximize()
            retry_ui(self.app.locator("window").first().wait_until,
                lambda item: item is not None and item.maximized, timeout=10)
        retry_ui(lambda timeout: window.activate())
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
        print(f"{self.name}: click {identifier}", flush=True)
        retry_ui(self.control(identifier).wait_enabled).press()

    def fill(self, identifier, value):
        print(f"{self.name}: edit {identifier}", flush=True)
        retry_ui(self.control(identifier).wait_enabled).set_value(value)
        retry_ui(self.control(identifier).wait_until,
            lambda item: item is not None and (item.value or "") == value, timeout=10)

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
        print(f"{self.name}: capturing {name}", flush=True)
        if activate:
            self.activate()
            # Accessibility can publish a layout before the GPU presents it.
            # Let the compositor settle after focus/resize before reading pixels.
            time.sleep(1)
        # A failed startup may leave a window beyond the X11 screen boundary.
        # Capture the dedicated display so failure evidence remains available.
        shot = (xa11y.screenshot() if PLATFORM == "linux" else
                xa11y.screenshot(element=retry_ui(self.app.locator("window").first().wait_attached)))
        shot.save_png(EVIDENCE / f"{name}.png")
        tree = re.sub(r"solomondarker://[^\s\"']+", "", retry_ui(lambda timeout: self.app.dump()))
        (EVIDENCE / f"{name}.txt").write_text(tree)
        print(f"{self.name}: captured {name}", flush=True)

    def title(self, fresh=False):
        entry = self.wait_ready("play", "prompt-secondary")
        if entry.name == "NO":
            assert fresh, "an established profile must retain its tutorial preference"
            entry.press()
            retry_ui(self.control("play").wait_enabled)

    def continue_game(self):
        self.title()
        self.click("play")
        self.click("continue")
        retry_ui(self.control("hud:inventory").wait_enabled, timeout=120)
        retry_ui(self.control("match-loading").wait_detached, timeout=120)

    def new_game(self, name, element):
        print(f"{self.name}: creating {name}", flush=True)
        self.title(fresh=True)
        self.click("play")
        self.click("new-game")
        entry = self.wait_ready("create-name", "panel:dialogue-next", "prompt-primary")
        if entry.name == "YES":
            entry.press()
            entry = self.wait_ready("create-name", "panel:dialogue-next")
        if entry.name != "Wizard name":
            entry.press()
            self.click("panel:dialogue-choice:0")
            self.key("Escape")
            retry_ui(self.control("panel:dialogue-next").wait_detached)
            # Walk around the Office desk's left edge, then through the south door.
            # Its authored bounds are x402.5..638.5, y644.5..741.5; the introduction
            # finishes near (420,415).
            self.hold("a", 1.1)
            self.hold("s", 3.6)
            self.hold("d", 1.9)
            self.activate()
            INPUT.key_down("s")
            try:
                retry_ui(self.control("create-name").wait_attached, timeout=15)
            finally:
                INPUT.key_up("s")
        self.fill("create-name", name)
        self.click(f"element:{element}")
        self.click("discipline:arcane")
        retry_ui(self.control("create-name").wait_detached)
        retry_ui(self.control("hud:map").wait_attached, timeout=120)
        retry_ui(self.control("match-loading").wait_detached, timeout=120)
        return wait(lambda: self.saved_as(name, element), f"saved {name}/{element}/arcane")

    def saved_as(self, name, element):
        record = save_record(self.profile)
        if record and identity(record[1]) == {"displayName": name, "element": element,
                                             "discipline": "arcane"}:
            return record
        return None

    def request_close(self):
        window = retry_ui(self.app.locator("window").first().wait_attached)
        try:
            window.close()
        except xa11y.PlatformError as error:
            if (PLATFORM != "darwin" or not str(error).startswith("Platform error (-25204):")):
                raise
            # The close may already be delivered. Callers verify the actual exit
            # or save-failure surface; replaying the action could affect that prompt.
            RECEIPT.setdefault("unacknowledgedCloses", []).append({
                "name": self.name, "error": str(error),
            })
            print(f"{self.name}: close acknowledgement unavailable: {error}", flush=True)

    def exited(self):
        code = self.process.wait(timeout=30)
        self.closed = True
        self.log.close()
        RECEIPT["exits"].append({"name": self.name, "code": code})
        assert code == 0, f"{self.name} failed; see {self.log_path}"
        print(f"{self.name}: clean exit", flush=True)
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
        retry_ui(slider.wait_enabled).set_numeric_value(37)
        retry_ui(slider.wait_until, lambda item: item is not None and item.numeric_value == 37, timeout=10)
        slider.focus()
        retry_ui(slider.wait_focused)
        INPUT.press("Home")
        retry_ui(slider.wait_until, lambda item: item is not None and item.numeric_value == 0, timeout=10)
        INPUT.press("ArrowRight")
    retry_ui(slider.wait_until, lambda item: item is not None and item.numeric_value == 1, timeout=10)
    app.click("settings-page:controls")
    reverse = app.control("settings-toggle:reverse-hotbar")
    if not verify:
        app.click("settings-toggle:reverse-hotbar")
    retry_ui(reverse.wait_until, lambda item: item is not None and item.checked == "on", timeout=10)
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
    retry_ui(app.control("desktop/keep-playing").wait_enabled)
    app.capture("save-conflict")
    assert path.read_bytes() == expected, "failed close overwrote the concurrent record"
    app.click("desktop/keep-playing")
    retry_ui(app.control("desktop/keep-playing").wait_detached)
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
        retry_ui(app.control("desktop/keep-playing").wait_enabled, timeout=35)
        app.capture("save-timeout")
        app.click("desktop/keep-playing")
        retry_ui(app.control("desktop/keep-playing").wait_detached)
    wait(lambda: save_record(app.profile)[0]["revision"] > before,
         "the delayed checkpoint finishing after Keep playing")
    app.key("Escape")
    retry_ui(app.control("hud:inventory").wait_enabled)
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
        retry_ui(app.control("desktop/close-unsaved").wait_enabled, timeout=35)
        app.click("desktop/close-unsaved")
        retry_ui(app.control("launcher/solo").wait_enabled)
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
    retry_ui(solo.control("launcher/status").wait_until,
        lambda item: item is not None and "invite" in (item.name or "").lower(), timeout=10)
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
    retry_ui(returned.control("launcher/solo").wait_enabled, timeout=120)
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
    invite = guest.control("launcher/invite")
    invite.focus()
    retry_ui(invite.wait_focused)
    INPUT.chord("v", held=["Meta" if PLATFORM == "darwin" else "Ctrl"])
    retry_ui(invite.wait_until,
        lambda item: item is not None and (item.value or "").startswith("solomondarker://"),
        timeout=10)
    guest.click("launcher/join")
    guest.new_game("NativeGuest", "fire")
    assert not retry_ui(guest.control("hud:map").wait_attached).enabled, "the guest may not start the match"
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
    initial_ticks = {app.name: save_record(app.profile)[1]["continuation"]["simulation"]["tick"]
                     for app in (host, guest)}

    def advancing_boneyard():
        documents = {app.name: save_record(app.profile)[1] for app in (host, guest)}
        if all(document["continuation"]["simulation"]["world"]["runId"] == run_id
               and document["continuation"]["simulation"]["tick"] > initial_ticks[name] + 100
               for name, document in documents.items()):
            return documents
        return None

    # A saved checkpoint can arrive before the client's loading surface appears.
    # The shared clock advances only after both renderers are ready and the resume
    # hold ends. Observe that durable state before testing each player's input.
    playing = wait(advancing_boneyard, "both peer clocks advancing after renderer readiness")
    before_x = {name: player(document)["locomotions"]["position"]["x"]
                for name, document in playing.items()}
    for app in (host, guest):
        app.activate()
        retry_ui(app.control("match-loading").wait_detached, timeout=120)
        # Two software-rendered peers can exceed two seconds per frame.
        # Keep the key held long enough for gameplay to sample it.
        app.hold("d", 3)
        app.capture(f"peer-{app.name}-boneyard")
    # Closing inventory starts the ordinary two-second shared resume hold.
    # Exercise it after movement so the hold cannot consume either input probe.
    host.key("i")
    retry_ui(host.control("panel:close").wait_enabled)
    host.capture("peer-host-inventory")
    host.key("Escape")
    retry_ui(host.control("panel:close").wait_detached)
    guest_save = guest.close()[1]
    host_save = host.close()[1]
    assert identity(host_save)["displayName"] == "NativeHost"
    assert identity(guest_save)["displayName"] == "NativeGuest"
    assert all(document["continuation"]["simulation"]["world"]["runId"] == run_id
               for document in (host_save, guest_save))
    movement = {}
    for name, document in (("host", host_save), ("guest", guest_save)):
        after_x = player(document)["locomotions"]["position"]["x"]
        assert after_x > before_x[name] + 10, f"{name} did not move in the shared Boneyard"
        movement[name] = {"beforeX": before_x[name], "afterX": after_x}
    RECEIPT["peer"] = {"runId": run_id, "host": identity(host_save),
                       "guest": identity(guest_save), "guestMatchStartDisabled": True,
                       "initialTicks": initial_ticks,
                       "playingTicks": {name: document["continuation"]["simulation"]["tick"]
                                        for name, document in playing.items()},
                       "movement": movement, "hostInventoryOpened": True}


def cleanup():
    # Guests need their host alive to finish saving.
    for app in reversed(RUNNING):
        result = {"name": app.name}
        if app.process.poll() is None:
            try:
                app.request_close()
            except Exception as error:
                result["closeError"] = str(error)
            try:
                # An AX close can take effect even when its acknowledgement fails.
                # Give save-before-close its normal window before forcing cleanup.
                app.process.wait(timeout=30)
            except subprocess.TimeoutExpired:
                app.process.terminate()
                try:
                    app.process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    app.process.kill()
                    app.process.wait(timeout=5)
                result["forced"] = True
        if not app.closed:
            result["code"] = app.process.returncode
            RECEIPT["cleanup"].append(result)
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
        print(f"{app.name}: process status {app.process.poll()}", flush=True)
        log_tail = re.sub(r"solomondarker://[^\s\"']+", "",
                          app.log_path.read_text(errors="replace")[-8000:])
        print(f"{app.name}: application log\n{log_tail}", flush=True)
        if app.process.poll() is None:
            if PLATFORM == "darwin":
                try:
                    sample = EVIDENCE / f"{app.name}-failure.sample.txt"
                    subprocess.run(["sample", str(app.process.pid), "2", "10", "-file", str(sample)],
                                   capture_output=True, text=True, timeout=10, check=True)
                except Exception as error:
                    RECEIPT.setdefault("diagnosticErrors", []).append(str(error))
            try:
                app.capture(f"{app.name}-failure", activate=False)
            except Exception as error:
                RECEIPT.setdefault("diagnosticErrors", []).append(str(error))
    raise
finally:
    cleanup()
    write_json(EVIDENCE / "receipt.json", RECEIPT)
    print(json.dumps(RECEIPT, indent=2))
