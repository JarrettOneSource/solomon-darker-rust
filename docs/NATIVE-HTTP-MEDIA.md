# Native HTTP media

Static asset responses must survive a paused reader and shut down promptly.
Browsers may stop downloading media after buffering enough data and resume later.
A TCP write timeout does not prove the reader disconnected.

## Ownership

The native transport parses bounded request headers under its handshake deadline.
Static responses then leave the framing pool and run in owned transfer workers.
Each retains its connection lease and shares the immutable `AssetServer`.

The asset worker streams the declared body once using nonblocking writes.
On `WouldBlock`, it retains the pending offset, waits for the existing 10 ms
I/O interval, and checks shutdown before trying again. A real write error ends the
response. There is no asset-consumption write deadline.

Accepted connections remain bounded by the transport's admission limit.
Slow asset readers cannot occupy all request-framing workers. Shutdown closes
retained sockets and joins owned workers; writes also check the stop flag because
closing a socket clone alone may not cancel an already-blocked platform send.
Admission and shutdown share the connection-bookkeeping lock.

Supervisor HTTP keeps its own write deadline. Request/header limits, upgrade
deadlines, WebSocket heartbeat/close rules, MIME types, path validation, and byte
ranges retain their respective owners.

## Regression checks

Tests use public `NativeServer` and real loopback TCP:

- A paused ranged response resumes with its complete length and SHA256.
- Four stalled asset bodies leave a small request serviceable.
- Shutdown completes while those readers remain paused.
- MIME, path, range, framing, upgrade, and API semantics still pass.

The pause/resume test checks integrity; it does not guarantee reproduction of a
particular operating system's blocked-send timeout. Browser playback remains an
independent integration check. Unexpected truncation is an error, distinct from a
trusted media suspension or deliberate cancellation.

Implementation lives in the launcher's transport and asset-server modules.
The contract follows [HTML media resource loading](https://html.spec.whatwg.org/multipage/media.html#concept-media-load-resource),
[Rust TCP write timeouts](https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.set_write_timeout),
and [Windows socket shutdown](https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-shutdown).
