#!/usr/bin/env bash
# Regenerates every Website differential fixture by running the unmodified Website kernels
# under Node. Run it after the Website checkout changes or a new oracle script lands, then
# `cargo test -p solomon-sim --test website_goldens` must still pass.
#
# Usage: scripts/record-website-goldens.sh            (uses ../Website)
#        SOLOMON_WEBSITE_DIR=/path/to/Website scripts/record-website-goldens.sh
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
for script in "$root"/tools/website-oracle/*-goldens.mjs; do
  node --no-warnings --experimental-strip-types "$script"
done
