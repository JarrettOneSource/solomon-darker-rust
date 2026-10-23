"""Canonical external loader-first initialization for every Trunk distribution."""

import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
BOOT_SCRIPT = re.compile(r'<script type="module" src="([^"]*solomon-boot-[0-9a-f]{16}\.mjs)"></script>')
RESOURCE_NAMES = {
    "loaderModule": r"solomon-loader-[0-9a-f]+\.js",
    "loaderWasm": r"solomon-loader-[0-9a-f]+_bg\.wasm",
    "mainModule": r"solomon-web-[0-9a-f]+\.js",
    "mainWasm": r"solomon-web-[0-9a-f]+_bg\.wasm",
}


def bootstrap_name(bootstrap: str) -> str:
    return "solomon-boot-" + hashlib.sha256(bootstrap.encode()).hexdigest()[:16] + ".mjs"


def mount_prefix(mount: str) -> str:
    if not re.fullmatch(r"/(?:[a-z0-9_-]+/)*[a-z0-9_-]*", mount):
        raise ValueError("the game public URL must be an absolute local mount path")
    return mount.rstrip("/") + "/"


def render_bootstrap(resources: dict[str, str]) -> str:
    if not isinstance(resources, dict) or set(resources) != set(RESOURCE_NAMES):
        raise ValueError("expected loader and main JavaScript/WASM resources")
    for key, pattern in RESOURCE_NAMES.items():
        if not isinstance(resources[key], str) or not re.fullmatch(pattern, resources[key]):
            raise ValueError(f"invalid hashed {key} resource")
    template = (ROOT / "web/bootstrap.mjs").read_text(encoding="utf-8")
    return template.replace("__SOLOMON_RESOURCES__", json.dumps(resources, sort_keys=True))


def validate_trunk_bootstrap(bootstrap: str, mount: str) -> None:
    mount_prefix(mount)
    match = re.search(r"^const resources = (\{[^\n]+\});$", bootstrap, re.MULTILINE)
    if match is None:
        raise ValueError("expected the canonical Rust loader bootstrap")
    try:
        resources = json.loads(match[1])
    except json.JSONDecodeError as error:
        raise ValueError("invalid bootstrap resource manifest") from error
    if render_bootstrap(resources) != bootstrap:
        raise ValueError("the game initializer differs from the canonical loader bootstrap")


def prepare_trunk_bootstrap(dist: Path, mount: str) -> tuple[str, str, str]:
    prefix = mount_prefix(mount)
    index = dist / "index.html"
    document = index.read_text(encoding="utf-8")
    if '<meta name="solomon-bootstrap" content="gpu-loader-v1"' not in document:
        raise ValueError("expected the GPU loader document")
    if re.search(r'<script\b[^>]*>\s*[^<\s]', document):
        raise ValueError("the game document must not contain inline JavaScript")
    bases=re.findall(r'<base\b[^>]*href=[\'"]([^\'"]+)[\'"][^>]*>',document)
    if bases != [prefix]:
        raise ValueError(f"the game document must use the {prefix} public URL")
    resources = {}
    files = [path.name for path in dist.iterdir() if path.is_file()]
    for key, pattern in RESOURCE_NAMES.items():
        candidates = [name for name in files if re.fullmatch(pattern, name)]
        if len(candidates) != 1:
            raise ValueError(f"expected exactly one hashed {key}; found {len(candidates)}")
        resources[key] = candidates[0]
    bootstrap = render_bootstrap(resources)
    name = bootstrap_name(bootstrap)
    scripts = list(BOOT_SCRIPT.finditer(document))
    if len(scripts) > 1:
        raise ValueError("the game document contains multiple initializers")
    without_bootstrap = BOOT_SCRIPT.sub("", document)
    if re.search(r'<script\b', without_bootstrap):
        raise ValueError("the game document has an unowned script")
    if document.count("</body>") != 1:
        raise ValueError("expected one game document body")
    tag=f'<script type="module" src="{prefix}{name}"></script>'
    if scripts:
        previous=dist / Path(scripts[0][1]).name
        existing=previous.read_text(encoding="utf-8")
        validate_trunk_bootstrap(existing,mount)
        if bootstrap_name(existing)!=previous.name or existing!=bootstrap or scripts[0][1]!=prefix+name:
            raise ValueError("the existing initializer does not match this distribution")
    else:
        document=document.replace("</body>",tag+"\n</body>")
    (dist / name).write_text(bootstrap, encoding="utf-8")
    index.write_text(document, encoding="utf-8")
    return document, name, bootstrap
