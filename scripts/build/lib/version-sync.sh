# shellcheck shell=bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Sourced by
#   scripts/build/build.sh — not a standalone executable (no main, no
#   argument parsing; the entry script owns both).
#
# COSMOSTRIX BUILD MODULE: version_sync
# verification-only version sync: reads Cargo.toml and defers to
# scripts/release/version-to.sh --check. Bumping is owned by the release
# script, never here.
#
# Part of the build.sh split (NIGHT-lts-2): the single-entry orchestrator
# was de-monolithed into sourced modules; function bodies are byte-identical
# moves from the former flat script.

# ── Version sync (verification only) ───────────────────────────────────
# Version bumping is owned by ./scripts/release/version-to.sh — see its --help for
# the full list of files it touches (Cargo.toml, Cargo.lock, PKGBUILD,
# .SRCINFO, README.md, docs/workflow/ABOUT_CI.md). build.sh only exposes
# the `version-sync` subcommand, which verifies all active version refs
# agree with Cargo.toml without writing anything.

# Read the current package version from Cargo.toml (single source of truth).
# Used by `version-sync` to know what to verify against.
read_cargo_version() {
	local cargo_toml="${PWD}/Cargo.toml"
	if [ ! -f "${cargo_toml}" ]; then
		log_error "Cargo.toml not found at ${cargo_toml}"
		return 1
	fi
	local ver
	ver="$(grep -E '^version = "' "${cargo_toml}" | head -1 | sed -E 's/^version = "(.+)"/\1/')"
	if [ -z "${ver}" ]; then
		log_error "Could not extract version from Cargo.toml"
		return 1
	fi
	echo "${ver}"
}

# Verify all active version refs agree with Cargo.toml. No build, no writes.
# Wraps `version-to.sh --check <cargo-version>` for convenience.
run_version_sync() {
	log_step "Verifying version sync across all active files..."

	local current
	current="$(read_cargo_version)" || return 1

	local bumper="${PWD}/scripts/release/version-to.sh"
	if [ ! -x "${bumper}" ]; then
		log_error "Version bumper not found or not executable: ${bumper}"
		return 1
	fi

	if [ "${QUIET_CHECK}" -eq 1 ]; then
		local vsync_output
		vsync_output=$("${bumper}" --check "${current}" 2>&1)
		local vsync_rc=$?
		echo "$vsync_output" | grep -iE '(FAIL|ERROR|mismatch|desync|differ|desync)' || true
		if [ $vsync_rc -eq 0 ]; then
			log_success_quietable "All active version refs agree with Cargo.toml (v${current})"
		else
			log_error "Version desync detected — run './scripts/release/version-to.sh v${current}' to fix"
		fi
		return $vsync_rc
	else
		if "${bumper}" --check "${current}"; then
			log_success "All active version refs agree with Cargo.toml (v${current})"
		else
			log_error "Version desync detected — run './scripts/release/version-to.sh v${current}' to fix"
			return 1
		fi
	fi
}
