#!/usr/bin/env python3
"""Retain the exact reused client and native PE stack boundary for Windows diagnosis."""

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import struct
import subprocess
import sys
import time
import zipfile


def file_sha256(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def extract_client(artifact_directory, destination, report):
    archive = artifact_directory / "Solomon-Darker-linux-x64.zip"
    expected = Path(str(archive) + ".sha256").read_text().split()[0]
    actual = file_sha256(archive)
    if actual != expected:
        raise ValueError("The reused desktop archive does not match its checksum")
    prefix = "Solomon Darker-linux-x64/resources/app/client/"
    files = []
    with zipfile.ZipFile(archive) as source:
        for member in source.infolist():
            if member.is_dir() or not member.filename.startswith(prefix):
                continue
            relative = PurePosixPath(member.filename.removeprefix(prefix))
            if relative.is_absolute() or ".." in relative.parts:
                raise ValueError("The reused client contains an invalid relative path")
            target = destination.joinpath(*relative.parts)
            target.parent.mkdir(parents=True, exist_ok=True)
            with source.open(member) as incoming, target.open("wb") as outgoing:
                shutil.copyfileobj(incoming, outgoing)
            files.append({"path": str(relative), "sha256": file_sha256(target)})
    if not (destination / "index.html").is_file():
        raise ValueError("The reused archive has no root desktop client")
    release = json.loads((artifact_directory / "desktop-release.json").read_text())
    report.parent.mkdir(parents=True, exist_ok=True)
    report.write_text(json.dumps({
        "sourceRunId": 39676774215,
        "sourceArtifact": "desktop-linux-x64",
        "sourceRevision": release["revision"],
        "archiveSha256": actual,
        "clientFiles": files,
    }, indent=2) + "\n")
    print(f"Verified and extracted {len(files)} unchanged client files at {release['revision']}")


def pe_header(executable, report):
    data = executable.read_bytes()
    pe_offset = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe_offset:pe_offset + 4] != b"PE\0\0":
        raise ValueError("The native executable has no PE signature")
    optional_header = pe_offset + 24
    magic = struct.unpack_from("<H", data, optional_header)[0]
    if magic != 0x20B:
        raise ValueError("The diagnostic expects the actual Windows x64 PE32+ executable")
    report.parent.mkdir(parents=True, exist_ok=True)
    result = {
        "executable": executable.name,
        "sha256": hashlib.sha256(data).hexdigest(),
        "optionalHeaderMagic": hex(magic),
        "stackReserveBytes": struct.unpack_from("<Q", data, optional_header + 72)[0],
        "stackCommitBytes": struct.unpack_from("<Q", data, optional_header + 80)[0],
    }
    report.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))


def tail(path, maximum=8192):
    if not path.is_file():
        return ""
    with path.open("rb") as stream:
        stream.seek(max(0, path.stat().st_size - maximum))
        return stream.read(maximum).decode("utf-8", errors="replace")


def print_summary(directory):
    print(json.dumps({"diagnosticSummary": "phase", "tail": tail(directory / "phases.jsonl")}), flush=True)
    for path in sorted((directory / "native-child").glob("*.jsonl")):
        print(json.dumps({"diagnosticSummary": "native", "file": path.name,
                          "bytes": path.stat().st_size, "tail": tail(path)}), flush=True)
    for name in ["original-error.json", "failure.json", "receipt.json"]:
        path = directory / name
        if path.is_file():
            print(json.dumps({"diagnosticSummary": name, "tail": tail(path)}), flush=True)


