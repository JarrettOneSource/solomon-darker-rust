"""Inventory the new native build and prove the verified client bytes were reused."""
import hashlib
import json
import os
import pathlib
import shutil
import subprocess

source = '5c094aaa962b9ef0b231651f08a91e4330ff4317'
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip() == source
old = json.loads(pathlib.Path('reports/windows-capture-control/verified-pair.json').read_text())
configuration = json.loads(pathlib.Path('reports/windows-static-crt/build-configuration.json').read_text())
root = pathlib.Path('dist-desktop/Solomon Darker-win32-x64')
manifest = json.loads((root / 'desktop-package-manifest.json').read_text())
assert manifest['revision'] == source
files = []
for path in sorted(root.rglob('*')):
    if path.is_file():
        with path.open('rb') as stream:
            digest = hashlib.file_digest(stream, 'sha256').hexdigest()
        files.append({'path': path.relative_to(root).as_posix(), 'bytes': path.stat().st_size, 'sha256': digest})
prior = {row['path']: row for row in old['files']}
current = {row['path']: row for row in files}
old_client = {path: row for path, row in prior.items() if path.startswith('resources/app/client/')}
new_client = {path: row for path, row in current.items() if path.startswith('resources/app/client/')}
assert new_client == old_client, 'Every packaged client byte must match the accepted retained Windows client'
native = 'resources/app/' + manifest['nativeAuthority']['executable']
assert current[native]['sha256'] == manifest['nativeAuthority']['sha256']
assert current[native]['sha256'] != old['nativeSha256'], 'The actual static native host must be newly built'
imports = json.loads(pathlib.Path('reports/windows-static-crt/new-native-imports.json').read_text())
assert imports['sha256'] == current[native]['sha256'] and imports['externalVisualCppRuntimeImports'] == []
comparison = {'reusedClientFiles': len(new_client), 'clientBytesIdentical': True,
              'addedPaths': sorted(current.keys() - prior.keys()), 'removedPaths': sorted(prior.keys() - current.keys()),
              'changedPaths': sorted(path for path in current.keys() & prior.keys() if current[path] != prior[path]),
              'priorArchiveSha256': old['archiveSha256'], 'priorNativeSha256': old['nativeSha256']}
configuration['pairComparison'] = comparison
receipt = pathlib.Path('reports/windows-static-crt-pair')
receipt.mkdir(parents=True, exist_ok=True)
(receipt / 'pair.json').write_text(json.dumps({'status': 'unqualified-capture-control-input', 'sourceRevision': source,
    'workflowRevision': os.environ['GITHUB_SHA'], 'runId': os.environ['GITHUB_RUN_ID'], 'runAttempt': os.environ['GITHUB_RUN_ATTEMPT'],
    'manifest': manifest, 'buildConfiguration': configuration, 'files': files}, indent=2) + '\n')
shutil.copy2(root / 'desktop-package-manifest.json', receipt)
(receipt / 'build-configuration.json').write_text(json.dumps(configuration, indent=2) + '\n')
print(json.dumps({'nativeSha256': current[native]['sha256'], 'fileCount': len(files), **comparison}))
