"""Private host trace and conditional input-release proposal for a fatal frame."""
from pathlib import Path
import json
import subprocess

subprocess.run(["git", "apply", "--check", "tools/diagnostic-failure-input.patch"], check=True)
subprocess.run(["git", "apply", "tools/diagnostic-failure-input.patch"], check=True)
subprocess.run(["git", "apply", "--check", "tools/diagnostic-fatal-release.patch"], check=True)
subprocess.run(["git", "apply", "tools/diagnostic-fatal-release.patch"], check=True)
subprocess.run(["python", "tools/diagnostic-fatal-inject.py"], check=True)

p = Path("crates/web/src/game.rs")
source = p.read_text()
needle = "        crate::runtime_ui::set_error(error);\n        self.input.set_blocked(true)?;\n"
assert source.count(needle) == 1
source = source.replace(needle, '''        crate::runtime_ui::set_error(error);
        eprintln!("PRIVATE_FATAL_FAILURE {}", serde_json::json!({
            "unixSeconds": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64(),
        }));
        if std::env::var_os("PRIVATE_FATAL_RELEASE_INPUT").is_none() {
            return Ok(());
        }
        self.input.set_blocked(true)?;
''')
p.write_text(source, newline="\n")

p = Path("crates/launcher/src/host/input.rs")
source = p.read_text()
needle = "        self.active\n    }\n\n    /// `stopAllClientInputs`"
assert source.count(needle) == 1
source = source.replace(needle, '''        if next_tick.is_multiple_of(10)
            && std::env::var_os("SDR_FATAL_STATUS_TRIGGER")
                .is_some_and(|path| std::path::Path::new(&path).exists())
        {
            eprintln!("PRIVATE_FATAL_AUTHORITY_INPUT {}", serde_json::json!({
                "unixSeconds": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64(),
                "tick": next_tick,
                "movement": [self.active.movement.x, self.active.movement.y],
                "primary": self.active.primary,
                "quickbar": self.active.quickbar,
            }));
        }
        self.active
    }

    /// `stopAllClientInputs`''')
p.write_text(source, newline="\n")
out = Path("reports/native-fatal-authority")
out.mkdir(parents=True, exist_ok=True)
(out / "private-instrumentation.patch").write_bytes(subprocess.check_output(["git", "diff"]))
(out / "proposed-release.patch").write_bytes(Path("tools/diagnostic-fatal-release.patch").read_bytes())
