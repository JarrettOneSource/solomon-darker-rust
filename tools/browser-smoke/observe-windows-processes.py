"""Private counters through native Windows APIs, without sampling subprocesses."""
import ctypes
from ctypes import wintypes
import json
import os
import threading
import time

kernel = ctypes.WinDLL('kernel32', use_last_error=True)

class ProcessEntry(ctypes.Structure):
    _fields_ = [('dwSize', wintypes.DWORD), ('cntUsage', wintypes.DWORD),
                ('th32ProcessID', wintypes.DWORD), ('th32DefaultHeapID', ctypes.c_size_t),
                ('th32ModuleID', wintypes.DWORD), ('cntThreads', wintypes.DWORD),
                ('th32ParentProcessID', wintypes.DWORD), ('pcPriClassBase', wintypes.LONG),
                ('dwFlags', wintypes.DWORD), ('szExeFile', wintypes.WCHAR * 260)]

class Memory(ctypes.Structure):
    _fields_ = [('length', wintypes.DWORD), ('load', wintypes.DWORD)] + [
        (name, ctypes.c_ulonglong) for name in
        ['total', 'available', 'pageTotal', 'pageAvailable', 'virtualTotal', 'virtualAvailable', 'extended']]

kernel.CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
kernel.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
for name in ('Process32FirstW', 'Process32NextW'):
    getattr(kernel, name).argtypes = [wintypes.HANDLE, ctypes.POINTER(ProcessEntry)]
    getattr(kernel, name).restype = wintypes.BOOL
kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
kernel.OpenProcess.restype = wintypes.HANDLE
kernel.CloseHandle.argtypes = [wintypes.HANDLE]
kernel.GetProcessTimes.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4
kernel.GetPriorityClass.argtypes = [wintypes.HANDLE]
kernel.GetPriorityClass.restype = wintypes.DWORD
kernel.GetSystemTimes.argtypes = [ctypes.POINTER(wintypes.FILETIME)] * 3
kernel.GlobalMemoryStatusEx.argtypes = [ctypes.POINTER(Memory)]

def seconds(value):
    return ((value.dwHighDateTime << 32) | value.dwLowDateTime) / 10000000

def counters():
    started = time.monotonic()
    memory = Memory(); memory.length = ctypes.sizeof(memory)
    if not kernel.GlobalMemoryStatusEx(ctypes.byref(memory)):
        raise ctypes.WinError(ctypes.get_last_error())
    idle, system, user = (wintypes.FILETIME() for _ in range(3))
    if not kernel.GetSystemTimes(ctypes.byref(idle), ctypes.byref(system), ctypes.byref(user)):
        raise ctypes.WinError(ctypes.get_last_error())
    rows = []
    snapshot = kernel.CreateToolhelp32Snapshot(2, 0)
    if snapshot == ctypes.c_void_p(-1).value:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        entry = ProcessEntry(); entry.dwSize = ctypes.sizeof(entry)
        valid = kernel.Process32FirstW(snapshot, ctypes.byref(entry))
        while valid:
            handle = kernel.OpenProcess(0x1000, False, entry.th32ProcessID)
            if handle:
                try:
                    created, exited, system_time, user_time = (wintypes.FILETIME() for _ in range(4))
                    if kernel.GetProcessTimes(handle, ctypes.byref(created), ctypes.byref(exited),
                                              ctypes.byref(system_time), ctypes.byref(user_time)):
                        rows.append({'pid': entry.th32ProcessID, 'name': entry.szExeFile,
                                     'parent': entry.th32ParentProcessID, 'threads': entry.cntThreads,
                                     'basePriority': entry.pcPriClassBase,
                                     'priorityClass': kernel.GetPriorityClass(handle),
                                     'created': seconds(created),
                                     'cpu': seconds(system_time) + seconds(user_time)})
                finally:
                    kernel.CloseHandle(handle)
            valid = kernel.Process32NextW(snapshot, ctypes.byref(entry))
    finally:
        kernel.CloseHandle(snapshot)
    return {'time': time.time(), 'duration': time.monotonic() - started,
            'cpuCount': os.cpu_count(), 'memoryLoad': memory.load,
            'availableMemory': memory.available, 'totalMemory': memory.total,
            'idle': seconds(idle), 'system': seconds(system), 'user': seconds(user), 'processes': rows}

def start(output, stop):
    def sample():
        with output.open('w') as log:
            while not stop.is_set():
                try:
                    row = counters()
                except Exception as error:
                    row = {'time': time.time(), 'error': str(error)}
                log.write(json.dumps(row, separators=(',', ':')) + '\n'); log.flush()
                stop.wait(5)
    thread = threading.Thread(target=sample, daemon=True)
    thread.start()
    return thread
