#!/usr/bin/env python3
"""Inspect the final desktop ZIP before it can be uploaded as a release artifact."""

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import posixpath
import stat
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[1]
BUILD_RESIDUE = {
    '.trunk-lock', '.git', 'node_modules', 'target', 'reports', 'logs', '__pycache__',
    '.cache', '.ds_store', 'gpucache', 'code cache', 'indexeddb', 'session storage',
    'local storage', 'crashpad', 'crash reports', 'mods', 'default-mods',
}
SHELL_FILES = (
    'main.mjs', 'preload.cjs', 'static-client-server.mjs', 'local-game-host.mjs',
    'runtime-service.mjs', 'local-peer.mjs', 'run-peer-service.mjs', 'peer-network.mjs',
    'updates.mjs', 'package.json',
)
LAUNCHER_FILES = ('launcher.html', 'launcher.css', 'launcher.mjs')


def verify_archive(archive_path, source_root, revision):
    with zipfile.ZipFile(archive_path) as archive:
        entries = archive.infolist()
        names = {entry.filename for entry in entries}
        if len(names) != len(entries):
            raise ValueError('Duplicate archive entries')
        for entry in entries:
            name = entry.filename
            parts = PurePosixPath(name).parts
            if name.startswith('/') or '..' in parts or '\\' in name:
                raise ValueError(f'Unsafe archive path: {name}')
            if any(part.lower() in BUILD_RESIDUE for part in parts) or name.lower().endswith(
                    ('.sdmod', '.log', '.pdb', '.pyc')):
                raise ValueError(f'Build residue: {name}')

        manifests = [name for name in names if name.endswith('/desktop-package-manifest.json')
                     and not name.startswith('__MACOSX/')]
        if len(manifests) != 1:
            raise ValueError('Expected one desktop package manifest')
        manifest = json.loads(archive.read(manifests[0]))
        target = f"{manifest['platform']}-{manifest['arch']}"
        if target not in ('linux-x64', 'win32-x64', 'darwin-x64', 'darwin-arm64'):
            raise ValueError(f'Unsupported desktop target: {target}')
        root = f'Solomon Darker-{target}/'
        if manifests[0] != root + 'desktop-package-manifest.json':
            raise ValueError('Desktop package directory differs from its target')
        macos = manifest['platform'] == 'darwin'
        app = root + ('Solomon Darker.app/Contents/Resources/app/' if macos else 'resources/app/')

        hashes = {}
        for entry in entries:
            name = entry.filename
            if not name.startswith(root) and not (macos and name.startswith('__MACOSX/')):
                raise ValueError(f'Unexpected archive root: {name}')
            # Reading every member also checks ZIP CRCs, including runtime files
            # which do not have a source or native-authority digest below.
            with archive.open(entry) as stream:
                digest = hashlib.file_digest(stream, 'sha256').hexdigest()
            if entry.is_dir():
                continue
            hashes[name] = digest
            if stat.S_ISLNK(entry.external_attr >> 16):
                link = archive.read(entry).decode()
                destination = posixpath.normpath(posixpath.join(posixpath.dirname(name), link))
                if link.startswith('/') or '\\' in link or not destination.startswith(root):
                    raise ValueError(f'Symlink escapes the package: {name}')
            if name.startswith('__MACOSX/'):
                parent, leaf = posixpath.split(name.removeprefix('__MACOSX/'))
                counterpart = posixpath.join(parent, leaf.removeprefix('._'))
                if (not leaf.startswith('._') or
                        not (counterpart in names or counterpart + '/' in names) or
                        archive.read(entry)[:4] != b'\x00\x05\x16\x07'):
                    raise ValueError(f'Invalid macOS resource fork: {name}')

        build = json.loads(archive.read(app + 'desktop-build.json'))
        if manifest['revision'] != revision or build['revision'] != revision:
            raise ValueError('Package revision differs from the source revision')
        if manifest['schemaVersion'] != 1 or any(
                manifest[key] != build[key]
                for key in ('schemaVersion', 'revision', 'sourceTimestamp', 'releaseRepository')):
            raise ValueError('Desktop build metadata differs from the package manifest')
        native = 'authority/solomon-darker' + ('.exe' if manifest['platform'] == 'win32' else '')
        if manifest['nativeAuthority']['executable'] != native:
            raise ValueError('Unexpected native authority path')
        native_path = app + native
        if (hashes.get(native_path) != manifest['nativeAuthority']['sha256'] or
                stat.S_ISLNK(archive.getinfo(native_path).external_attr >> 16)):
            raise ValueError('Native authority hash differs from its manifest')

        stock = json.loads((source_root / 'web/assets/stock-files.json').read_text())
        prefix = app + 'client/assets/stock/'
        actual_stock = {name.removeprefix(prefix) for name in hashes if name.startswith(prefix)}
        if actual_stock != stock.keys():
            changed = sorted(actual_stock ^ stock.keys())
            raise ValueError('Stock membership differs: ' + ', '.join(changed[:5]))
        for name, digest in stock.items():
            if hashes[prefix + name] != digest:
                raise ValueError(f'Stock hash differs: {name}')

        copied = [(name, name) for name in SHELL_FILES]
        copied += [('client/__desktop/' + name, name) for name in LAUNCHER_FILES]
        for destination, source in copied:
            actual = archive.read(app + destination)
            expected = (source_root / 'desktop' / source).read_bytes()
            if source == 'package.json':
                # Electron Packager drops npm's private flag from the application.
                actual, expected = json.loads(actual), json.loads(expected)
                actual.pop('private', None)
                expected.pop('private', None)
            else:
                actual = actual.replace(b'\r\n', b'\n')
                expected = expected.replace(b'\r\n', b'\n')
            if actual != expected:
                raise ValueError(f'Source-owned shell differs: {destination}')
        allowed_root_files = {*SHELL_FILES, 'desktop-build.json'}
        for name in hashes:
            if name.startswith(app):
                relative = name.removeprefix(app)
                if '/' not in relative and relative not in allowed_root_files:
                    raise ValueError(f'Unexpected application file: {relative}')
    return {'revision': revision, 'target': target, 'entries': len(entries),
            'stockFiles': len(stock), 'sourceFiles': len(copied), 'nativeSha256': hashes[native_path]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archive', type=Path)
    args = parser.parse_args()
    revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    try:
        print(json.dumps(verify_archive(args.archive, ROOT, revision)))
    except (OSError, ValueError, KeyError, TypeError, zipfile.BadZipFile) as error:
        raise SystemExit(f'Desktop archive rejected: {error}') from error


if __name__ == '__main__':
    main()
