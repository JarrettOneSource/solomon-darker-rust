from pathlib import Path
import hashlib,json,os,subprocess,time
root=Path.cwd()
output=root/'reports/desktop-disconnect'
output.mkdir(parents=True,exist_ok=True)
source=root/'crates/launcher/src/transport/socket.rs'
original=source.read_bytes()
text=source.read_text()
needle='        let close = drive_peer(socket, &worker_server, &peer);\n        finish_peer(&worker_server, &peer);\n'
replacement=needle+'        eprintln!("DISCONNECT_DIAGNOSTIC closed socket {id:?}; delaying authority notification");\n        thread::sleep(std::time::Duration::from_millis(750));\n'
assert text.count(needle)==1
source.write_text(text.replace(needle,replacement))
(output/'private-delay.patch').write_bytes(subprocess.check_output(['git','diff','--',str(source.relative_to(root))]))
patch=root/'tools/desktop-disconnect-test.patch'
(output/'proposed-test.patch').write_bytes(patch.read_bytes())
base=['cargo','test','-p','solomon-launcher','--test','hosted_supervisor','--all-features']
focused=base+['desktop_tests::desktop_authority_retains_source_bootstrap_console_save_and_owner_lifetimes','--','--exact','--nocapture']
rows=[]
def run(label,command):
    started=time.monotonic()
    log=output/(label+'.log')
    with log.open('w') as handle:
        result=subprocess.run(command,cwd=root,stdout=handle,stderr=subprocess.STDOUT)
    row={'label':label,'command':command,'exit':result.returncode,'elapsedSeconds':round(time.monotonic()-started,3),'log':log.name}
    rows.append(row)
    print(json.dumps(row),flush=True)
    (output/'progress.json').write_text(json.dumps(rows,indent=2)+'\n')
    return result.returncode,log.read_text(errors='replace')
try:
    code,log=run('red-delayed-disconnect',focused)
    assert code!=0 and 'assertion `left == right` failed: last-human-resets-owner' in log and 'DISCONNECT_DIAGNOSTIC' in log,'Expected missing-membership-wait failure was not reproduced.'
    subprocess.run(['git','apply','--ignore-space-change',str(patch)],check=True)
    code,log=run('green-delayed-disconnect',focused)
    assert code==0 and 'DISCONNECT_DIAGNOSTIC' in log,'The test correction did not pass the same delayed-disconnect scenario.'
finally:
    source.write_bytes(original)
    assert source.read_bytes()==original
subprocess.run(['cargo','fmt','--all'],check=True)
paths=['crates/launcher/tests/hosted_supervisor/desktop.rs','crates/launcher/tests/hosted_supervisor/runtime_events.rs']
changed=subprocess.check_output(['git','diff','--name-only'],text=True).splitlines()
assert sorted(changed)==sorted(paths),changed
(output/'verified-source.patch').write_bytes(subprocess.check_output(['git','diff','--',*paths]))
for path in paths:
    (output/Path(path).name).write_bytes((root/path).read_bytes())
for label,command in [
    ('format',['cargo','fmt','--all','--','--check']),
    ('clippy',['cargo','clippy','-p','solomon-launcher','--all-targets','--all-features','--','-D','warnings']),
    ('hosted-suite',base+['--','--nocapture']),
]:
    code,_=run(label,command)
    assert code==0,label
receipt={'baseRevision':'9d8c4dccae5d7b859c5185a98296118f0accd233','diagnosticRevision':os.environ['GITHUB_SHA'],'controlledCondition':'Delay the socket worker 750ms after close and before sending Disconnected to the authority; no changes to authority behavior or test fixture.','redExpectedFailure':True,'greenSameConditionPassed':True,'productionSourceRestored':True,'restoredTransportSha256':hashlib.sha256(original).hexdigest(),'verifiedPatchSha256':hashlib.sha256((output/'verified-source.patch').read_bytes()).hexdigest(),'rows':rows}
(output/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps(receipt),flush=True)
