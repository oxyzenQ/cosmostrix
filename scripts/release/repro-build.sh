#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
# PLATFORM: UNIX-only (Linux). The pinned lane builds the x86_64-unknown-
#   linux-musl target (static, self-contained rust-lld linking).
#
# LOCAL REPRODUCIBLE-BUILD VERIFIER (night-frontier-Z1, Proof Moat 1b)
#
# Proves PROCESS identity on one host: two cold builds of the SAME
# commit, from the SAME checkout path, with the SAME pinned epoch and
# the release lane's exact RUSTFLAGS composition, must produce
# byte-identical binaries. This is the local half of the reproducible
# release chain; the two-host half lives in CI (ci.yml:
# reproducible_build + reproducible_verify, byte-identical across two
# independent runners via cmp).
#
# Why the same checkout path: the release lane redacts ${PWD} and
# ${HOME}/.cargo through --remap-path-prefix, so the embedded path
# strings are constant regardless of the checkout location; building
# twice in place keeps every other input constant too. The CI job is
# the stronger proof (two truly independent hosts).
#
# What is pinned here:
#   - toolchain: rust-toolchain.toml (the repo's single version source)
#   - epoch: SOURCE_DATE_EPOCH = the commit timestamp (git log -1 %ct),
#     the same pin the CI reproducible lane uses
#   - flags: -C target-cpu=x86-64-v3 + redacted path prefixes +
#     -Cforce-frame-pointers=yes (scripts/lib/builds.sh composition)
#   - profile: pro-linux-amd64-v3-musl (static musl, LTO fat, CGU 1)
#   - deps: --locked (Cargo.lock)
#
# Cost: two full optimized builds (LTO fat). This script is opt-in —
# it is NOT wired into check-all; CI runs the two-host proof on every
# push, so a local run is only needed to reproduce a CI failure.
#
# Usage: bash scripts/release/repro-build.sh
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${REPO_ROOT}"

BIN_REL="target/x86_64-unknown-linux-musl/pro-linux-amd64-v3-musl/cosmostrix"
PROFILE_DIR="target/x86_64-unknown-linux-musl/pro-linux-amd64-v3-musl"

if ! command -v cargo >/dev/null 2>&1; then
	if [ -f "${HOME}/.cargo/env" ]; then
		# shellcheck disable=SC1091
		. "${HOME}/.cargo/env"
	else
		echo "error: cargo not found (install rustup first)" >&2
		exit 1
	fi
fi

if ! rustup target list --installed 2>/dev/null | grep -qx 'x86_64-unknown-linux-musl'; then
	echo "error: the pinned lane builds x86_64-unknown-linux-musl, which is not installed"
	echo "  Fix: rustup target add x86_64-unknown-linux-musl"
	exit 1
fi

EPOCH="$(git log -1 --format=%ct)"
export SOURCE_DATE_EPOCH="${EPOCH}"
export COSMOSTRIX_BUILD="linux-x86_64-v3-musl"
export COSMOSTRIX_PROFILE="pro-linux-amd64-v3-musl"
export RUSTFLAGS="-C target-cpu=x86-64-v3 --remap-path-prefix=${HOME}/.cargo=redacted --remap-path-prefix=${PWD}=redacted -Cforce-frame-pointers=yes"
export CARGO_INCREMENTAL=0

echo "repro-build: commit $(git rev-parse --short=7 HEAD), SOURCE_DATE_EPOCH=${EPOCH}"

# Artifacts are staged OUTSIDE target/: the cold-wipe between passes
# removes the profile's build directory, and a stage inside target/
# would be destroyed with it.
STAGE="$(mktemp -d)"
trap 'rm -rf "${STAGE}"' EXIT

build_once() {
	local label="$1"
	local dest="$2"
	echo "repro-build: building ${label} (full LTO-fat cold build, be patient)"
	cargo build --profile pro-linux-amd64-v3-musl --locked --target x86_64-unknown-linux-musl
	mkdir -p "$(dirname "${dest}")"
	cp "${BIN_REL}" "${dest}"
	# Cold wipe of ONLY the pinned lane's artifact directory: the proof
	# is about the PROCESS, so the next build recompiles the whole graph
	# from the lockfile (mirroring the cache-free CI lane) without
	# touching any other profile the machine may have built.
	rm -rf "${PROFILE_DIR}"
}

build_once "host-a (same host, cold pass 1)" "${STAGE}/host-a/cosmostrix"
build_once "host-b (same host, cold pass 2)" "${STAGE}/host-b/cosmostrix"

sha256sum "${STAGE}/host-a/cosmostrix" "${STAGE}/host-b/cosmostrix"
if cmp -s "${STAGE}/host-a/cosmostrix" "${STAGE}/host-b/cosmostrix"; then
	echo "REPRODUCIBLE RELEASE CHAIN (local, same host): PASS — two cold builds are byte-identical"
	echo "  artifact: ${STAGE}/host-a/cosmostrix"
else
	echo "REPRODUCIBLE RELEASE CHAIN (local, same host): FAIL — the two builds differ"
	cmp -l "${STAGE}/host-a/cosmostrix" "${STAGE}/host-b/cosmostrix" | head -20 || true
	exit 1
fi
