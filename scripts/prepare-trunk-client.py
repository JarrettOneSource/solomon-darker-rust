#!/usr/bin/env python3
"""Trunk post-build hook: install the shared external Rust-loader bootstrap."""

import os
from pathlib import Path

from trunk_bootstrap import prepare_trunk_bootstrap


if __name__ == "__main__":
    prepare_trunk_bootstrap(Path(os.environ["TRUNK_STAGING_DIR"]), os.environ["TRUNK_PUBLIC_URL"])
