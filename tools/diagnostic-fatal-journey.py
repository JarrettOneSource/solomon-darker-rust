"""Exercise fatal recovery through accessibility and ordinary pointer input."""
from pathlib import Path
import os
import subprocess
import sys

mode = sys.argv[1]
assert mode in ("accessibility", "pointer"), mode
source_path = Path('tools/smoke-desktop-runtime.py')
source = source_path.read_text()
needle = '    check_offline()\n    check_peers()\n'
assert source.count(needle) == 1
journey = '''    profile = PROFILES / "fatal-status"
    app = Application("fatal-status", profile)
    app.click("launcher/solo")
    app.new_game("Failure", "ether")
    Path(os.environ["SDR_FATAL_STATUS_TRIGGER"]).touch()
    message = "Native audio output: The stream configuration is no longer valid and must be rebuilt."
    status = app.app.locator("status[name=" + json.dumps(message) + "]")
    read_ui(status.wait_attached)
    app.capture("fatal-status-visible")
    action = read_ui(app.control("desktop/launcher").wait_enabled)
    assert action.bounds is not None
    window = read_ui(app.app.locator("window").first().wait_attached)
    shot = xa11y.screenshot(element=window)
    shot.save_png(EVIDENCE / "fatal-status-pixels.png")
    pixels = shot.pixels
    red = [(i // 4 % shot.width, i // 4 // shot.width) for i in range(0, len(pixels), 4)
           if pixels[i] > 160 and pixels[i] > pixels[i + 1] * 1.5
           and abs(pixels[i + 1] - pixels[i + 2]) < 12]
    assert len(red) > 100, "the actual fatal explanation has no visible red text"
    left, top = min(p[0] for p in red), min(p[1] for p in red)
    right, bottom = max(p[0] for p in red), max(p[1] for p in red)
    assert abs((left + right) / 2 - shot.width / 2) < shot.width * 0.1, "fatal text is not horizontally centered"
    assert abs((top + bottom) / 2 - shot.height / 2) < shot.height * 0.1, "fatal text is not vertically centered"
    assert shot.mapping_available
    _, text_bottom = shot.image_to_desktop(left, bottom)
    assert text_bottom < action.bounds.y, "fatal explanation overlaps recovery controls"
    RECEIPT["fatalTextPixels"] = {"count": len(red), "bounds": [left, top, right, bottom],
                                  "imageSize": [shot.width, shot.height],
                                  "textBottom": text_bottom, "actionTop": action.bounds.y}
    RECEIPT["recoveryInput"] = RECOVERY_INPUT
    if RECOVERY_INPUT == "accessibility":
        app.click("desktop/launcher")
    else:
        INPUT.click(action)
    read_ui(app.control("launcher/solo").wait_enabled, timeout=30)
    app.capture("returned-after-failure")
    record = save_record(profile)
    assert record is not None and identity(record[1])["displayName"] == "Failure"
    RECEIPT["savedAfterFailure"] = True
    write_json(EVIDENCE / "saved-after-failure.json", record[0])
    app.request_close()
    app.exited()
'''
journey = '    RECOVERY_INPUT = ' + repr(mode) + '\n' + journey
source = source.replace(needle, journey)
source = source.replace('    shutil.rmtree(PROFILES)\n', '''    for app in RUNNING:
        record = save_record(app.profile)
        if record:
            write_json(EVIDENCE / f"{app.name}-cleanup-save.json", record[0])
    shutil.rmtree(PROFILES)
''')
out = Path('reports/native-fatal-status') / mode
out.mkdir(parents=True, exist_ok=True)
driver = out / 'driver.py'
driver.write_text(source)
env = os.environ.copy()
env['SDR_DESKTOP_EVIDENCE'] = str((out / 'journey').resolve())
env['SDR_FATAL_STATUS_TRIGGER'] = str((out / 'trigger').resolve())
result = subprocess.run([sys.executable, '-u', str(driver)], env=env)
raise SystemExit(result.returncode)
