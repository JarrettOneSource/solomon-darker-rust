"""Private before/after check for the captured native audio underrun failure."""
from pathlib import Path
import sys
p=Path('crates/web/src/audio/native.rs')
if sys.argv[1] == 'red':
    p.write_bytes(Path('tools/diagnostic-native-audio.rs').read_bytes())
else:
    assert sys.argv[1] == 'green'
    source=p.read_text()
    old='fn record_output_error(error: &Mutex<Option<String>>, event: rodio::cpal::StreamError) {\n    *error.lock().expect("audio error lock") = Some(format!("Native audio output: {event}"));\n}'
    new='fn record_output_error(error: &Mutex<Option<String>>, event: rodio::cpal::StreamError) {\n    if matches!(event, rodio::cpal::StreamError::BufferUnderrun) {\n        // CPAL recovers an underrun; a failed recovery arrives as a separate backend error.\n        crate::platform::warn(&format!("Native audio output: {event}"));\n    } else {\n        *error.lock().expect("audio error lock") = Some(format!("Native audio output: {event}"));\n    }\n}'
    assert source.count(old) == 1
    p.write_text(source.replace(old,new))
