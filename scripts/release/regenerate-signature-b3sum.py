#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
"""Regenerate assets/signature.b3sum over the signed set.

The signed set is every image under assets/ (png, webp, jpg, jpeg)
plus the D5 GPG trust anchors under assets/gpg/ (the estate law the
zelynic signature-assets.md established). Digests come from the
project's own pure-Python BLAKE3 mirror in
scripts/release/generate-provenance-words.py, which is vector-pinned
to the blake3 crate and proven byte-identical to the official b3sum
CLI against the tracked sidecar — so this script reproduces exactly
what `b3sum <files>` would write, on hosts without b3sum installed.

Run from the repository root:

    python3 scripts/release/regenerate-signature-b3sum.py
"""

import importlib.util
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SIDECAR = REPO_ROOT / "assets" / "signature.b3sum"

IMAGE_SUFFIXES = {".png", ".webp", ".jpg", ".jpeg"}


def load_blake3():
    """Import the generator's BLAKE3 mirror without polluting sys.path."""
    src = REPO_ROOT / "scripts" / "release" / "generate-provenance-words.py"
    spec = importlib.util.spec_from_file_location("gpw", src)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.blake3


def signed_set() -> list[Path]:
    """Every tracked asset under the signed set, sorted for stability."""
    assets = REPO_ROOT / "assets"
    files = [
        path
        for path in assets.rglob("*")
        if path.is_file()
        and (path.suffix.lower() in IMAGE_SUFFIXES or "assets/gpg/" in path.as_posix())
    ]
    return sorted(files, key=lambda p: p.as_posix())


def main() -> int:
    blake3 = load_blake3()
    lines = []
    for path in signed_set():
        digest = blake3(path.read_bytes()).hex()
        rel = path.relative_to(REPO_ROOT).as_posix()
        lines.append(f"{digest}  {rel}")
    SIDECAR.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {SIDECAR.relative_to(REPO_ROOT).as_posix()} ({len(lines)} entries):")
    for line in lines:
        print(f"  {line}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
