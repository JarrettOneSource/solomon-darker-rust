#!/usr/bin/env python3
"""Prepare root-origin WASM assets for the existing desktop script policy."""

import argparse
from pathlib import Path
import re

from trunk_bootstrap import (
    INLINE_MODULE,
    bootstrap_name,
    externalize_trunk_bootstrap,
    validate_trunk_bootstrap,
)


def prepare_desktop_client(dist: Path) -> str:
    index = dist / "index.html"
    document = index.read_text(encoding="utf-8")
    if re.search(r'<meta\s+name=[\'"]solomon-(?:website-mount|game-mount|editor-test|build-revision)[\'"]', document):
        raise ValueError("Desktop requires a root Trunk build without Website/editor metadata")
    if INLINE_MODULE.search(document):
        document, name, bootstrap = externalize_trunk_bootstrap(document, "/")
        (dist / name).write_text(bootstrap, encoding="utf-8")
        index.write_text(document, encoding="utf-8")
        return name
    # A prebuilt desktop distribution may already have been prepared by dev or
    # packaging. Validate this exact owned initializer instead of rewriting it.
    scripts = re.findall(r'<script type="module" src="([^"]+)"></script>', document)
    if len(scripts) != 1 or not re.fullmatch(r"/solomon-boot-[0-9a-f]{16}\.mjs", scripts[0]):
        raise ValueError("Desktop requires a root Trunk initializer")
    name = scripts[0][1:]
    bootstrap = (dist / name).read_text(encoding="utf-8")
    validate_trunk_bootstrap(bootstrap, "/")
    if bootstrap_name(bootstrap) != name:
        raise ValueError("The desktop initializer does not match its content hash")
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
