#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
#
# COSMOSTRIX RUST VERSION SYNC CHECK
#
# Verifies the Rust toolchain version is consistent across all sources:
#   1. rust-toolchain.toml  (channel = "X.Y.Z" — authoritative source)
#   2. Cargo.toml           (rust-version = "X.Y" — MSRV for downstream)
#   3. scripts/pgo-runner/Cargo.toml (rust-version = "X.Y" — pgo-runner MSRV)
#   4. .github/workflows/*.yml       (env: RUST_VERSION — CI install version)
#
# Fails if any source disagrees with the others.
#
# Usage: bash scripts/check-rust-version-sync.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# ── 1. rust-toolchain.toml (authoritative) ──
TOOLCHAIN_FILE="rust-toolchain.toml"
if [ ! -f "$TOOLCHAIN_FILE" ]; then
    echo "FAIL: $TOOLCHAIN_FILE not found"
    exit 1
fi
FULL_VERSION=$(grep '^channel = ' "$TOOLCHAIN_FILE" | sed 's/channel = "\(.*\)".*/\1/')
if [ -z "$FULL_VERSION" ]; then
    echo "FAIL: could not parse channel from $TOOLCHAIN_FILE"
    exit 1
fi

# Extract major.minor (strip patch) for MSRV comparison
MSRV_EXPECTED=$(echo "$FULL_VERSION" | sed 's/\([0-9]*\.[0-9]*\)\..*/\1/')
if [ -z "$MSRV_EXPECTED" ]; then
    echo "FAIL: could not derive MSRV from full version '$FULL_VERSION'"
    exit 1
fi

echo "Authoritative source: rust-toolchain.toml channel = \"$FULL_VERSION\""
echo "Expected MSRV (major.minor): $MSRV_EXPECTED"
echo ""

FAILED=0

# ── 2. Cargo.toml ──
CARGO_MSRV=$(grep '^rust-version = ' Cargo.toml | sed 's/rust-version = "\(.*\)".*/\1/')
if [ "$CARGO_MSRV" != "$MSRV_EXPECTED" ]; then
    echo "FAIL: Cargo.toml rust-version = \"$CARGO_MSRV\" — expected \"$MSRV_EXPECTED\""
    FAILED=$((FAILED + 1))
else
    echo "OK: Cargo.toml rust-version = \"$CARGO_MSRV\""
fi

# ── 3. scripts/pgo-runner/Cargo.toml ──
PGO_CARGO="scripts/pgo-runner/Cargo.toml"
if [ -f "$PGO_CARGO" ]; then
    PGO_MSRV=$(grep '^rust-version = ' "$PGO_CARGO" | sed 's/rust-version = "\(.*\)".*/\1/')
    if [ "$PGO_MSRV" != "$MSRV_EXPECTED" ]; then
        echo "FAIL: $PGO_CARGO rust-version = \"$PGO_MSRV\" — expected \"$MSRV_EXPECTED\""
        FAILED=$((FAILED + 1))
    else
        echo "OK: $PGO_CARGO rust-version = \"$PGO_MSRV\""
    fi
fi

# ── 4. .github/workflows/*.yml ──
for wf in .github/workflows/*.yml; do
    [ -f "$wf" ] || continue
    # Skip miri.yml (uses nightly, not the pinned version)
    [ "$(basename "$wf")" = "miri.yml" ] && continue
    # Skip docs-ci.yml (no Rust toolchain)
    [ "$(basename "$wf")" = "docs-ci.yml" ] && continue
    # Skip aur.yml (uses pre-built binaries, no Rust install)
    [ "$(basename "$wf")" = "aur.yml" ] && continue

    if grep -q 'RUST_VERSION:' "$wf"; then
        WF_VERSION=$(grep 'RUST_VERSION:' "$wf" | head -1 | sed 's/.*RUST_VERSION: *"\(.*\)".*/\1/')
        if [ "$WF_VERSION" != "$FULL_VERSION" ]; then
            echo "FAIL: $wf RUST_VERSION = \"$WF_VERSION\" — expected \"$FULL_VERSION\""
            FAILED=$((FAILED + 1))
        else
            echo "OK: $wf RUST_VERSION = \"$WF_VERSION\""
        fi
    fi
done

echo ""
if [ "$FAILED" -gt 0 ]; then
    echo "FAIL: $FAILED source(s) out of sync with rust-toolchain.toml"
    echo ""
    echo "To fix: update all sources to match, then rerun this check."
    echo "  rust-toolchain.toml  → channel = \"$FULL_VERSION\""
    echo "  Cargo.toml           → rust-version = \"$MSRV_EXPECTED\""
    echo "  scripts/pgo-runner/Cargo.toml → rust-version = \"$MSRV_EXPECTED\""
    echo "  .github/workflows/*.yml → RUST_VERSION: \"$FULL_VERSION\""
    exit 1
else
    echo "OK: all Rust version sources in sync"
    exit 0
fi
