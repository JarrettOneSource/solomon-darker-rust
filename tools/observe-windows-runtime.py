"""Bounded diagnostic only: compare driver priority and retain Windows resource data."""
import ctypes as c
from ctypes import wintypes as w
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time


class ProcessEntry(c.Structure):
    _fields_ = [("size", w.DWORD), ("usage", w.DWORD), ("pid", w.DWORD),
                ("heap", c.c_size_t), ("module", w.DWORD), ("threads", w.DWORD),
                ("parent", w.DWORD), ("basePriority", w.LONG), ("flags", w.DWORD),
                ("name", w.WCHAR * 260)]


class Performance(c.Structure):
    _fields_ = [("size", w.DWORD)] + [(name, c.c_size_t) for name in (
        "commit", "commitLimit", "commitPeak", "physical", "available", "cache",
        "kernel", "kernelPaged", "kernelNonpaged", "pageSize")] + [
        (name, w.DWORD) for name in ("handles", "processes", "threads")]


class Memory(c.Structure):
    _fields_ = [("size", w.DWORD), ("pageFaults", w.DWORD)] + [
        (name, c.c_size_t) for name in ("peakWorkingSet", "workingSet", "peakPagedPool",
        "pagedPool", "peakNonpagedPool", "nonpagedPool", "pagefile", "peakPagefile", "private")]


class Io(c.Structure):
    _fields_ = [(name, c.c_ulonglong) for name in (
        "readOperations", "writeOperations", "otherOperations", "readBytes", "writeBytes", "otherBytes")]


k = c.WinDLL("kernel32", use_last_error=True)
p = c.WinDLL("psapi", use_last_error=True)
for library, name, result, args in [
    (k, "CreateToolhelp32Snapshot", w.HANDLE, [w.DWORD, w.DWORD]),
    (k, "Process32FirstW", w.BOOL, [w.HANDLE, c.POINTER(ProcessEntry)]),
    (k, "Process32NextW", w.BOOL, [w.HANDLE, c.POINTER(ProcessEntry)]),
    (k, "OpenProcess", w.HANDLE, [w.DWORD, w.BOOL, w.DWORD]),
    (k, "CloseHandle", w.BOOL, [w.HANDLE]),
    (k, "GetProcessTimes", w.BOOL, [w.HANDLE] + [c.POINTER(w.FILETIME)] * 4),
    (k, "GetSystemTimes", w.BOOL, [c.POINTER(w.FILETIME)] * 3),
    (k, "GetProcessIoCounters", w.BOOL, [w.HANDLE, c.POINTER(Io)]),
    (k, "GetPriorityClass", w.DWORD, [w.HANDLE]),
    (p, "GetProcessMemoryInfo", w.BOOL, [w.HANDLE, c.POINTER(Memory), w.DWORD]),
    (p, "GetPerformanceInfo", w.BOOL, [c.POINTER(Performance), w.DWORD]),
]:
    function = getattr(library, name)
    function.restype, function.argtypes = result, args


def ticks(value):
    return (value.dwHighDateTime << 32) | value.dwLowDateTime


def processes():
    snapshot = k.CreateToolhelp32Snapshot(2, 0)
    if snapshot == c.c_void_p(-1).value:
        raise c.WinError(c.get_last_error())
    rows = []
    try:
        entry = ProcessEntry(size=c.sizeof(ProcessEntry))
        present = k.Process32FirstW(snapshot, c.byref(entry))
        while present:
            rows.append({name: getattr(entry, name) for name in
                         ("pid", "parent", "name", "threads", "basePriority")})
            present = k.Process32NextW(snapshot, c.byref(entry))
    finally:
        k.CloseHandle(snapshot)
    return rows


def process_data(row):
    handle = k.OpenProcess(0x0400 | 0x0010, False, row["pid"])
    if not handle:
        return {**row, "accessError": c.get_last_error()}
    try:
        created, exited, kernel, user = [w.FILETIME() for _ in range(4)]
        if not k.GetProcessTimes(handle, c.byref(created), c.byref(exited), c.byref(kernel), c.byref(user)):
            return {**row, "timesError": c.get_last_error()}
        memory = Memory(size=c.sizeof(Memory))
        memory_ok = p.GetProcessMemoryInfo(handle, c.byref(memory), c.sizeof(memory))
        io = Io()
        io_ok = k.GetProcessIoCounters(handle, c.byref(io))
        return {**row, "created": ticks(created), "cpuSeconds": (ticks(kernel) + ticks(user)) / 1e7,
                "priority": k.GetPriorityClass(handle),
                "memory": {name: getattr(memory, name) for name, _ in Memory._fields_[1:]} if memory_ok else None,
                "io": {name: getattr(io, name) for name, _ in Io._fields_} if io_ok else None}
    finally:
        k.CloseHandle(handle)


