"""Serve browser acceptance files with MIME types independent of the OS registry."""

import mimetypes
import runpy


if __name__ == "__main__":
    mimetypes.add_type("text/javascript", ".js")
    mimetypes.add_type("text/javascript", ".mjs")
    mimetypes.add_type("application/wasm", ".wasm")
    runpy.run_module("http.server", run_name="__main__")
