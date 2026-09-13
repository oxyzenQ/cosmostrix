#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).
#
# cosmostrix Rust version bumper - owner-facing entry point.
#
# One command bumps the pinned Rust toolchain version in every
# structural source, with no manual editing:
#
#   ./scripts/bump-rust-to.sh 1.98.1
#
# Updates (via the implementation script):
#   - rust-toolchain.toml      channel = "X.Y.Z" + version comments
#   - Cargo.toml               rust-version = "X.Y" (MSRV)
#   - pgo-runner/Cargo.toml    rust-version = "X.Y" (MSRV)
#   - .github/workflows/*.yml  RUST_VERSION: "X.Y.Z" (CI install pin)
#
# It also audits narrative docs (docs/, README.md, CONTRIBUTING.md)
# for stale version references and warns instead of auto-editing
# them, because release dates and rationale need editorial review.
#
# This script is a thin forwarder: scripts/rust-version-to.sh is the
# single source of truth for the bump contract (validation, sync
# verification, idempotency, dirty-tree guard, summary). Keeping one
# implementation avoids contract drift between two bumpers. All
# arguments pass through unchanged:
#
#   ./scripts/bump-rust-to.sh 1.99.0            bump to 1.99.0
#   ./scripts/bump-rust-to.sh --check 1.98.1     verify everything is at 1.98.1
#   ./scripts/bump-rust-to.sh --allow-dirty 1.99.0   bump on a dirty tree
#   ./scripts/bump-rust-to.sh --help             full usage from the implementation
#
# See scripts/rust-version-to.sh for the contract and
# docs/workflow/ABOUT_CI.md for the versioning policy.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly TARGET="${REPO_ROOT}/scripts/rust-version-to.sh"

if [[ ! -f "${TARGET}" ]]; then
	echo "ERROR: implementation script not found: ${TARGET}"
	exit 1
fi

exec bash "${TARGET}" "$@"
