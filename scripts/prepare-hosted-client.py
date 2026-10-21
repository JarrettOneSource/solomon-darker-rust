#!/usr/bin/env python3
"""Externalize Trunk's generated bootstrap for the hosted site's script policy."""

import argparse
import json
from pathlib import Path
import re

from trunk_bootstrap import externalize_trunk_bootstrap


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dist", type=Path, help="completed Trunk distribution")
    parser.add_argument("--mount", default="/game-stg")
    parser.add_argument("--website-mount", help="Website API and account-storage prefix; defaults to the game mount")
    parser.add_argument("--revision", required=True, help="full source revision of this build")
    args = parser.parse_args()
    if not re.fullmatch(r"/[a-z0-9/-]+", args.mount) or args.mount.endswith("/"):
        parser.error("mount must be an absolute URL path without a trailing slash")
    website_mount = args.mount if args.website_mount is None else args.website_mount
    if website_mount and (not re.fullmatch(r"/[a-z0-9/-]+", website_mount)
                          or website_mount.endswith("/") or "//" in website_mount):
        parser.error("website-mount must be empty or an absolute URL path without a trailing slash")
    if not re.fullmatch(r"[0-9a-f]{40}", args.revision):
        parser.error("revision must be a full lowercase source commit")
    index = args.dist / "index.html"
    document = index.read_text(encoding="utf-8")
    try:
        document, name, bootstrap = externalize_trunk_bootstrap(document, args.mount)
    except ValueError as error:
        parser.error(str(error))
    (args.dist / name).write_text(bootstrap, encoding="utf-8")
    if 'name="solomon-build-revision"' in document or document.count("</head>") != 1:
        parser.error("expected one document head without preexisting deployment metadata")
    document = document.replace("</head>", (
        f'<base href="{args.mount}/">\n'
        f'<meta name="solomon-build-revision" content="{args.revision}">\n'
        f'<meta name="solomon-game-mount" content="{args.mount}">\n'
        f'<meta name="solomon-website-mount" content="{website_mount}">\n</head>'
    ))
    index.write_text(document, encoding="utf-8")
    editor_boot = "editor-test-boot.mjs"
    (args.dist / editor_boot).write_text(
        (Path(__file__).resolve().parents[1] / "web" / editor_boot).read_text(encoding="utf-8"),
        encoding="utf-8",
    )
    editor_document = document.replace(
        f'<script type="module" src="{args.mount}/{name}"></script>',
        f'<script type="module" src="{args.mount}/{editor_boot}" '
        f'data-editor-test-boot data-game-boot="{args.mount}/{name}"></script>',
    ).replace("</head>", '<meta name="solomon-editor-test">\n</head>')
    editor = args.dist / "editor-test"
    editor.mkdir(exist_ok=True)
    (editor / "index.html").write_text(editor_document, encoding="utf-8")
    (args.dist / "deployment.json").write_text(
        json.dumps({"revision": args.revision}) + "\n", encoding="utf-8"
    )
    print(name)


if __name__ == "__main__":
    main()
