#!/usr/bin/env python3
"""Inspect the final native desktop ZIP before release publication."""

import argparse
import hashlib
import json
from pathlib import Path
import plistlib
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


def source_hashes(directory):
    result = {}
    for path in sorted(directory.rglob('*')):
        if path.is_symlink():
            raise ValueError(f'Source asset cannot be a symlink: {path}')
        if path.is_file():
            with path.open('rb') as stream:
                result[path.relative_to(directory).as_posix()] = hashlib.file_digest(stream, 'sha256').hexdigest()
    if not result:
        raise ValueError(f'No source files in {directory}')
    return result


def verify_archive(archive_path, source_root, revision):
    with zipfile.ZipFile(archive_path) as archive:
        entries = archive.infolist()
        names = {entry.filename for entry in entries}
        if len(names) != len(entries):
            raise ValueError('Duplicate archive entries')
        for entry in entries:
            name = entry.filename
            parts = name.rstrip('/').split('/')
            if any(part in ('', '.', '..') or ':' in part for part in parts) or '\\' in name:
                raise ValueError(f'Unsafe archive path: {name}')
            if any(part.lower() in BUILD_RESIDUE for part in parts) or name.lower().endswith(
                    ('.sdmod', '.log', '.pdb', '.pyc')):
                raise ValueError(f'Build residue: {name}')
            if stat.S_ISLNK(entry.external_attr >> 16):
                raise ValueError(f'Native package contains a symlink: {name}')

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
        executable = ('Solomon Darker.app/Contents/MacOS/solomon-darker' if macos
                      else 'solomon-darker.exe' if manifest['platform'] == 'win32' else 'solomon-darker')
        resources = 'Solomon Darker.app/Contents/Resources' if macos else '.'
        prefix = resources + '/' if macos else ''
        if (manifest['schemaVersion'] != 2 or manifest['runtime'] != 'native'
                or manifest['executable'] != executable or manifest['resources'] != resources):
            raise ValueError('Expected the native desktop package layout')

        hashes = {}
        for entry in entries:
            name = entry.filename
            if not name.startswith(root) and not (macos and name.startswith('__MACOSX/')):
                raise ValueError(f'Unexpected archive root: {name}')
            # Consume every member, including signed metadata and resource forks,
            # so the final archive's ZIP CRC is part of its acceptance.
            with archive.open(entry) as stream:
                digest = hashlib.file_digest(stream, 'sha256').hexdigest()
            if entry.is_dir():
                continue
            if name.startswith('__MACOSX/'):
                parent, leaf = posixpath.split(name.removeprefix('__MACOSX/'))
                counterpart = posixpath.join(parent, leaf.removeprefix('._'))
                if (not leaf.startswith('._') or
                        not (counterpart in names or counterpart + '/' in names) or
                        archive.read(entry)[:4] != b'\x00\x05\x16\x07'):
                    raise ValueError(f'Invalid macOS resource fork: {name}')
                continue
            if name != manifests[0]:
                hashes[name.removeprefix(root)] = digest

        build = json.loads(archive.read(root + prefix + 'desktop-build.json'))
        if manifest['revision'] != revision or build['revision'] != revision:
            raise ValueError('Package revision differs from the source revision')
        if build['schemaVersion'] != 1 or any(
                manifest[key] != build[key]
                for key in ('revision', 'sourceTimestamp', 'releaseRepository')):
            raise ValueError('Desktop build metadata differs from the package manifest')
        if hashes.keys() != manifest['files'].keys():
            raise ValueError('Package membership differs from its manifest')
        for name, digest in hashes.items():
            if manifest['files'][name] != digest:
                raise ValueError(f'Package hash differs: {name}')
        if not macos and manifest['platform'] != 'win32' and not (
                archive.getinfo(root + executable).external_attr >> 16 & 0o111):
            raise ValueError('The native application is not executable')

        assets = source_hashes(source_root / 'web/assets')
        asset_prefix = prefix + 'game/assets/'
        actual_assets = {name.removeprefix(asset_prefix): digest for name, digest in hashes.items()
                         if name.startswith(asset_prefix)}
        if actual_assets.keys() != assets.keys():
            raise ValueError('Source asset membership differs')
        for name, digest in assets.items():
            if actual_assets[name] != digest:
                raise ValueError(f'Source asset differs: {name}')
        stock = json.loads((source_root / 'web/assets/stock-files.json').read_text())
        if {name.removeprefix('stock/'): digest for name, digest in actual_assets.items()
                if name.startswith('stock/')} != stock:
            raise ValueError('Stock files differ from their reviewed manifest')

        licenses = source_hashes(source_root / 'docs/third-party')
        license_prefix = prefix + 'licenses/'
        for name, digest in licenses.items():
            if hashes.get(license_prefix + name) != digest:
                raise ValueError(f'License differs: {name}')
        allowed = {executable, prefix + 'desktop-build.json',
                   *(asset_prefix + name for name in assets),
                   *(license_prefix + name for name in licenses)}
        if macos:
            info = 'Solomon Darker.app/Contents/Info.plist'
            signature = 'Solomon Darker.app/Contents/_CodeSignature/CodeResources'
            allowed.update((info, signature))
            plist = plistlib.loads(archive.read(root + info))
            if (plist['CFBundleExecutable'] != 'solomon-darker'
                    or plist['CFBundleIdentifier'] != 'com.jarrett.solomondarker'
                    or plist['CFBundlePackageType'] != 'APPL'):
                raise ValueError('Unexpected native application bundle metadata')
        extra = hashes.keys() - allowed
        if extra:
            raise ValueError('Unexpected native package file: ' + ', '.join(sorted(extra)[:5]))
        if allowed != hashes.keys():
            raise ValueError('The native package is incomplete')
    return {'revision': revision, 'target': target, 'runtime': 'native', 'entries': len(entries),
            'stockFiles': len(stock), 'assetFiles': len(assets), 'nativeSha256': hashes[executable]}


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
