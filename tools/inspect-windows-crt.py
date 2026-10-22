"""Use the installed Microsoft linker tools to retain actual PE import evidence."""
import hashlib
import json
import os
import pathlib
import re
import subprocess
import sys

path = pathlib.Path(sys.argv[1]).resolve()
output = pathlib.Path(sys.argv[2])
expected = sys.argv[3]
assert expected in ['dynamic', 'static']
vswhere = pathlib.Path(os.environ['ProgramFiles(x86)']) / 'Microsoft Visual Studio/Installer/vswhere.exe'
installation = pathlib.Path(subprocess.check_output([str(vswhere), '-latest', '-products', '*', '-requires',
    'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', '-property', 'installationPath'], text=True).strip())
tools = sorted((installation / 'VC/Tools/MSVC').glob('*/bin/Hostx64/x64/dumpbin.exe'))
assert tools, 'The normal MSVC build must supply its PE inspection tool'
dumpbin = tools[-1]
result = subprocess.run([str(dumpbin), '/DEPENDENTS', str(path)], capture_output=True, text=True, check=True, timeout=30)
imports = sorted(set(re.findall(r'^\s+([\w.-]+\.dll)\s*$', result.stdout, re.IGNORECASE | re.MULTILINE)), key=str.lower)
assert imports, 'PE inspection must identify actual import DLLs'
external_crt = [name for name in imports if name.lower().startswith(('vcruntime', 'msvcp', 'msvcr', 'concrt'))]
with path.open('rb') as stream:
    digest = hashlib.file_digest(stream, 'sha256').hexdigest()
receipt = {'executable': str(path), 'sha256': digest, 'inspector': str(dumpbin), 'imports': imports,
           'externalVisualCppRuntimeImports': external_crt, 'expectedLinkage': expected}
output.parent.mkdir(parents=True, exist_ok=True)
output.write_text(json.dumps(receipt, indent=2) + '\n')
output.with_suffix('.txt').write_text(result.stdout)
print(json.dumps(receipt))
if expected == 'dynamic':
    assert any(name.lower() == 'vcruntime140.dll' for name in external_crt), 'The retained old binary must reproduce the observed runtime dependency'
else:
    assert not external_crt, 'The newly rebuilt static host must not require external Visual C++ runtime DLLs'
