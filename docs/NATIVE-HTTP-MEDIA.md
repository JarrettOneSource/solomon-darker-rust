# Native HTTP media backpressure

## Confirmed failure

The unchanged Tutorial journey against immutable game9fab/nativee607 produced an
uncancelled truncated206 for `combat.mp3`. The separate ef5d diagnostic native binary
observed one `io::copy` for an advertised18,799,976-byte response begin at
1792816542.571333 and return OS35/WouldBlock at1792816577.795616:35.224308166seconds.
Chrome request4272.404 started at1792816542.499295, observed206 and the declared size,
received6,291,456 data bytes/5,242,880 encoded bytes, and later reported the uncancelled
content-length failure at171.273seconds. This is a native response truncation, not a
permitted browser cancellation or an acceptable UI/audio error.

Raw trace and unchanged-driver result are retained under the full-UI task's
`common-browser/9fab4e4/m5/tutorial-native-transport-trace-r2/`, including
`native-host.log`, `receipt.json`, `browser-errors-1.json` and `launch-identity.json`.
The diagnostic is separate from the serving immutable native; it is not part of the fix.

## Owner and contract

`transport/socket.rs::serve_connection` reads the complete bounded request headers under
the existing handshake deadline, then sets a30-second TCP write deadline and calls the
asset responder inline on one of four framing workers. `AssetServer::serve_stream`
writes the declared range headers and streams the file once with `io::copy`. A write
timeout propagates out, dropping the connection after only part of the declared body.
Four independently paused bodies can also occupy all framing workers.

The [HTML media loading contract](https://html.spec.whatwg.org/multipage/media.html#concept-media-load-resource)
allows a user agent to stop downloading after sufficient buffering and resume later.
The [Rust TCP write-timeout contract](https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.set_write_timeout)
documents that Unix can report a timeout as WouldBlock; that error does not establish
that the reader disconnected. Raising the timeout or retrying a partly written body
would not establish the response contract.

The native transport already bounds accepted connections with `ConnectionLease`, keeps
their socket clones for shutdown, and owns/reaps/join its long-lived workers. Static
responses belong to that lifetime after header framing: share the immutable asset
responder, keep the lease in an owned transfer worker, and remove the asset consumption
write deadline. No asset may occupy a framing worker while waiting for a reader.
Shutdown closes the owned TCP connections before joining workers; the standard
[TCP shutdown contract](https://doc.rust-lang.org/std/net/enum.Shutdown.html) wakes blocked
writes. Connection count remains bounded by the existing admission limit.

Request read/header limits, upgrade deadlines, supervisor HTTP deadlines, WebSocket
heartbeat/close behavior, MIME, path validation and byte-range semantics stay with their
existing owners. The asset file streams directly to TCP without a new body buffer,
retry loop, polling protocol, async runtime or frontend error allowance.

## Verification boundary

Regressions use public `NativeServer` and real loopback TCP: a paused ranged response
must resume past the original deadline with its complete length/SHA256; four stalled
asset bodies must leave a small request serviceable; shutdown must finish while those
readers remain paused. Existing framing/upgrade/API and asset semantics tests remain
required. Record the observed red before implementation, then native focused/full
gates and immutable fixed-native identity. Root/QA owns the original browser rerun and
paired artifact admission; a native regression alone does not claim that browser result.

The initial public-TCP regression on source0e74d53 demonstrated framing starvation:
four paused64MiB bodies made the small request hit its actual2s client read deadline
(OS35/WouldBlock). Its18,799,976-byte single-range control still passed after40s, so
that control did not establish a blocked sender and is not a timeout reproduction.
Job `job_20261024T044655Z_f60a766645` retains both independent outcomes. The range is
therefore strengthened to the64MiB fixture minus its17-byte offset, without changing
production deadlines or adding a test-only transport knob.
