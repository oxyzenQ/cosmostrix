#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
# PLATFORM: UNIX-only (Linux, macOS, BSD). Uses grep, sed, wc, awk and
#   the cargo packaging machinery. Not for Windows cmd.exe / PowerShell
#   - use Git Bash or WSL on Windows.
#
# COSMOSTRIX PACKAGE-SIZE CAP CHECK
#
# The registry upload size is a hard external contract: crates.io
# refuses an oversized .crate with HTTP 413 Payload Too Large (max
# upload 10485760 bytes). The v100.1.3-dragon.2 publish died on
# exactly that answer in 1m36s: the demo webp grew to 9,217,878 bytes
# while the [package] exclude lane covered assets/*.gif and
# assets/*.png but not *.webp, pushing the tarball to ~11.25 MiB. No
# gate measured the package, so the refusal only surfaced at the
# registry. This gate builds the real .crate (metadata + packaging
# only, no verify build) and asserts its byte count against the
# documented server cap, so the refusal class fails in the CI package
# gate in about a minute instead of in the publish job.
#
# Equality with the cap fails too: the server's exact boundary
# behaviour at 10485760 bytes is not part of this contract, headroom
# is. The measured tarball must be the publish candidate, so the gate
# refuses to run over a dirty tree - untracked or modified files
# change the bytes the gate measures, and a size verdict over a tree
# that will never publish is a lie.
#
# Usage: bash scripts/gates/check-package-size.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

# crates.io's documented max upload size (the 413 answer quotes it).
CAP=10485760

if ! command -v cargo >/dev/null 2>&1; then
	echo "FAIL: cargo not found on PATH - the gate measures the real"
	echo ".crate bytes, which requires the packaging machinery."
	exit 1
fi

VERSION=$(grep -m1 '^version = ' Cargo.toml | sed 's/^version = "\(.*\)".*/\1/')
if [ -z "$VERSION" ]; then
	echo "FAIL: no package version line found in Cargo.toml"
	exit 1
fi

CRATE="target/package/cosmostrix-${VERSION}.crate"
rm -f "$CRATE"

if ! cargo package --locked --no-verify >/dev/null 2>&1; then
	echo "FAIL: cargo package --locked --no-verify refused the tree. The"
	echo "gate refuses dirty trees on purpose (untracked or modified"
	echo "files change the measured bytes) - commit or restore the"
	echo "working tree, then re-run this gate."
	exit 1
fi

if [ ! -f "$CRATE" ]; then
	echo "FAIL: expected $CRATE was not produced by cargo package"
	exit 1
fi

BYTES=$(wc -c <"$CRATE" | tr -d ' ')
MIB=$(awk -v b="$BYTES" 'BEGIN { printf "%.2f", b / 1048576 }')

if [ "$BYTES" -ge "$CAP" ]; then
	OVER=$(awk -v b="$BYTES" 'BEGIN { printf "%.2f", (b - 10485760) / 1048576 }')
	echo "FAIL: cosmostrix-${VERSION}.crate is ${BYTES} bytes (${MIB} MiB)"
	echo "- at or over the crates.io hard upload cap of ${CAP} bytes"
	echo "(10 MiB), over by ${OVER} MiB. The registry answers such an"
	echo "upload with HTTP 413 Payload Too Large. Find the heavyweight"
	echo "that entered the package (cargo package --list) and exclude it"
	echo "in Cargo.toml [package] exclude."
	exit 1
fi

HEADROOM=$(awk -v b="$BYTES" 'BEGIN { printf "%.2f", (10485760 - b) / 1048576 }')
echo "OK: cosmostrix-${VERSION}.crate is ${BYTES} bytes (${MIB} MiB),"
echo "under the crates.io hard upload cap of ${CAP} bytes (10 MiB) with"
echo "${HEADROOM} MiB headroom."
