#!/usr/bin/env python3
"""Build a self-contained game directory and ZIP with the installed Rust/Trunk toolchain."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import zipfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="new package directory")
    parser.add_argument("--profile", choices=("debug", "release"), default="release")
    parser.add_argument("--web-dist", type=Path, help="reuse an already verified Trunk dist directory")
    parser.add_argument("--no-archive", action="store_true", help="leave only the launchable directory")
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    output = args.output.resolve()
    archive = output.with_suffix(".zip")
    if output.exists() or (not args.no_archive and archive.exists()):
        parser.error("the output directory or archive already exists")

    cargo = ["cargo", "build", "-p", "solomon-launcher"]
    if args.profile == "release":
        cargo.append("--release")
    subprocess.run(cargo, cwd=root, check=True)
    web_dist = args.web_dist.resolve() if args.web_dist else root / "web/dist"
    if not args.web_dist:
        if not (root / "web/assets/stock/College.png").is_file():
            parser.error("the tracked stock client assets are missing")
        trunk = ["trunk", "build"]
        if args.profile == "release":
            trunk.append("--release")
        subprocess.run(trunk, cwd=root / "web", check=True)
    if not (web_dist / "index.html").is_file() or not any(web_dist.glob("*.wasm")):
        parser.error("the web distribution needs index.html and a compiled wasm module")

    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=root, text=True))
    target = Path(metadata["target_directory"])
    executable = "solomon-darker.exe" if sys.platform == "win32" else "solomon-darker"
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    dirty = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=root, text=True))
    subprocess.run([sys.executable, str(root / "scripts/verify-client-assets.py")], check=True)
    output.parent.mkdir(parents=True, exist_ok=True)
    # Build beside the destination, then publish the complete directory in one rename.
    with tempfile.TemporaryDirectory(prefix=".solomon-package-", dir=output.parent) as staging:
        package = Path(staging) / output.name
        package.mkdir()
        shutil.copy2(target / args.profile / executable, package / executable)
        shutil.copytree(web_dist, package / "game")
        (package / "README.txt").write_text(
            "Solomon Darker\n\n"
            "Run solomon-darker (solomon-darker.exe on Windows). On macOS you can\n"
            "double-click Play Solomon Darker.command. The launcher opens your browser.\n"
            "Keep its terminal window open while playing; press Ctrl+C to quit.\n\n"
            "The game is bundled here. Node, Rust, Trunk and the Website checkout are\n"
            "not needed to play. Use a browser with WebGL2 enabled.\n\n"
            "The launcher saves the shared game in your application data folder:\n"
            "  macOS: ~/Library/Application Support/Solomon Darker/host-4188.json\n"
            "  Windows: %LOCALAPPDATA%/Solomon Darker/host-4188.json\n"
            "  Linux: $XDG_DATA_HOME/Solomon Darker/host-4188.json, or\n"
            "         ~/.local/share/Solomon Darker/host-4188.json\n"
            "Other ports use their own host-PORT.json. Use Leave Game to save\n"
            "before returning to the title. Preferences and your wizard's resume\n"
            "credential stay in the browser, so use the same browser/profile and\n"
            "address to Continue after restarting the launcher.\n\n"
            "For a direct multiplayer game, use --listen ADDRESS and share\n"
            "http://ADDRESS:4188/?host=1 with the other players. The launcher\n"
            "must remain running for everyone in its shared game.\n\n"
            "Advanced: --no-open, --port NUMBER, --listen ADDRESS, --save FILE,\n"
            "          --assets DIRECTORY, --help.\n",
            encoding="utf-8",
        )
        if sys.platform == "darwin":
            launch = package / "Play Solomon Darker.command"
            launch.write_text(
                '#!/bin/sh\ncd -- "$(dirname -- "$0")" || exit 1\nexec ./solomon-darker\n',
                encoding="utf-8",
            )
            launch.chmod(0o755)
        (package / "build.json").write_text(json.dumps({
            "revision": revision,
            "workingTreeChanges": dirty,
            "platform": sys.platform,
            "launcherProfile": args.profile,
            "reusedWebDistribution": args.web_dist is not None,
        }, indent=2) + "\n", encoding="utf-8")
        package.rename(output)
    if not args.no_archive:
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=3) as bundle:
            for path in sorted(output.rglob("*")):
                if path.is_file():
                    bundle.write(path, path.relative_to(output.parent))
    print(json.dumps({"package": str(output), "archive": None if args.no_archive else str(archive)}))


if __name__ == "__main__":
    main()
