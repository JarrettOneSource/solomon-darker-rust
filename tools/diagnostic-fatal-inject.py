"""Enter the existing fatal-frame path after a native file signal."""
from pathlib import Path
p = Path("crates/web/src/game.rs")
source = p.read_text()
needle = "        self.poll_native(time)?;\n"
assert source.count(needle) == 1
source = source.replace(needle, needle + '        if std::env::var_os("SDR_FATAL_STATUS_TRIGGER").is_some_and(|path| std::path::Path::new(&path).exists()) {\n            return Err("Native audio output: The stream configuration is no longer valid and must be rebuilt.".into());\n        }\n')
p.write_text(source, newline="\n")
