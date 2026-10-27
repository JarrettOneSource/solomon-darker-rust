"""Own the real Windows host and deliver its ordinary console shutdown event."""
from pathlib import Path
import ctypes,json,os,signal,subprocess,time,urllib.request
root=Path.cwd();output=root/'reports/browser-admission';output.mkdir(parents=True,exist_ok=True)
receipt={'pass':False,'hostExit':None,'consoleAllocated':False}
# A detached runner can lack a console. Allocate one before creating the host's group.
kernel=ctypes.WinDLL('kernel32',use_last_error=True)
processes=(ctypes.c_ulong*1)()
if kernel.GetConsoleProcessList(processes,1)==0:
    if not kernel.AllocConsole(): raise ctypes.WinError(ctypes.get_last_error())
    receipt['consoleAllocated']=True
with (output/'host.log').open('w') as log:
    host=subprocess.Popen([str(root/'target/debug/solomon-darker.exe'),'--no-open','--port','4190','--save',str(output/'host-save.json'),'--assets',str(root/'web/dist')],stdout=log,stderr=subprocess.STDOUT,creationflags=subprocess.CREATE_NEW_PROCESS_GROUP)
    try:
        deadline=time.monotonic()+30
        while True:
            if host.poll() is not None: raise RuntimeError(f'Host exited before readiness: {host.returncode}')
            try:
                with urllib.request.urlopen('http://127.0.0.1:4190/',timeout=1) as response:
                    assert response.status==200
                break
            except OSError:
                if time.monotonic()>=deadline: raise
                time.sleep(.1)
        env=os.environ.copy();env.update(PW=str(root/'node_modules/playwright-core'),CHROME=str(Path(os.environ['ProgramFiles'])/'Google/Chrome/Application/chrome.exe'),URL='http://127.0.0.1:4190/?host=1',OUT=str(output/'browser'))
        result=subprocess.run(['node','tools/browser-smoke/native-admission-acceptance.cjs'],env=env)
        receipt['browserExit']=result.returncode
        assert result.returncode==0,'Actual browser admission failed'
    finally:
        try:
            if host.poll() is None: host.send_signal(signal.CTRL_BREAK_EVENT)
            receipt['hostExit']=host.wait(timeout=30)
            assert receipt['hostExit']==0,'Native host did not save and exit normally'
            receipt['pass']=receipt.get('browserExit')==0
        finally:
            if host.poll() is None:
                receipt['forcedCleanup']=True
                host.kill();host.wait(timeout=10)
            (output/'host-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
            if receipt['consoleAllocated']: kernel.FreeConsole()
