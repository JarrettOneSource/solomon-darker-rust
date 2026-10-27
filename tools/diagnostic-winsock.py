"""Private, bounded Windows socket comparison against an owned loopback server."""

from concurrent.futures import ThreadPoolExecutor
import ctypes
import hashlib
import json
from pathlib import Path
import platform
import select
import socket
import subprocess
import sys
import threading
import time
import traceback

assert sys.platform == "win32"
OUT = Path("reports/winsock").resolve()
OUT.mkdir(parents=True, exist_ok=True)
ADDRESS = ("127.0.0.1", 4181)
# Microsoft ws2def.h; Chromium's TCPSocketWin::DoConnect sets this before connect.
SO_RANDOMIZE_PORT = 0x3005
FD_CONNECT = 0x10
ws = ctypes.WinDLL("Ws2_32")
ws.WSACreateEvent.argtypes = []
ws.WSACreateEvent.restype = ctypes.c_void_p
ws.WSAEventSelect.argtypes = [ctypes.c_size_t, ctypes.c_void_p, ctypes.c_long]
ws.WSAEventSelect.restype = ctypes.c_int
ws.WSACloseEvent.argtypes = [ctypes.c_void_p]
ws.WSACloseEvent.restype = ctypes.c_int
ws.WSAGetLastError.argtypes = []
ws.WSAGetLastError.restype = ctypes.c_int


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def resources():
    class MemoryStatus(ctypes.Structure):
        _fields_ = [("length", ctypes.c_ulong), ("load", ctypes.c_ulong),
                    *[(name, ctypes.c_ulonglong) for name in
                      ["totalPhysical", "availablePhysical", "totalPageFile",
                       "availablePageFile", "totalVirtual", "availableVirtual",
                       "availableExtendedVirtual"]]]

    memory = MemoryStatus()
    memory.length = ctypes.sizeof(memory)
    assert ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(memory))
    tcp = subprocess.run(["netstat", "-ano", "-p", "tcp"], capture_output=True,
                         text=True, timeout=15)
    rows = [line.split() for line in tcp.stdout.splitlines()]
    rows = [row for row in rows if len(row) == 5 and row[0] == "TCP"]
    states = {}
    for row in rows:
        states[row[3]] = states.get(row[3], 0) + 1
    return {"time": time.time(), "memoryLoadPercent": memory.load,
            "availablePhysicalBytes": memory.availablePhysical,
            "totalPhysicalBytes": memory.totalPhysical,
            "netstatExit": tcp.returncode, "netstatError": tcp.stderr,
            "localEndpoints": len({row[1] for row in rows}), "states": states,
            "acceptanceConnections": [row for row in rows
                                      if row[1].endswith(":4181") or row[2].endswith(":4181")]}


def request(index, randomized):
    record = {"index": index, "start": time.time(), "stage": "socket"}
    event = None
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM, socket.IPPROTO_TCP) as client:
            event = ws.WSACreateEvent()
            if not event:
                raise OSError(ws.WSAGetLastError(), "WSACreateEvent")
            client.setblocking(False)
            if ws.WSAEventSelect(client.fileno(), event, FD_CONNECT):
                raise OSError(ws.WSAGetLastError(), "WSAEventSelect")
            record["stage"] = "socket-option"
            record["defaultRandomizePort"] = client.getsockopt(socket.SOL_SOCKET, SO_RANDOMIZE_PORT)
            assert record["defaultRandomizePort"] == 0, "control socket must begin without randomization"
            if randomized:
                client.setsockopt(socket.SOL_SOCKET, SO_RANDOMIZE_PORT, 1)
            record["effectiveRandomizePort"] = client.getsockopt(socket.SOL_SOCKET, SO_RANDOMIZE_PORT)
            assert record["effectiveRandomizePort"] == int(randomized)
            record["stage"] = "connect"
            started = time.perf_counter()
            record["connectCode"] = client.connect_ex(ADDRESS)
            record["connectCallMs"] = (time.perf_counter() - started) * 1000
            if record["connectCode"] not in (0, 10035):
                raise OSError(record["connectCode"], "immediate connect")
            record["localAfterConnect"] = client.getsockname()
            if record["connectCode"] == 10035:
                record["stage"] = "connect-completion"
                _, ready, exceptional = select.select([], [client], [client], 5)
                assert ready or exceptional, "connect completion exceeded five seconds"
                record["completionCode"] = client.getsockopt(socket.SOL_SOCKET, socket.SO_ERROR)
                if record["completionCode"]:
                    raise OSError(record["completionCode"], "asynchronous connect")
            if ws.WSAEventSelect(client.fileno(), None, 0):
                raise OSError(ws.WSAGetLastError(), "cancel connect event")
            client.settimeout(5)
            record["stage"] = "http"
            asset = index % 5
            client.sendall(f"GET /asset-{asset}.bin HTTP/1.1\r\nHost: 127.0.0.1:4181\r\nConnection: close\r\n\r\n".encode())
            chunks = []
            while chunk := client.recv(32768):
                chunks.append(chunk)
            response = b"".join(chunks)
            headers, body = response.split(b"\r\n\r\n", 1)
            assert headers.split(b"\r\n", 1)[0] == b"HTTP/1.0 200 OK"
            assert body == bytes([asset + 1]) * 16384, "complete expected response body"
            record.update(stage="complete", passed=True, bytes=len(body))
    except Exception as error:
        record.update(passed=False, error=str(error), errno=getattr(error, "errno", None),
                      winerror=getattr(error, "winerror", None), traceback=traceback.format_exc())
    finally:
        if event and not ws.WSACloseEvent(event):
            record.update(passed=False, cleanupError=ws.WSAGetLastError())
        record["finish"] = time.time()
    return record


