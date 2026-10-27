"""Private real-failure layout regression and ordinary OS recovery journey."""
from pathlib import Path
import subprocess,sys
mode=sys.argv[1]
if mode == 'red':
    subprocess.run(['git','apply','tools/diagnostic-fatal-status.patch'],check=True)
elif mode == 'green':
    p=Path('crates/web/src/runtime_ui.rs');s=p.read_text()
    old="pub(crate) fn paint_failure(frame: &mut UiFrame<'_>) {\n    paint(frame);\n}"
    new='pub(crate) fn paint_failure(frame: &mut UiFrame<\'_>) {\n    let Some(message) = error() else { return };\n    let painter = frame.ctx.layer_painter(layer_id());\n    let screen = frame.ctx.content_rect();\n    let color = Color32::from_rgb(255, 123, 123);\n    let text = painter.layout(\n        message.clone(),\n        FontId::new(14.0, FontFamily::Name("Arial".into())),\n        color,\n        (screen.width() - 48.0).clamp(1.0, 720.0),\n    );\n    let bounds = egui::Rect::from_center_size(screen.center(), text.size());\n    painter.galley(bounds.min, text, color);\n    frame.record_rect("runtime:status", &message, "status", bounds, true);\n    frame.record_value("runtime:status", &message, None);\n    frame.announce("runtime:status", &message);\n}'
    assert s.count(old)==1
    p.write_text(s.replace(old,new),newline='\n')
elif mode == 'inject':
    p=Path('crates/web/src/game.rs');s=p.read_text();needle='        self.poll_native(time)?;\n'
    assert s.count(needle)==1
    p.write_text(s.replace(needle,needle+'        if std::env::var_os("SDR_FATAL_STATUS_TRIGGER").is_some_and(|path| std::path::Path::new(&path).exists()) {\n            return Err("Native audio output: The stream configuration is no longer valid and must be rebuilt.".into());\n        }\n'),newline='\n')
    p=Path('tools/smoke-desktop-runtime.py');s=p.read_text();needle='    check_offline()\n    check_peers()\n'
    assert s.count(needle)==1
    p.write_text(s.replace(needle,'    profile = PROFILES / "fatal-status"\n    app = Application("fatal-status", profile)\n    app.click("launcher/solo")\n    app.new_game("FailureReview", "ether")\n    Path(os.environ["SDR_FATAL_STATUS_TRIGGER"]).touch()\n    message = "Native audio output: The stream configuration is no longer valid and must be rebuilt."\n    wait(lambda: message in app.app.dump(), "the actual fatal status")\n    app.capture("fatal-status-visible")\n    status = read_ui(app.control("runtime:status").wait_attached)\n    action = read_ui(app.control("desktop/launcher").wait_enabled)\n    assert status.bounds is not None and action.bounds is not None\n    assert status.bounds.y + status.bounds.height < action.bounds.y, "error overlaps actions"\n    app.click("desktop/launcher")\n    read_ui(app.control("launcher/solo").wait_enabled, timeout=30)\n    app.capture("returned-after-failure")\n    record = save_record(profile)\n    assert record is not None and identity(record[1])["displayName"] == "FailureReview"\n    RECEIPT["savedAfterFailure"] = True\n    app.request_close()\n    app.exited()\n'),newline='\n')
else:
    raise ValueError(mode)
