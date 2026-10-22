"""Verify/extract the one retained normal Windows package for capture controls."""
import hashlib
import json
import os
import pathlib
import zipfile

source = '5c094aaa962b9ef0b231651f08a91e4330ff4317'
inputs = pathlib.Path('capture-input')
pairs = list(inputs.rglob('pair.json'))
assert len(pairs) == 1, pairs
pair = json.loads(pairs[0].read_text())
assert pair['status'] == 'unqualified-capture-control-input'
assert pair['sourceRevision'] == source
assert pair['runId'] == os.environ['SDR_CAPTURE_PAIR_RUN_ID']
archives = list(inputs.rglob('Solomon-Darker-win32-x64.zip'))
assert len(archives) == 1, archives
archive = archives[0]
with archive.open('rb') as stream:
    archive_hash = hashlib.file_digest(stream, 'sha256').hexdigest()
assert archive_hash == pathlib.Path(str(archive) + '.sha256').read_text().split()[0]
destination = pathlib.Path('dist-desktop').resolve()
with zipfile.ZipFile(archive) as package:
    for entry in package.infolist():
        assert (destination / entry.filename).resolve().is_relative_to(destination), entry.filename
    package.extractall(destination)
root = destination / 'Solomon Darker-win32-x64'
assert {path.relative_to(root).as_posix() for path in root.rglob('*') if path.is_file()} == {row['path'] for row in pair['files']}
for row in pair['files']:
    path = root / row['path']
    assert path.stat().st_size == row['bytes'], row['path']
    with path.open('rb') as stream:
        assert hashlib.file_digest(stream, 'sha256').hexdigest() == row['sha256'], row['path']
manifest = json.loads((root / 'desktop-package-manifest.json').read_text())
assert manifest == pair['manifest']
assert manifest['revision'] == source
assert not (root / 'resources/app/client/.trunk-lock').exists()
receipt = {'status': 'verified-unqualified-control-input', 'sourceRevision': source,
           'pairRunId': pair['runId'], 'pairWorkflowRevision': pair['workflowRevision'],
           'driverRevision': os.environ['GITHUB_SHA'], 'archiveSha256': archive_hash,
           'nativeSha256': manifest['nativeAuthority']['sha256'], 'fileCount': len(pair['files']),
           'files': pair['files']}
reports = pathlib.Path('reports/windows-capture-control')
reports.mkdir(parents=True, exist_ok=True)
(reports / 'verified-pair.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps({key: value for key, value in receipt.items() if key != 'files'}))
