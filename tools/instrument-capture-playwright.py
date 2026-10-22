"""Temporary, pinned SDK timing only; never writes protocol payloads or errors."""
import hashlib
import json
import pathlib

bundle = pathlib.Path('node_modules/playwright-core/lib/coreBundle.js')
original = bundle.read_bytes()
expected = '6be5c2ea035554e9b184b1dbc7aa5e7f1fb428dd1b5c202022858dcfae9bee27'
assert hashlib.sha256(original).hexdigest() == expected, 'Unexpected Playwright 1.61.1 bundle'
source = original.decode()
replacements = [
    ('const viewportSize = await this._originalViewportSize(progress2);',
     'const viewportSize = await __sdrCaptureTimed("originalViewportSize", () => this._originalViewportSize(progress2));', 2),
    ('await this._preparePageForScreenshot(progress2, this._page.mainFrame(), options.style, options.caret !== "initial", options.animations === "disabled");',
     'await __sdrCaptureTimed("preparePageForScreenshot", () => this._preparePageForScreenshot(progress2, this._page.mainFrame(), options.style, options.caret !== "initial", options.animations === "disabled"));', 1),
    ('frame.nonStallingEvaluateInExistingContext("document.fonts.ready", "utility").catch(() => {\n            })',
     '__sdrCaptureTimed("document.fonts.ready", () => frame.nonStallingEvaluateInExistingContext("document.fonts.ready", "utility").catch(() => {\n            }))', 1),
    ('this._mainFrameSession._client.send("Page.getLayoutMetrics")',
     '__sdrCaptureTimed("Page.getLayoutMetrics", () => this._mainFrameSession._client.send("Page.getLayoutMetrics"))', 1),
    ('this._mainFrameSession._client.send("Page.captureScreenshot", { format: format2, quality, clip, captureBeyondViewport: !fitsViewport })',
     '__sdrCaptureTimed("Page.captureScreenshot", () => this._mainFrameSession._client.send("Page.captureScreenshot", { format: format2, quality, clip, captureBeyondViewport: !fitsViewport }))', 1),
]
for old, new, count in replacements:
    assert source.count(old) == count, (old, source.count(old), count)
    source = source.replace(old, new)
prefix = r'''
// SDR_CAPTURE_DIAGNOSTIC: timing and method names only; no protocol payloads.
let __sdrCaptureSequence = 0;
function __sdrCaptureLog(row) {
  const path = process.env.SDR_CAPTURE_PROTOCOL_LOG;
  if (!path) return;
  const fs = require('node:fs');
  const line = JSON.stringify({wallTime: new Date().toISOString(), monotonicMs: Number(process.hrtime.bigint()) / 1e6, ...row}) + '\n';
  let bytes = 0;
  try { bytes = fs.statSync(path).size; } catch (error) { if (error.code !== 'ENOENT') throw error; }
  if (bytes + Buffer.byteLength(line) <= 65536) fs.appendFileSync(path, line);
}
async function __sdrCaptureTimed(method, operation) {
  const id = ++__sdrCaptureSequence;
  const start = Number(process.hrtime.bigint()) / 1e6;
  __sdrCaptureLog({id, method, event: 'begin'});
  try {
    const value = await operation();
    __sdrCaptureLog({id, method, event: 'end', durationMs: Number(process.hrtime.bigint()) / 1e6 - start});
    return value;
  } catch (error) {
    __sdrCaptureLog({id, method, event: 'failed', durationMs: Number(process.hrtime.bigint()) / 1e6 - start});
    throw error;
  }
}
'''
assert source.startswith('"use strict";\n')
patched = ('"use strict";\n' + prefix + source.removeprefix('"use strict";\n')).encode()
bundle.write_bytes(patched)
receipt = {'dependency': 'playwright-core', 'version': '1.61.1', 'originalSha256': expected,
           'instrumentedSha256': hashlib.sha256(patched).hexdigest(),
           'scope': 'capture stage timing only; no parameters, results, URLs or errors logged',
           'replacementCounts': [count for _, _, count in replacements]}
pathlib.Path('reports/windows-capture-control').mkdir(parents=True, exist_ok=True)
pathlib.Path('reports/windows-capture-control/sdk-timing-patch.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt))
