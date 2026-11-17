#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).
#
#
# COSMOSTRIX WORKSPACE-WIDE DEPENDENCY REFRESH
#
# The single canonical walk over every committed Cargo.toml in the
# repository (night-zen-1 law, owner directive: "any Cargo.toml
# anywhere in the project must ride the weekly dependency
# maintenance lane - never skipped, never manually registered").
# The walk is position-independent: a manifest at the repo root, in
# a subfolder, or in a test/audit scratch crate added tomorrow is
# detected by the same `find` rule with zero maintenance.
#
# This script is the ONLY place that defines the manifest walk. Both
# phases of the maintenance workflow consume it:
#   - validate: runs the refresh (default mode)
#   - commit:   derives the expected lockfile set (--list mode)
# so the two walks can never drift apart.
#
# Usage:
#   bash scripts/ci/update-all-workspaces.sh           # refresh locks
#   bash scripts/ci/update-all-workspaces.sh --list    # print manifests
#
# Modes:
#   --list  Print every detected Cargo.toml as a repo-relative path,
#           one per line, sorted. No cargo invocation, no writes.
#   default For each detected manifest: cd into its directory and run
#           `cargo update` so each crate's Cargo.lock rides its own
#           dependency graph.
#
# Exclusions: target/ trees (cache-restored build output must never
# join the walk) and .git internals.

set -euo pipefail

LIST_ONLY=false
if [[ "${1:-}" == "--list" ]]; then
	LIST_ONLY=true
elif [[ $# -gt 0 ]]; then
	echo "Usage: $0 [--list]" >&2
	exit 2
fi

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "${REPO_ROOT}"

# Sorted for deterministic output: same manifest set on every runner,
# every run, so logs and the derived lockfile set are reproducible.
MANIFESTS="$(find . -name Cargo.toml -not -path '*/target/*' -not -path './.git/*' | sort)"

if [[ -z "${MANIFESTS}" ]]; then
	echo "No Cargo.toml found in the repository" >&2
	exit 1
fi

if ${LIST_ONLY}; then
	# Strip the leading ./ so the paths match `git diff --name-only`
	# output exactly (the commit phase diffs against that form).
	sed 's|^\./||' <<< "${MANIFESTS}"
	exit 0
fi

while IFS= read -r manifest; do
	dir="$(dirname "${manifest}")"
	echo "Refreshing ${dir}"
	(
		cd "${dir}"
		cargo update
	)
done <<< "${MANIFESTS}"