def phase(name, randomized):
    folder = OUT / name
    folder.mkdir()
    report = {"phase": name, "randomized": randomized, "attemptLimit": 60000,
              "durationLimitSeconds": 180, "concurrency": 4, "start": time.time(),
              "attempted": 0, "passed": 0, "failures": [], "before": resources()}
    deadline = time.monotonic() + report["durationLimitSeconds"]
    lock = threading.Lock()
    stop = threading.Event()
    next_index = 0
    with (folder / "connections.jsonl").open("w", encoding="utf-8") as log:
        def worker():
            nonlocal next_index
            while not stop.is_set() and time.monotonic() < deadline:
                with lock:
                    if next_index >= report["attemptLimit"]:
                        return
                    index = next_index
                    next_index += 1
                result = request(index, randomized)
                with lock:
                    log.write(json.dumps(result) + "\n")
                    report["attempted"] += 1
                    if result["passed"]:
                        report["passed"] += 1
                    else:
                        stop.set()
                        report["failures"].append(result)
                        if "failureResources" not in report:
                            report["failureResources"] = resources()
                    if report["attempted"] % 10000 == 0 or not result["passed"]:
                        print(json.dumps({"phase": name, "attempted": report["attempted"],
                                          "failures": len(report["failures"])}), flush=True)

        with ThreadPoolExecutor(max_workers=4) as pool:
            futures = [pool.submit(worker) for _ in range(4)]
            for future in futures:
                future.result()
    report["finish"] = time.time()
    report["after"] = resources()
    report["stopReason"] = ("observed failure" if stop.is_set() else
                            "attempt limit" if next_index == report["attemptLimit"] else "time limit")
    write(folder / "report.json", report)
    print(json.dumps({"phase": name, "attempted": report["attempted"],
                      "passed": report["passed"], "failures": len(report["failures"]),
                      "stopReason": report["stopReason"]}), flush=True)
    return report


def main():
    site = OUT / "site"
    site.mkdir()
    for index in range(5):
        (site / f"asset-{index}.bin").write_bytes(bytes([index + 1]) * 16384)
    files = [Path(__file__), Path("scripts/serve-client.py"), *sorted(site.iterdir())]
    write(OUT / "source.json", {
        "commit": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
        "sha256": {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in files},
        "scope": "Private loopback-only counterfactual; no canonical game, browser, firewall or registry changes."})
    write(OUT / "system.json", {"platform": platform.platform(), "python": sys.version,
                               "windowsVersion": list(sys.getwindowsversion())})
    for name, command in [("dynamic-ports", ["netsh", "int", "ipv4", "show", "dynamicport", "tcp"]),
                          ("excluded-ports", ["netsh", "int", "ipv4", "show", "excludedportrange", "protocol=tcp"])]:
        result = subprocess.run(command, capture_output=True, text=True, timeout=15)
        write(OUT / f"{name}.json", {"command": command, "code": result.returncode,
                                   "stdout": result.stdout, "stderr": result.stderr})
    with (OUT / "server.log").open("w") as stdout, (OUT / "server-error.log").open("w") as stderr:
        server = subprocess.Popen([sys.executable, "scripts/serve-client.py", "4181", "--bind",
                                   "127.0.0.1", "--directory", str(site)], stdout=stdout, stderr=stderr)
        try:
            deadline = time.monotonic() + 15
            while True:
                try:
                    with socket.create_connection(ADDRESS, timeout=1):
                        break
                except OSError:
                    if server.poll() is not None or time.monotonic() >= deadline:
                        raise
                    time.sleep(0.1)
            results = []
            for name, randomized in [("randomized-before", True), ("default-unset", False),
                                     ("randomized-restored", True)]:
                results.append(phase(name, randomized))
                write(OUT / "comparison.json", {"complete": len(results) == 3, "results": results,
                      "meaning": "Completed measurements do not imply all requests passed."})
        finally:
            if server.poll() is None:
                server.terminate()
            server.wait(timeout=15)
            write(OUT / "server-cleanup.json", {"pid": server.pid, "exit": server.returncode,
                                               "running": server.poll() is None})


if __name__ == "__main__":
    try:
        main()
    except Exception:
        write(OUT / "diagnostic-error.json", {"traceback": traceback.format_exc()})
        raise
