#!/usr/bin/env python3
"""Prepare root-origin WASM assets for the existing desktop script policy."""

import argparse
from pathlib import Path
import re

from trunk_bootstrap import prepare_trunk_bootstrap


def prepare_desktop_client(dist: Path) -> str:
    index = dist / "index.html"
    document = index.read_text(encoding="utf-8")
    if re.search(r'<meta\s+name=[\'"]solomon-(?:website-mount|game-mount|editor-test|build-revision)[\'"]', document):
        raise ValueError("Desktop requires a root Trunk build without Website/editor metadata")
    bases = re.findall(r'<base\b[^>]*href=[\'"]([^\'"]+)[\'"][^>]*>', document)
    if bases not in ([], ["/"]):
        raise ValueError("Desktop requires a root Trunk public URL")
    document, name, _ = prepare_trunk_bootstrap(dist, "/")
    # /game is a root-origin document; its native base already resolves root assets.
    # Remove Trunk's redundant base to honor the desktop's base-uri 'none' policy.
    document = re.sub(r'<base\b[^>]*>', '', document)
    index.write_text(document, encoding="utf-8")
    for lock in dist.rglob(".trunk-lock"):
        lock.unlink()
    return name


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dist", type=Path, help="completed root-public-url Trunk distribution")
    args = parser.parse_args()
    try:
        print(prepare_desktop_client(args.dist))
    except (OSError, ValueError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
