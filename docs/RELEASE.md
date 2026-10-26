# Release and deployment

## Qualified release

The qualified game release is
[`92e3fe4c1bd7ca1eacff871ff79e604f904dfcf1`](https://github.com/JarrettOneSource/solomon-darker-rust/releases/tag/desktop-92e3fe4c1bd7ca1eacff871ff79e604f904dfcf1).
[Its workflow](https://github.com/JarrettOneSource/solomon-darker-rust/actions/runs/40248178185)
passes the game gate and packaged journeys for Windows x64, Linux x64, Intel Mac,
and Apple silicon Mac. The release contains four desktop archives, their checksums,
the update manifest, and the Linux game artifact and checksum.

The recorded production installation at
[solomondarker.com/game](https://solomondarker.com/game/) uses that game through
website `db9a9c98434aa5df56e3f9c402719209001f061b`.
Documentation commits do not change the qualified runtime identity.

The Linux artifact SHA256 is
`29a9a722dd0ba8d5b83ba11997a7b0fc527fdb13e75d64dda9ee37b60299ab87`.
Its manifest SHA256 is
`4e1a633291c1aaa8ed67806957229e11877e9248cc67ad58d04db55204870aea`.

## Ownership

This repository publishes the Rust game, native authority, WebAssembly client,
save tools, mod compiler, and desktop app. The website consumes a verified artifact
and owns accounts, cloud storage, community content, and editing.
The original C++ game remains a retail reference.

The published desktop app uses the shell described in
[Desktop runtime](DESKTOP-RUNTIME.md). Native-window work is tracked separately
in [Open work](WORK-QUEUE.md) and is not an installed-release claim.

## Qualification

Source validation uses [the game checks](../AGENTS.md#checks).
A release additionally requires:

1. Build each platform's native host and client from the intended revision.
2. Verify complete asset membership, payload hashes, runtime identity, archive
   contents, and platform requirements.
3. Exercise the final packages through offline play, save/Continue, rejected-save
   handling, return to launcher, update presentation, and encrypted peer play.
4. Review screenshots, resource/media errors, and process cleanup.
5. Publish only the qualified archives and their checksums.

The configured workflow is `.github/workflows/validate.yml`. A configured job is
not evidence that a particular run passed. Software-renderer checks establish
functionality; [Performance](PERFORMANCE.md) describes the measured workload.

## Installation and live acceptance

The owning deployment worker validates the artifact before installation and
preserves existing account, save, and content data. It drains active sessions,
waits for durable checkpoints and acknowledgements, then admits the replacement.

Verify the installed manifest and payload membership, native supervisor identity,
HTTPS documents, content policy, revision identity, and deployment readiness.
The website publication must agree with the announced game revision and sequence.

Live acceptance checks that Saving remains visible until persistence, the matching
ACK precedes transport retirement, and the browser loads the exact replacement.
A fresh authenticated read must match the saved checkpoint. Continue must restore
the same wizard and loadout, accept movement, save through Leave, and survive a
fresh document.

The qualified installation passes authenticated save and continuation checks.
That acceptance establishes persisted continuation and replacement identity;
it does not establish zero transient HTTP errors during deployment.

## Recovery and evidence

Device-owned deployment guides hold runtime paths, managed release inputs, backup
locations, credentials, and scheduler operation. Rollback must preserve newer
database, save, and content data.

Keep test artifacts tied to the revision and package actually exercised. Current
documentation records contracts and qualified identities; commit history and CI
artifacts retain previous attempts and receipts.
