# Release and deployment

## Qualified release

The [published releases](https://github.com/JarrettOneSource/solomon-darker-rust/releases)
contain the native Rust desktop archives, checksums, update manifest and Linux
hosted-game artifact. Each revision's successful
[workflow](https://github.com/JarrettOneSource/solomon-darker-rust/actions/workflows/validate.yml)
identifies its source and platform qualification. Confirm that identity before
selecting an artifact; a branch build or an unqualified diagnostic archive is
not a release.

Production serves the Rust game at
[solomondarker.com/game-rust](https://solomondarker.com/game-rust/) alongside the
separate PixiJS game at `/game`. Each retains its own save namespace. Device-owned
project guides and deployment publications record the installed revisions and
live acceptance; a newly published release is not automatically installed.

## Ownership

This repository publishes the Rust game, native authority, WebAssembly client,
save tools, mod compiler, and desktop app. The website consumes a verified artifact
and owns accounts, cloud storage, community content, and editing.

Desktop packages use the native Rust app described in
[Desktop runtime](DESKTOP-RUNTIME.md). Older Electron archives are historical
releases, not runtime dependencies of the native packages.

## Qualification

Source validation uses [the game checks](../AGENTS.md#checks).
A release additionally requires:

1. Build each platform's native executable and the Website artifact from the intended revision.
2. Verify complete asset membership, payload hashes, runtime identity, archive
   contents, and platform requirements.
3. Exercise the final packages through offline play, save/Continue, rejected-save
   handling, return to launcher, update presentation, and encrypted peer play.
   Exercise the browser embedding's gameplay and settings/persistence on Windows.
4. Review screenshots, resource/media errors, and process cleanup.
5. Publish only the qualified archives and their checksums.

The release workflow is `.github/workflows/validate.yml`.
[Performance](PERFORMANCE.md) documents hardware measurements separately from
the functional package checks.

The package journey uses ordinary input against the actual launcher and game.
Save checks compare completed documents and fresh-process continuation; a title
screen or successful WASM load alone does not establish offline play. Keep
controlled setup, emulated capabilities, and hardware results identifiable.
Inspect media-channel state before classifying a cancellation as expected, and
require owned processes to exit cleanly.

The native driver retries transient macOS accessibility reads and idempotent focus
operations within their deadlines. An unavailable close acknowledgement is recorded
without repeating the action; the actual process exit and save, or the expected
save-failure choices, remain required. Failure cleanup closes guests before their
host so each can finish saving.

Peer movement holds ordinary keyboard input until a real checkpoint from the same
run records eastward displacement and a later footstep tick. A software-rendered
frame can outlast a fixed key hold. The observation uses the bounded checkpoint
wait and always releases the key; the final saved displacement and clean process
exits remain required.

CI creates immutable package candidates before acceptance and publishes qualified
archives only after the required platform journeys pass. Diagnostic artifacts
named `unqualified-desktop-*` are excluded from release selection. Confirm the
published repository, revision, update manifest, and checksums match the tested
candidate, then verify the downloadable assets themselves.

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

## Recovery and evidence

Device-owned deployment guides hold runtime paths, managed release inputs, backup
locations, credentials, and scheduler operation. Rollback must preserve newer
database, save, and content data.

Keep test artifacts tied to the revision and package actually exercised.

Preserve valid evidence for unchanged behavior and repeat affected checks when
code, assets, packaging, or an observed failure changes the acceptance question.
Keep original failures attributable rather than overwriting them with later
results. Source validation, package qualification, installation, and live player
continuation each establish their own boundary.