OWNED_CLEANUP = r'''
param([string]$RecordsPath)
Get-Content -LiteralPath $RecordsPath | ForEach-Object {
  $record = $_ | ConvertFrom-Json
  $taskPid = [int]$record.pid
  $taskProcess = Get-CimInstance Win32_Process -Filter "ProcessId=$taskPid"
  if ($null -eq $taskProcess) {
    @{ cleanup = 'already-exited'; pid = $taskPid; label = $record.label } | ConvertTo-Json -Compress
  } elseif ([string]::Equals([IO.Path]::GetFullPath($taskProcess.ExecutablePath), [IO.Path]::GetFullPath($record.executable), [StringComparison]::OrdinalIgnoreCase)) {
    @{ cleanup = 'owned-tree'; pid = $taskPid; label = $record.label; workingSetBytes = $taskProcess.WorkingSetSize; privateBytes = $taskProcess.PrivatePageCount } | ConvertTo-Json -Compress
    & taskkill.exe /PID $taskPid /T /F 2>&1 | Select-Object -Last 8
  } else {
    @{ cleanup = 'identity-mismatch-skipped'; pid = $taskPid; label = $record.label } | ConvertTo-Json -Compress
  }
}
'''


def cleanup_owned(directory):
    records = directory / "owned-processes.jsonl"
    if not records.is_file():
        return
    script = directory / "cleanup-owned.ps1"
    script.write_text(OWNED_CLEANUP)
    result = subprocess.run(
        ["powershell.exe", "-NoProfile", "-NonInteractive", "-File", str(script),
         "-RecordsPath", str(records)], capture_output=True, timeout=25,
    )
    text = (result.stdout + result.stderr)[-16384:].decode("utf-8", errors="replace")
    print(json.dumps({"diagnosticCleanupExit": result.returncode, "tail": text}), flush=True)


def smoke(directory, timeout_seconds):
    if sys.platform != "win32":
        raise ValueError("The owned Windows process-tree watchdog runs on Windows only")
    directory = directory.resolve()
    directory.mkdir(parents=True, exist_ok=True)
    node = Path(shutil.which("node")).resolve()
    process = subprocess.Popen([str(node), "tools/smoke-desktop-runtime.mjs"], env=os.environ.copy())
    record = {"label": "smoke-harness", "pid": process.pid, "executable": str(node)}
    with (directory / "owned-processes.jsonl").open("a") as stream:
        stream.write(json.dumps(record) + "\n")
    started = time.monotonic()
    deadline = started + timeout_seconds
    expired = False
    try:
        while process.poll() is None:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                expired = True
                break
            try:
                process.wait(timeout=min(15, remaining))
            except subprocess.TimeoutExpired:
                print(json.dumps({"diagnosticWatchdogSeconds": round(time.monotonic() - started),
                                  "phaseTail": tail(directory / "phases.jsonl", 2048)}), flush=True)
        if expired:
            print(json.dumps({"diagnosticWatchdogExpired": timeout_seconds}), flush=True)
    finally:
        try:
            cleanup_owned(directory)
        finally:
            if process.poll() is None:
                process.kill()
            process.wait(timeout=10)
            print_summary(directory)
    return 124 if expired else process.returncode


parser = argparse.ArgumentParser(description=__doc__)
commands = parser.add_subparsers(dest="command", required=True)
extract = commands.add_parser("extract-client")
extract.add_argument("artifact_directory", type=Path)
extract.add_argument("destination", type=Path)
extract.add_argument("report", type=Path)
pe = commands.add_parser("pe-header")
pe.add_argument("executable", type=Path)
pe.add_argument("report", type=Path)
watch = commands.add_parser("smoke")
watch.add_argument("directory", type=Path)
watch.add_argument("--timeout-seconds", type=int, default=720)
recover = commands.add_parser("recover")
recover.add_argument("directory", type=Path)
args = parser.parse_args()
if args.command == "extract-client":
    extract_client(args.artifact_directory, args.destination, args.report)
elif args.command == "pe-header":
    pe_header(args.executable, args.report)
elif args.command == "smoke":
    raise SystemExit(smoke(args.directory, args.timeout_seconds))
else:
    try:
        cleanup_owned(args.directory.resolve())
    finally:
        print_summary(args.directory)