def sample(root, known, previous, elapsed):
    performance = Performance(size=c.sizeof(Performance))
    if not p.GetPerformanceInfo(c.byref(performance), c.sizeof(performance)):
        raise c.WinError(c.get_last_error())
    idle, kernel, user = [w.FILETIME() for _ in range(3)]
    if not k.GetSystemTimes(c.byref(idle), c.byref(kernel), c.byref(user)):
        raise c.WinError(c.get_last_error())
    readings = [process_data(row) for row in processes() if row["pid"] > 0]
    owned = {root} | {row["pid"] for row in readings
                      if row.get("created") is not None and known.get(row["pid"]) == row["created"]}
    while True:
        expanded = owned | {row["pid"] for row in readings if row["parent"] in owned}
        if expanded == owned:
            break
        owned = expanded
    for row in readings:
        identity = (row["pid"], row.get("created"))
        old_cpu = previous.get(identity)
        row["cpuPercentOneCore"] = (100 * (row["cpuSeconds"] - old_cpu) / elapsed
                                    if old_cpu is not None else None)
        if "cpuSeconds" in row:
            previous[identity] = row["cpuSeconds"]
        if row["pid"] in owned and row.get("created") is not None:
            known[row["pid"]] = row["created"]
    own_rows = [row for row in readings if row["pid"] in owned]
    other_rows = sorted((row for row in readings if row["pid"] not in owned),
                        key=lambda row: row.get("cpuPercentOneCore") or 0, reverse=True)[:8]
    return {"at": round(time.time() * 1000), "intervalSeconds": elapsed, "cpuCount": os.cpu_count(),
            "systemCpuSeconds": {"idle": ticks(idle) / 1e7, "kernelIncludingIdle": ticks(kernel) / 1e7,
                                 "user": ticks(user) / 1e7},
            "system": {name: getattr(performance, name) for name, _ in Performance._fields_[1:]},
            "owned": own_rows, "topOther": other_rows}


def run_attempt(attempt, priority):
    output = Path(f"reports/windows-input/attempt-{attempt}")
    output.mkdir(parents=True)
    bundle = Path("node_modules/playwright-core/lib/coreBundle.js").read_bytes()
    assert b'this._client.send("Emulation.setFocusEmulationEnabled", { enabled: true })' in bundle
    flags = subprocess.NORMAL_PRIORITY_CLASS if priority == "normal" else subprocess.ABOVE_NORMAL_PRIORITY_CLASS
    record = {"attempt": attempt, "driverPriority": priority, "creationFlags": flags,
              "focusEmulation": "unmodified", "playwrightSha256": hashlib.sha256(bundle).hexdigest()}
    known, previous = {}, {}
    with (output / "driver.log").open("w") as log, (output / "system.jsonl").open("w", buffering=1) as journal:
        child = subprocess.Popen(["node", "tools/smoke-desktop-runtime.mjs"], creationflags=flags,
                                 env=dict(os.environ, SDR_DESKTOP_EVIDENCE=str(output.resolve())),
                                 stdout=log, stderr=subprocess.STDOUT)
        record["driverPid"] = child.pid
        (output / "automation.json").write_text(json.dumps(record, indent=2) + "\n")
        start = last = time.monotonic()
        try:
            while child.poll() is None:
                now = time.monotonic()
                journal.write(json.dumps(sample(child.pid, known, previous, max(now - last, 0.001))) + "\n")
                last = now
                if now - start >= 600:
                    subprocess.run(["taskkill", "/PID", str(child.pid), "/T", "/F"], stdout=log, stderr=subprocess.STDOUT)
                    child.wait(timeout=30)
                    record["outerTimeout"] = True
                    break
                time.sleep(1)
        finally:
            if child.poll() is None:
                subprocess.run(["taskkill", "/PID", str(child.pid), "/T", "/F"], stdout=log, stderr=subprocess.STDOUT)
                child.wait(timeout=30)
            survivors = [process_data(row) for row in processes() if row["pid"] in known]
            survivors = [row for row in survivors if row.get("created") == known[row["pid"]]]
            record["survivingOwnedProcesses"] = survivors
            for row in survivors:
                subprocess.run(["taskkill", "/PID", str(row["pid"]), "/T", "/F"], stdout=log, stderr=subprocess.STDOUT)
    record["exitCode"] = 124 if record.get("outerTimeout") else child.returncode
    record["elapsedSeconds"] = time.monotonic() - start
    (output / "automation.json").write_text(json.dumps(record, indent=2) + "\n")
    return record


if __name__ == "__main__":
    assert c.sizeof(ProcessEntry) == 568 and c.sizeof(Memory) == 80 and c.sizeof(Performance) == 104
    self_check = sample(os.getpid(), {}, {}, 1)
    assert self_check["system"]["physical"] > 0
    assert any(row["pid"] == os.getpid() and row.get("cpuSeconds") is not None for row in self_check["owned"])
    results = []
    for attempt, priority in [(1, "normal"), (2, "above-normal"), (3, "above-normal")]:
        results.append(run_attempt(attempt, priority))
        Path("reports/windows-input/results.json").write_text(json.dumps(results, indent=2) + "\n")
    print(json.dumps([{key: value for key, value in result.items() if key != "survivingOwnedProcesses"}
                      for result in results]))
