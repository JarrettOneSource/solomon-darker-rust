#!/usr/bin/env python3
"""Retain the exact reused client and native PE stack boundary for Windows diagnosis."""

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import struct
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


parser = argparse.ArgumentParser(description=__doc__)
commands = parser.add_subparsers(dest="command", required=True)
extract = commands.add_parser("extract-client")
extract.add_argument("artifact_directory", type=Path)
extract.add_argument("destination", type=Path)
extract.add_argument("report", type=Path)
pe = commands.add_parser("pe-header")
pe.add_argument("executable", type=Path)
pe.add_argument("report", type=Path)
args = parser.parse_args()
if args.command == "extract-client":
    extract_client(args.artifact_directory, args.destination, args.report)
else:
    pe_header(args.executable, args.report)
