"""Serve browser acceptance files with MIME types independent of the OS registry."""

import mimetypes
import runpy
import socket
import socketserver


if __name__ == "__main__":
    mimetypes.add_type("text/javascript", ".js")
    mimetypes.add_type("text/javascript", ".mjs")
    mimetypes.add_type("application/wasm", ".wasm")
    # The browser loads assets concurrently; Python 3.12's queue of five can
    # refuse connections while the software renderer occupies the CI worker.
    socketserver.TCPServer.request_queue_size = socket.SOMAXCONN
    runpy.run_module("http.server", run_name="__main__")
