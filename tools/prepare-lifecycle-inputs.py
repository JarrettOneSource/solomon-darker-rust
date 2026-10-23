"""Verify unchanged build inputs, then recover exact retained desktop components."""

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import tarfile
import zipfile


BASE = 'af80793bf21331547c08071d9ff3e4b18468ee85'
ARM_ARCHIVE = 'd2b339d3dc869ce4a9e6ed9bbae132a987917c5dc2bc8db1558bdc86afc06c92'
NATIVE = {
    'darwin-arm64': '4cabb8c7a4585c216ee04a5b62a9cf534914ca7be5b54c927e55b7b50f6cd522',
    'linux-x64': 'bf768fcd7d3c2688b1d281db5a303ee1e4f555527fcd11105fb3f8390f5d54dc',
    'win32-x64': '8bb7db55e53eed01580da51ee1f3a16b13a83f9891489563ce446af1317c2638',
}
BUILD_INPUTS = [
    'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo', 'crates', 'web',
    'scripts/prepare-desktop-client.py', 'scripts/trunk_bootstrap.py', 'tools/package-desktop.mjs',
    'tools/desktop-build.mjs', 'tools/stage-node-runtime.mjs',
    'package.json', 'package-lock.json', '.node-version', 'desktop/package.json',
]


def digest(path):
    with Path(path).open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def git(*args):
    return subprocess.check_output(['git', *args])


def checked_archive(path, expected):
    actual = digest(path)
    if actual != expected:
        raise ValueError(f'Archive checksum mismatch: {path}')
    return actual


def safe_relative(value):
    path = PurePosixPath(value)
    if not value or path.is_absolute() or '..' in path.parts or '\\' in value:
        raise ValueError(f'Invalid retained member: {value}')
    return path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--target', required=True,
                        choices=['darwin-arm64', 'darwin-x64', 'linux-x64', 'win32-x64'])
    parser.add_argument('--intel-sha256')
    args = parser.parse_args()
    revision = git('rev-parse', 'HEAD').decode().strip()
    if git('status', '--porcelain', '--untracked-files=no'):
        raise ValueError('Product checkout is not clean')
    before = git('ls-tree', '-r', BASE, '--', *BUILD_INPUTS)
    after = git('ls-tree', '-r', revision, '--', *BUILD_INPUTS)
    if before != after:
        raise ValueError('Native, client, dependency or packaging build inputs changed')

    destination = Path('retained-inputs')
    client = destination / 'client'
    native = destination / ('solomon-darker.exe' if args.target == 'win32-x64' else 'solomon-darker')
    destination.mkdir()
    arm_path = Path('arm-input/Solomon-Darker-darwin-arm64.zip')
    archives = {'darwin-arm64': checked_archive(arm_path, ARM_ARCHIVE)}
    app_prefix = 'Solomon Darker-darwin-arm64/Solomon Darker.app/Contents/Resources/app/'
    files = {}
    with zipfile.ZipFile(arm_path) as archive:
        for member in archive.infolist():
            prefix = app_prefix + 'client/'
            if member.is_dir() or not member.filename.startswith(prefix):
                continue
            relative = str(safe_relative(member.filename[len(prefix):]))
            if relative in files:
                raise ValueError(f'Duplicate retained client member: {relative}')
            data = archive.read(member)
            path = client / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
            files[relative] = hashlib.sha256(data).hexdigest()
        if args.target == 'darwin-arm64':
            native.write_bytes(archive.read(app_prefix + 'authority/solomon-darker'))
    if len(files) != 5723 or 'index.html' not in files:
        raise ValueError('Retained client membership differs from its accepted inventory')

    if args.target == 'win32-x64':
        path = Path('native-input/Solomon-Darker-win32-x64.zip')
        archives[args.target] = checked_archive(path, '9e32ec7e2ad279c2b178e9ab5bc4c98b0bd935befb253dc939093fb2b2d568d3')
        with zipfile.ZipFile(path) as archive:
            native.write_bytes(archive.read('Solomon Darker-win32-x64/resources/app/authority/solomon-darker.exe'))
    elif args.target == 'linux-x64':
        path = Path('native-input/solomon-game-linux.tar.gz')
        archives[args.target] = checked_archive(path, '0c78d00b026b9a9c659150af6b8bfcbbd8a6c31589ee4f50c775018e7485bd66')
        with tarfile.open(path, 'r:gz') as archive:
            members = [row for row in archive.getmembers()
                       if row.isfile() and PurePosixPath(row.name).name == 'solomon-darker']
            if len(members) != 1:
                raise ValueError('Expected exactly one hosted native executable')
            native.write_bytes(archive.extractfile(members[0]).read())
    elif args.target == 'darwin-x64':
        native.write_bytes(Path('native-input/solomon-darker').read_bytes())
        if not args.intel_sha256 or len(args.intel_sha256) != 64:
            raise ValueError('The separately retained Intel native checksum is required')
        NATIVE[args.target] = args.intel_sha256

    native_hash = checked_archive(native, NATIVE[args.target])
    native_origin = {
        'kind': 'retained-ci-artifact', 'runId': 39864248104,
        'artifactId': {'darwin-arm64': 12465301379, 'linux-x64': 12464067859,
                       'win32-x64': 12465384038}.get(args.target),
    }
    if args.target == 'darwin-x64':
        native_origin = {
            'kind': 'M2-native-only-build',
            'releaseAssetId': int(os.environ['SDR_INTEL_ASSET_ID']),
            'buildReceipt': 'af-intel-native-20261022/build.json',
            'buildReceiptSha256': '48e9a0c9a5227d16e691001ddbb63667f7d3d0b6ad3cfadb96d517754aa87767',
            'rustToolchain': '1.98.1', 'target': 'x86_64-apple-darwin',
            'minimumMacOS': '12.0', 'sdk': '27.0',
        }
    if os.name != 'nt':
        native.chmod(0o755)
    reports = Path('reports/desktop-smoke')
    reports.mkdir(parents=True, exist_ok=True)
    receipt = {
        'productRevision': revision, 'nativeSourceRevision': BASE,
        'clientSourceRevision': BASE, 'clientRunId': 39864248104,
        'nativeOrigin': native_origin,
        'target': args.target, 'buildInputsUnchanged': True,
        'buildInputsGitTreeSha256': hashlib.sha256(after).hexdigest(),
        'buildInputPaths': BUILD_INPUTS, 'sourceArchives': archives,
        'nativeInputSha256': native_hash, 'clientFiles': len(files),
        'clientFileSha256': files,
        'qualification': 'Inputs verified; the new package still requires archive and runtime acceptance.',
    }
    (reports / 'retained-inputs.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps({key: value for key, value in receipt.items() if key != 'clientFileSha256'}))


if __name__ == '__main__':
    main()
