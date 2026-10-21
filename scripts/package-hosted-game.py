#!/usr/bin/env python3
"""Build a verified native host and mounted WASM client for the Website deployment."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def run(command, cwd=ROOT):
    subprocess.run(command, cwd=cwd, check=True)


def output(command):
    return subprocess.check_output(command, cwd=ROOT, text=True).strip()


def bindgen():
    lock = tomllib.loads((ROOT / 'Cargo.lock').read_text())
    version = next(package['version'] for package in lock['package']
                   if package['name'] == 'wasm-bindgen')
    executable = 'wasm-bindgen.exe' if sys.platform == 'win32' else 'wasm-bindgen'
    candidates = [shutil.which('wasm-bindgen')]
    for cache in [Path.home() / 'Library/Caches/dev.trunkrs.trunk',
                  Path(os.environ.get('XDG_CACHE_HOME', Path.home() / '.cache')) / 'trunk']:
        candidates.append(cache / f'wasm-bindgen-{version}' / executable)
    for candidate in candidates:
        if candidate and Path(candidate).is_file():
            if output([str(candidate), '--version']) == f'wasm-bindgen {version}':
                return str(candidate)
    raise SystemExit(f'Install wasm-bindgen-cli {version}, or let Trunk install its matching CLI.')


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True, help='new artifact directory')
    parser.add_argument('--mount', default='/game', help='game artifact URL prefix')
    parser.add_argument('--website-mount', default='', help='Website API and storage URL prefix')
    parser.add_argument('--profile', choices=('debug', 'release'), default='release')
    parser.add_argument('--target', help='native Rust target; default is the installed host')
    args = parser.parse_args()
    destination = args.output.resolve()
    archive = destination.with_name(destination.name + '.tar.gz')
    if destination.exists() or archive.exists():
        parser.error('output directory and archive must not already exist')
    if not re.fullmatch(r'/[a-z0-9/-]+', args.mount) or args.mount.endswith('/'):
        parser.error('mount must be an absolute path without a trailing slash')
    if output(['git', 'status', '--porcelain']):
        parser.error('commit the complete reviewed source before building a release artifact')
    revision = output(['git', 'rev-parse', 'HEAD'])
    host = next(line.removeprefix('host: ') for line in output(['rustc', '-vV']).splitlines()
                if line.startswith('host: '))
    native_target = args.target or host
    target = Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target')).resolve()
    executable = 'solomon-darker.exe' if 'windows' in native_target else 'solomon-darker'
    release = ['--release'] if args.profile == 'release' else []
    native_flags = ['--target', args.target] if args.target else []
    destination.parent.mkdir(parents=True, exist_ok=True)
    run([sys.executable, 'scripts/verify-client-assets.py'])
    with tempfile.TemporaryDirectory(prefix='.solomon-hosted-', dir=destination.parent) as work:
        package = Path(work) / 'artifact'
        client = package / 'client'
        native = package / 'bin' / executable
        native.parent.mkdir(parents=True)
        run(['cargo', 'build', '-p', 'solomon-launcher', '--bin', 'solomon-darker', *release, *native_flags])
        built_native = target / args.target if args.target else target
        shutil.copy2(built_native / args.profile / executable, native)
        run(['trunk', 'build', '--config', 'web/Trunk.toml', *release,
             '--public-url', args.mount + '/', '--dist', str(client)])
        run([sys.executable, 'scripts/prepare-hosted-client.py', str(client),
             '--mount', args.mount, '--website-mount', args.website_mount, '--revision', revision])
        run(['cargo', 'build', '-p', 'solomon-save-tools', '--target', 'wasm32-unknown-unknown', *release])
        run([bindgen(), '--target', 'web', '--out-dir', str(client / 'save-tools'),
             str(target / 'wasm32-unknown-unknown' / args.profile / 'solomon_save_tools.wasm')])
        for lock in client.rglob('.trunk-lock'):
            lock.unlink()
        if output(['git', 'rev-parse', 'HEAD']) != revision or output(['git', 'status', '--porcelain']):
            raise SystemExit('Source changed during the artifact build; no artifact was published')
        files = {}
        for path in sorted(package.rglob('*')):
            if path.is_symlink():
                raise SystemExit(f'Artifact input cannot be a symlink: {path}')
            if path.is_file():
                files[path.relative_to(package).as_posix()] = digest(path)
        manifest = {'schemaVersion': 1, 'revision': revision, 'target': native_target, 'files': files}
        (package / 'game-artifact.json').write_text(json.dumps(manifest, indent=2) + '\n')
        staged_archive = Path(work) / 'artifact.tar.gz'
        with tarfile.open(staged_archive, 'w:gz') as tar:
            for name in ['game-artifact.json', 'bin', 'client']:
                tar.add(package / name, arcname=name)
        package.rename(destination)
        staged_archive.rename(archive)
    print(json.dumps({'revision': revision, 'target': native_target, 'directory': str(destination),
                      'archive': str(archive), 'sha256': digest(archive), 'files': len(files)}))


if __name__ == '__main__':
    main()
