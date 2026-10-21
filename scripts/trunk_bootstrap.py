"""The one external initializer used by hosted and desktop Trunk clients."""

import hashlib
import re

INLINE_MODULE = re.compile(r'<script type="module">(.*?)</script>', re.DOTALL)


def bootstrap_name(bootstrap: str) -> str:
    return "solomon-boot-" + hashlib.sha256(bootstrap.encode()).hexdigest()[:16] + ".mjs"


def validate_trunk_bootstrap(bootstrap: str, mount: str) -> None:
    prefix = mount.rstrip("/") + "/"
    module = re.search(r"\bfrom\s+(['\"])([^'\"]+\.js)\1", bootstrap)
    wasm = re.search(r"module_or_path:\s*(['\"])([^'\"]+\.wasm)\1", bootstrap)
    if module is None or wasm is None or "window.wasmBindings = bindings" not in bootstrap:
        raise ValueError("Trunk's bootstrap changed; review its initialization")
    for resource in (module[2], wasm[2]):
        if not resource.startswith(prefix):
            raise ValueError(f"Trunk's generated assets must use the {prefix} public URL")
        filename = resource[len(prefix):]
        if not filename or "/" in filename or filename in (".", ".."):
            raise ValueError(f"Trunk's generated assets must be directly below {prefix}")


def externalize_trunk_bootstrap(document: str, mount: str) -> tuple[str, str, str]:
    scripts = list(INLINE_MODULE.finditer(document))
    if len(scripts) != 1:
        raise ValueError("expected one inline Trunk module in the completed distribution")
    script = scripts[0]
    bootstrap = script[1].strip() + "\n"
    validate_trunk_bootstrap(bootstrap, mount)
    name = bootstrap_name(bootstrap)
    public_path = mount.rstrip("/") + "/" + name
    document = document[:script.start()] + (
        f'<script type="module" src="{public_path}"></script>'
    ) + document[script.end():]
    return document, name, bootstrap
