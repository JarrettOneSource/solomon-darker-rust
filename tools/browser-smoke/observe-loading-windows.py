"""Own the canonical asset server and sample Windows process/resource counters."""
from pathlib import Path
import ctypes,json,os,subprocess,sys,threading,time,urllib.request

root=Path.cwd();output=root/'reports/browser-loading';output.mkdir(parents=True,exist_ok=True)
stop=threading.Event()
class Memory(ctypes.Structure):
    _fields_=[('length',ctypes.c_ulong),('load',ctypes.c_ulong)]+[(name,ctypes.c_ulonglong) for name in ['total','available','pageTotal','pageAvailable','virtualTotal','virtualAvailable','extended']]
def sample():
    with (output/'processes.jsonl').open('w') as log:
        while not stop.is_set():
            memory=Memory();memory.length=ctypes.sizeof(memory)
            ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(memory))
            row={'time':time.time(),'memoryLoad':memory.load,'totalMemory':memory.total,'availableMemory':memory.available}
            try:
                result=subprocess.run(['powershell','-NoProfile','-Command',"Get-Process chrome,python,node -ErrorAction SilentlyContinue | Select-Object Id,ProcessName,CPU,WorkingSet64,PrivateMemorySize64,Handles | ConvertTo-Json -Compress"],capture_output=True,text=True,timeout=15)
                row['processes']=json.loads(result.stdout) if result.stdout.strip() else []
            except Exception as error:row['error']=str(error)
            log.write(json.dumps(row)+'\n');log.flush();stop.wait(15)
sampler=threading.Thread(target=sample,daemon=True);sampler.start()
receipt={'pass':False}
with (output/'server.log').open('w') as log:
    server=subprocess.Popen([sys.executable,'scripts/serve-client.py','4181','--bind','127.0.0.1','--directory','web/dist'],stdout=log,stderr=subprocess.STDOUT)
    try:
        deadline=time.monotonic()+30
        while True:
            try:
                with urllib.request.urlopen('http://127.0.0.1:4181/',timeout=1) as response:assert response.status==200
                break
            except OSError:
                if time.monotonic()>=deadline:raise
                time.sleep(.1)
        env=os.environ.copy();env.update(PW=str(root/'node_modules/playwright-core'),CHROME=str(Path(os.environ['ProgramFiles'])/'Google/Chrome/Application/chrome.exe'),URL='http://127.0.0.1:4181/',OUT=str(output/'settings'))
        result=subprocess.run(['node','tools/browser-smoke/observe-loading.cjs'],env=env)
        receipt['browserDriverExit']=result.returncode
        receipt['pass']=result.returncode==0
    finally:
        receipt['serverExitedBeforeCleanup']=server.poll() is not None
        if server.poll() is None:server.terminate()
        receipt['serverExit']=server.wait(timeout=15)
        stop.set();sampler.join(timeout=20)
        (output/'driver-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
sys.exit(0 if receipt['pass'] else 1)
