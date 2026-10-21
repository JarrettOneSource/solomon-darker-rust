#!/usr/bin/env python3
"""Verify the tracked stock client inputs, or record an intentional asset import."""

import argparse
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / 'web/assets/stock'
MANIFEST = ROOT / 'web/assets/stock-files.json'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--record', action='store_true', help='record the reviewed stock inputs')
    args = parser.parse_args()
    actual = {}
    for path in sorted(ASSETS.rglob('*')):
        if path.is_symlink():
            raise SystemExit(f'Client asset cannot be a symlink: {path}')
        if path.is_file():
            actual[path.relative_to(ASSETS).as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
    if not actual:
        raise SystemExit('The tracked client assets are missing')
    catalog = (ROOT / 'crates/sim/src/audio/catalog.rs').read_text()
    audio = re.findall(r'id: "[^\"]+",\s*path: "([^\"]+)"', catalog)
    if not audio:
        raise SystemExit('The native audio catalog has no entries')
    missing = sorted({f'audio/{path}' for path in audio} - actual.keys())
    if missing:
        raise SystemExit('Native audio inputs are missing: ' + ', '.join(missing))
    if args.record:
        MANIFEST.write_text(json.dumps(actual, indent=2, sort_keys=True) + '\n')
    else:
        declared = json.loads(MANIFEST.read_text())
        changed = sorted(name for name in actual.keys() | declared.keys()
                         if actual.get(name) != declared.get(name))
        if changed:
            raise SystemExit('Client assets differ from their reviewed manifest: ' + ', '.join(changed))
    print(f'Verified {len(actual)} stock files and {len(audio)} native audio entries.')


if __name__ == '__main__':
    main()
