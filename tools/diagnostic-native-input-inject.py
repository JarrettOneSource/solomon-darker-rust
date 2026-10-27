"""Private read-only native input trace for the missed 1.2-second peer input."""
from pathlib import Path
import hashlib
import json

root = Path("reports/native-focus")
root.mkdir(parents=True, exist_ok=True)
edits = []

def replace(path, needle, replacement):
    target = Path(path)
    source = target.read_text()
    assert source.count(needle) == 1, path
    changed = source.replace(needle, replacement)
    target.write_text(changed)
    edits.append({"path": path, "oldSha256": hashlib.sha256(source.encode()).hexdigest(),
                  "newSha256": hashlib.sha256(changed.encode()).hexdigest()})

needle = "        self.dispatch_ui_input(time)?;\n        self.gpu_ui.window_event(event)?;\n"
replacement = '''        if matches!(event, winit::event::WindowEvent::KeyboardInput { .. }
            | winit::event::WindowEvent::Focused(_) | winit::event::WindowEvent::Occluded(_)) {
            eprintln!("PRIVATE_INPUT_EVENT unix={:?} now={time} event={event:?}",
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH));
        }
''' + needle
replace("crates/web/src/game/desktop.rs", needle, replacement)

needle = "        let input = self.input.sample()?;\n"
replacement = needle + '''        #[cfg(not(target_arch = "wasm32"))]
        if self.session.boneyard.is_some() {
            eprintln!("PRIVATE_INPUT_SAMPLE unix={:?} now={time} tick={} blocked={blocked} native_held={} world_loading={} match_loading={} input={input:?}",
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH),
                self.session.ticks, self.native_gameplay_held(), self.world_loading,
                self.match_loading.is_some());
        }
'''
replace("crates/web/src/game.rs", needle, replacement)

needle = "        if state.world_input_blocked() {\n            return Ok(GameplayInputSample {\n"
replacement = '''        #[cfg(not(target_arch = "wasm32"))]
        eprintln!("PRIVATE_INPUT_STATE unix={:?} blocked={} ui_text={} ui_scope={:?} held={:?}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH),
            state.blocked, state.ui.text, state.ui.scope, state.held);
''' + needle
replace("crates/web/src/input.rs", needle, replacement)
(root / "input-trace-source.json").write_text(json.dumps(edits, indent=2) + "\n")
