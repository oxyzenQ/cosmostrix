#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
#
# ─────────────────────────────────────────────────────────────────────────────
# PLATFORM: UNIX-only (Linux, macOS, BSD).
#   Uses `sed -i`, `grep -q`, `awk`, `git status --porcelain`. Not for
#   Windows cmd.exe / PowerShell — use Git Bash or WSL on Windows.
# ─────────────────────────────────────────────────────────────────────────────
#
# cosmostrix Version Bump Helper
#
# Updates all version references consistently across the repo.
#
# USAGE:
#   ./scripts/dev/version-to.sh <VERSION>              Bump to VERSION (stable or pre-release)
#   ./scripts/dev/version-to.sh <VERSION> --force      Bump, recording a deliberate release-line reset
#   ./scripts/dev/version-to.sh --check <VERSION>     Verify version is VERSION (no changes)
#   ./scripts/dev/version-to.sh --help                Show this help
#
# EXAMPLES:
#   ./scripts/dev/version-to.sh v50.0.0                # Stable release
#   ./scripts/dev/version-to.sh v50.0.0-alpha.2        # Pre-release
#
# Safety:
#   - Refuses to run if git working tree has unrelated changes
#   - Does not commit, tag, or push automatically
#   - Only edits version-related files
#
# Ride-along lane (night-cafe-mclaren-1, the generalized
# night-improve-9 shape): every [package] manifest under the repo
# root other than the primary Cargo.toml rides the root's version -
# pgo-runner, cosmostrix-fuzz, and any crate the owner adds
# tomorrow, wherever it lives. Discovery is dynamic (see
# scripts/lib/ridealong.sh): a hardcoded path list is the exact
# stale-data class this lane exists to close, so a new Cargo.toml
# is adopted by the next bump with no script edit, and its drift
# fails --check with no script edit either. Every bump also records
# the release line in the root CHANGELOG.md ledger: a
# bare vX.Y.Z line under a '## Released' heading above
# '## Unreleased', newest first, pre-release and stable alike. The
# ledger lane runs as a verified pre-flight before any file is
# mutated (the zelynic NIGHT-bump-1 port - the shape, semver and
# verification laws live in scripts/lib/changelog-ledger.sh): a
# release line stranded outside '## Released' heals back inside, a
# stale-tree downgrade refuses unless --force records a deliberate
# reset, and a write that does not verify in place restores the file
# and fails the bump. Both lanes probe honestly: a missing file warns
# and writes nothing.
#

set -euo pipefail

#
# Constants
#

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
readonly REPO_ROOT
readonly CARGO_TOML="${REPO_ROOT}/Cargo.toml"
readonly CARGO_LOCK="${REPO_ROOT}/Cargo.lock"
readonly PKGBUILD="${REPO_ROOT}/aur/cosmostrix-bin/PKGBUILD"
readonly SRCINFO="${REPO_ROOT}/aur/cosmostrix-bin/.SRCINFO"
readonly README="${REPO_ROOT}/README.md"
readonly ABOUT_CI="${REPO_ROOT}/docs/workflow/ABOUT_CI.md"
readonly CHANGELOG="${REPO_ROOT}/CHANGELOG.md"

# Files that contain active version references to update
readonly DOC_FILES=(
	"${README}"
	"${ABOUT_CI}"
)

# Workflow files to audit for hardcoded versions (not auto-updated unless necessary)
readonly WORKFLOW_FILES=(
	"${REPO_ROOT}/.github/workflows/release.yml"
	"${REPO_ROOT}/.github/workflows/aur.yml"
	"${REPO_ROOT}/.github/workflows/ci.yml"
)

#
# Colors
#
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly BLUE='\033[0;34m'
readonly NC='\033[0m'

log_info() { printf '%b[INFO]%b %s\n' "${BLUE}" "${NC}" "$*"; }
log_ok() { printf '%b[OK]%b %s\n' "${GREEN}" "${NC}" "$*"; }
log_warn() { printf '%b[WARN]%b %s\n' "${YELLOW}" "${NC}" "$*"; }
log_err() { printf '%b[ERROR]%b %s\n' "${RED}" "${NC}" "$*" >&2; }

#
# The CHANGELOG.md release-ledger machinery: the verified writer
# pre-flight plus the positional probes shared with --check
# verification and the shape gate. Sourced after the logging
# helpers the writer entry relies on.
#
# shellcheck disable=SC1091 # lib/ modules are analyzed standalone by
# their own shellcheck run
source "${REPO_ROOT}/scripts/lib/changelog-ledger.sh"
source "${REPO_ROOT}/scripts/lib/ridealong.sh"

#
# Usage
#
show_help() {
	cat <<'HELP'
cosmostrix Version Bump Helper

USAGE:
    ./scripts/dev/version-to.sh <VERSION>              Bump to VERSION (stable or pre-release)
    ./scripts/dev/version-to.sh <VERSION> --force      Bump, recording a deliberate release-line reset
    ./scripts/dev/version-to.sh --check <VERSION>      Verify version is VERSION
    ./scripts/dev/version-to.sh --help                 Show this help

EXAMPLES:
    ./scripts/dev/version-to.sh v50.0.0               # Stable release
    ./scripts/dev/version-to.sh v50.0.0-alpha.2       # Pre-release
HELP
}

#
# Version validation
#
validate_version() {
	local ver="$1"

	if [[ -z "${ver}" ]]; then
		log_err "Version argument is required"
		exit 1
	fi

	# Accept and strip optional leading 'v' prefix (e.g. v18.0.0 → 18.0.0).
	# This is a convenience: tags use the 'v' prefix by convention, so
	# users naturally type `./scripts/dev/version-to.sh v18.0.0`. Internally
	# we always store the bare X.Y.Z form in Cargo.toml.
	# NOTE: This function only validates. The caller is responsible for
	# applying the strip to its own TARGET_VERSION variable.
	if [[ "${ver}" == v* ]]; then
		ver="${ver#v}"
	fi

	# Reject pre-release suffixes
	if [[ "${ver}" == *-* ]]; then
		log_err "Pre-release versions are not supported by this script."
		log_err "Got: ${ver}"
		log_err "This script handles stable SemVer only: X.Y.Z"
		exit 1
	fi

	# Must be exactly X.Y.Z with digits
	if ! [[ "${ver}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
		log_err "Invalid version format: ${ver}"
		log_err "Expected stable SemVer: X.Y.Z (e.g. 2.5.0)"
		exit 1
	fi
}

#
# Read current version from Cargo.toml
#
read_current_version() {
	if [[ ! -f "${CARGO_TOML}" ]]; then
		log_err "Cargo.toml not found at ${CARGO_TOML}"
		exit 1
	fi

	local ver
	ver="$(grep -E '^version = "' "${CARGO_TOML}" | head -1 | sed -E 's/^version = "(.+)"/\1/')"

	if [[ -z "${ver}" ]]; then
		log_err "Could not extract version from Cargo.toml"
		exit 1
	fi

	echo "${ver}"
}

#
# Safety: check git working tree
#
check_git_status() {
	if ! git -C "${REPO_ROOT}" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
		log_warn "Not inside a git repository — skipping dirty check"
		return 0
	fi

	local dirty
	dirty="$(git -C "${REPO_ROOT}" status --porcelain 2>/dev/null || true)"

	if [[ -n "${dirty}" ]]; then
		if [[ "${ALLOW_DIRTY}" == "1" ]]; then
			log_warn "Working tree has uncommitted changes (--allow-dirty):"
			git -C "${REPO_ROOT}" status --short
			echo ""
		else
			log_err "Working tree has uncommitted changes. Commit or stash first,"
			log_err "or pass --allow-dirty to proceed anyway."
			echo ""
			git -C "${REPO_ROOT}" status --short
			exit 1
		fi
	fi
}

#
# Update Cargo.toml
#
update_cargo_toml() {
	local old_ver="$1"
	local new_ver="$2"

	log_info "Updating Cargo.toml: ${old_ver} -> ${new_ver}"

	# Use sed to replace only the package version line
	# Match: ^version = "OLD" (the first occurrence under [package])
	sed -i -E "s|^version = \"${old_ver}\"|version = \"${new_ver}\"|" "${CARGO_TOML}"

	# Verify
	local got
	got="$(read_current_version)"
	if [[ "${got}" != "${new_ver}" ]]; then
		log_err "Cargo.toml version update failed. Expected: ${new_ver}, Got: ${got}"
		exit 1
	fi

	log_ok "Cargo.toml updated: version = \"${new_ver}\""
}

#
# Update Cargo.lock
#
update_cargo_lock() {
	local old_ver="$1"
	local new_ver="$2"

	log_info "Refreshing Cargo.lock for cosmostrix ${old_ver} -> ${new_ver}"

	# Cargo.lock is a machine-generated file that tracks exact dependency
	# versions. When we bump the root package version, we only need to update
	# the cosmostrix entry — NOT the dependency entries.
	#
	# cargo generate-lockfile would also update dependency versions, which is
	# undesirable (it's the job of cargo update, not a version bump).
	#
	# Instead, we directly update the version field for the cosmostrix entry
	# in Cargo.lock, then verify that cargo check --locked still passes.

	if [[ ! -f "${CARGO_LOCK}" ]]; then
		log_warn "Cargo.lock not found — skipping lock update"
		return 0
	fi

	# Update the version field that appears right after name = "cosmostrix"
	# The pattern in Cargo.lock is:
	#   [[package]]
	#   name = "cosmostrix"
	#   version = "OLD"
	#
	# We use a multi-line sed to match the name+version pair and update it.
	# This is safe because the cosmostrix package appears exactly once.
	sed -i -E "/^name = \"cosmostrix\"$/{n;s|^version = \"${old_ver}\"|version = \"${new_ver}\"|;}" "${CARGO_LOCK}"

	# Verify
	local lock_ver
	lock_ver="$(grep -A1 '^name = "cosmostrix"' "${CARGO_LOCK}" | grep '^version = "' | head -1 | sed -E 's/^version = "(.+)"/\1/')"

	if [[ "${lock_ver}" == "${new_ver}" ]]; then
		log_ok "Cargo.lock updated: cosmostrix version = ${new_ver}"
	else
		log_err "Cargo.lock update failed. Expected: ${new_ver}, Got: ${lock_ver}"
		log_err "You may need to run 'cargo generate-lockfile' manually"
		# Do not exit — let verification catch this
	fi
}

#
# Generate .SRCINFO from PKGBUILD metadata
#
generate_srcinfo_from_pkgbuild() {
	local pkgbuild_file="$1"
	local srcinfo_file="$2"

	# shellcheck disable=SC2154  # variables from sourced PKGBUILD
	(
		set -euo pipefail
		# shellcheck disable=SC1090
		source "${pkgbuild_file}"

		local pkgbase_value="${pkgbase:-${pkgname}}"

		print_values() {
			local key="$1"
			shift
			local value
			for value in "$@"; do
				printf '\t%s = %s\n' "${key}" "${value}"
			done
		}

		echo "pkgbase = ${pkgbase_value}"
		printf '\tpkgdesc = %s\n' "${pkgdesc}"
		printf '\tpkgver = %s\n' "${pkgver}"
		printf '\tpkgrel = %s\n' "${pkgrel}"
		printf '\turl = %s\n' "${url}"
		print_values "arch" "${arch[@]}"
		print_values "license" "${license[@]}"
		print_values "depends" "${depends[@]}"
		print_values "provides" "${provides[@]}"
		print_values "conflicts" "${conflicts[@]}"
		print_values "options" "${options[@]}"
		echo ""
		echo "pkgname = ${pkgname}"
	) >"${srcinfo_file}"
}

#
# Update PKGBUILD
#
update_pkgbuild() {
	local old_ver="$1"
	local new_ver="$2"

	log_info "Updating PKGBUILD: pkgver=${old_ver} -> ${new_ver}, _tag= (tag derived from pkgver)"

	# Update pkgver — includes the full version (stable or pre-release).
	# The PKGBUILD prepare() function constructs the download tag as v${pkgver}
	# when _tag is empty, so pre-release versions like 50.0.0-alpha.2 work
	# correctly with _tag= (tag becomes v50.0.0-alpha.2).
	sed -i -E "s|^pkgver=.*|pkgver=${new_ver}|" "${PKGBUILD}"

	# Ensure _tag is empty — the download tag is always v${pkgver}
	sed -i -E 's|^_tag=.*|_tag=|' "${PKGBUILD}"

	# Verify
	local got_pkgver
	got_pkgver="$(grep '^pkgver=' "${PKGBUILD}" | head -1 | sed 's/^pkgver=//')"
	local got_tag
	got_tag="$(grep '^_tag=' "${PKGBUILD}" | head -1 | sed 's/^_tag=//')"

	if [[ "${got_pkgver}" != "${new_ver}" ]]; then
		log_err "PKGBUILD pkgver update failed. Expected: ${new_ver}, Got: ${got_pkgver}"
		exit 1
	fi

	if [[ -n "${got_tag}" ]]; then
		log_err "PKGBUILD _tag should be empty for stable, got: ${got_tag}"
		exit 1
	fi

	log_ok "PKGBUILD updated: pkgver=${new_ver}, _tag="

	# .SRCINFO handling
	if [[ -f "${SRCINFO}" ]]; then
		log_info "Regenerating .SRCINFO from PKGBUILD"
		generate_srcinfo_from_pkgbuild "${PKGBUILD}" "${SRCINFO}"
		local got_srcinfo_pkgver
		got_srcinfo_pkgver="$(grep -E '^[[:space:]]*pkgver = ' "${SRCINFO}" | head -1 | sed -E 's/^[[:space:]]*pkgver = //')"
		if [[ "${got_srcinfo_pkgver}" != "${new_ver}" ]]; then
			log_err ".SRCINFO pkgver update failed. Expected: ${new_ver}, Got: ${got_srcinfo_pkgver:-<missing>}"
			exit 1
		fi
		log_ok ".SRCINFO regenerated from PKGBUILD"
	else
		log_info ".SRCINFO not tracked locally — it is generated by the AUR sync workflow"
	fi
}

#
# Update docs/examples
#
update_docs() {
	local old_ver="$1"
	local new_ver="$2"

	# Strict-versioning policy: the ONLY active version reference in any
	# doc file is the single `TAG="vX.Y.Z"` line in the README install
	# snippet. Every other version-shaped string in the docs is either
	# historical narrative (e.g. "variants were dropped in an earlier
	# release") or an illustrative example (`docs/workflow/ABOUT_CI.md`
	# shows `git tag v4.0.0` as a teaching example, not the current
	# release). Those must NOT be touched on a version bump.
	#
	# This function therefore replaces ONLY the literal TAG="v<OLD>" →
	# TAG="v<NEW>" substring. Anything else is left alone.
	local old_tag="TAG=\"v${old_ver}\""
	local new_tag="TAG=\"v${new_ver}\""

	for f in "${DOC_FILES[@]}"; do
		if [[ ! -f "${f}" ]]; then
			log_warn "Doc file not found: ${f}"
			continue
		fi

		log_info "Updating TAG= install line in $(basename "${f}")"

		if grep -qF "${old_tag}" "${f}"; then
			sed -i "s|${old_tag}|${new_tag}|g" "${f}"
			log_ok "  Updated $(basename "${f}"): ${old_tag} → ${new_tag}"
		else
			# Expected for doc files that have no install snippet
			# (e.g. ABOUT_CI.md). Not an error — silently skipped.
			log_info "  No TAG= line in $(basename "${f}") (skipped)"
		fi
	done
}

#
# Audit workflow files for hardcoded versions
#
audit_workflows() {
	local new_ver="$1"

	log_info "Auditing workflow files for hardcoded version references..."

	for f in "${WORKFLOW_FILES[@]}"; do
		if [[ ! -f "${f}" ]]; then
			continue
		fi

		# Check for any version-specific hardcoded references that look like
		# they should be updated (not just comment examples)
		local name
		name="$(basename "${f}")"

		# Workflow files use dynamic tag detection from GITHUB_REF_NAME,
		# so hardcoded versions should only appear in comments/examples.
		# We check but do NOT auto-update workflow files — they should
		# derive versions dynamically.
		local refs
		refs="$(grep -nF "${new_ver}" "${f}" 2>/dev/null | grep -v '#' || true)"

		if [[ -n "${refs}" ]]; then
			log_warn "  ${name} has non-comment references to ${new_ver}:"
			echo "${refs}" | while IFS= read -r line; do
				log_warn "    ${line}"
			done
		fi
	done

	log_ok "Workflow audit complete (workflows use dynamic version detection)"
}

#
# Verification
#
verify_version() {
	local expected_ver="$1"
	local errors=0

	echo ""
	log_info "=== Verification ==="
	echo ""

	# 1. Cargo.toml
	local cargo_ver
	cargo_ver="$(read_current_version)"
	if [[ "${cargo_ver}" == "${expected_ver}" ]]; then
		log_ok "Cargo.toml: version = \"${expected_ver}\""
	else
		log_err "Cargo.toml: expected ${expected_ver}, got ${cargo_ver}"
		errors=$((errors + 1))
	fi

	# 2. Cargo.lock
	if [[ -f "${CARGO_LOCK}" ]]; then
		local lock_ver
		lock_ver="$(grep -A1 '^name = "cosmostrix"' "${CARGO_LOCK}" | grep '^version = "' | head -1 | sed -E 's/^version = "(.+)"/\1/')"
		if [[ "${lock_ver}" == "${expected_ver}" ]]; then
			log_ok "Cargo.lock: cosmostrix version = ${expected_ver}"
		else
			log_err "Cargo.lock: expected ${expected_ver}, got ${lock_ver}"
			errors=$((errors + 1))
		fi
	fi

	# 3. PKGBUILD
	local pkg_ver
	pkg_ver="$(grep '^pkgver=' "${PKGBUILD}" | head -1 | sed 's/^pkgver=//')"
	local pkg_tag
	pkg_tag="$(grep '^_tag=' "${PKGBUILD}" | head -1 | sed 's/^_tag=//')"
	if [[ "${expected_ver}" == *-* ]]; then
		# night-cafe-5: pre-release versions never sync to the AUR —
		# aur.yml rejects any hyphenated tag (AUR only accepts stable
		# packages) and pacman forbids hyphens in pkgver (the hyphen is
		# the pkgver-pkgrel separator). The PKGBUILD keeps the last
		# stable pkgver; assert validity, not equality.
		if [[ "${pkg_ver}" =~ ^[0-9.]+$ ]]; then
			log_ok "PKGBUILD: pkgver=${pkg_ver} (stable base, untouched — pre-release ${expected_ver} never syncs to AUR)"
		else
			log_err "PKGBUILD: pkgver=${pkg_ver} carries a hyphen — pacman forbids it; the AUR files must keep the last stable pkgver"
			errors=$((errors + 1))
		fi
	elif [[ "${pkg_ver}" == "${expected_ver}" ]]; then
		log_ok "PKGBUILD: pkgver=${expected_ver}"
	else
		log_err "PKGBUILD: expected pkgver=${expected_ver}, got ${pkg_ver}"
		errors=$((errors + 1))
	fi
	if [[ -z "${pkg_tag}" ]]; then
		log_ok "PKGBUILD: _tag= (empty, tag=v${pkg_ver})"
	else
		log_ok "PKGBUILD: _tag=${pkg_tag} (tag=v${pkg_ver}-${pkg_tag})"
	fi

	# 4. .SRCINFO
	if [[ -f "${SRCINFO}" ]]; then
		local srcinfo_pkgver
		srcinfo_pkgver="$(grep -E '^[[:space:]]*pkgver = ' "${SRCINFO}" | head -1 | sed -E 's/^[[:space:]]*pkgver = //')"
		if [[ "${expected_ver}" == *-* ]]; then
			if [[ "${srcinfo_pkgver}" =~ ^[0-9.]+$ ]]; then
				log_ok ".SRCINFO: pkgver = ${srcinfo_pkgver} (stable base, untouched — pre-release never syncs to AUR)"
			else
				log_err ".SRCINFO: pkgver = ${srcinfo_pkgver:-<missing>} carries a hyphen — pacman forbids it"
				errors=$((errors + 1))
			fi
		elif [[ "${srcinfo_pkgver}" == "${expected_ver}" ]]; then
			log_ok ".SRCINFO: pkgver = ${expected_ver}"
		else
			log_err ".SRCINFO: expected pkgver = ${expected_ver}, got ${srcinfo_pkgver:-<missing>}"
			errors=$((errors + 1))
		fi

		local expected_srcinfo
		expected_srcinfo="$(mktemp)"
		generate_srcinfo_from_pkgbuild "${PKGBUILD}" "${expected_srcinfo}"
		if diff -u "${expected_srcinfo}" "${SRCINFO}" >/dev/null; then
			log_ok ".SRCINFO: metadata matches PKGBUILD"
		else
			log_err ".SRCINFO: metadata differs from PKGBUILD"
			diff -u "${expected_srcinfo}" "${SRCINFO}" >&2 || true
			errors=$((errors + 1))
		fi
		rm -f "${expected_srcinfo}"
	fi

	# 5. cargo metadata
	if command -v cargo >/dev/null 2>&1; then
		local meta_ver
		meta_ver="$(cargo metadata --no-deps --format-version 1 2>/dev/null | grep -o '"version":"[^"]*"' | head -1 | sed 's/"version":"//;s/"//')"
		if [[ "${meta_ver}" == "${expected_ver}" ]]; then
			log_ok "cargo metadata: package version = ${expected_ver}"
		else
			log_err "cargo metadata: expected ${expected_ver}, got ${meta_ver}"
			errors=$((errors + 1))
		fi
	fi

	# 6. README install example TAG="vX.Y.Z"
	# The README install snippet uses TAG="v<VERSION>" — this is the ONLY
	# active version reference in any doc file (strict-versioning policy).
	# It must agree with Cargo.toml. Without this check, a README-only
	# desync would silently pass `version-to.sh --check` and only be
	# caught later by the Rust test suite (docs_tests::metadata).
	#
	# Other doc files (ABOUT_CI.md) intentionally do NOT contain the
	# current version — they use illustrative examples (`git tag v4.0.0`)
	# that are not meant to track the release version. Step 7 below used
	# to require every doc file to reference vX.Y.Z at least once; that
	# was removed when the strict-versioning policy made TAG the single
	# source of truth.
	if [[ -f "${README}" ]]; then
		local expected_tag="TAG=\"v${expected_ver}\""
		if grep -qF "${expected_tag}" "${README}"; then
			log_ok "README.md: ${expected_tag}"
		else
			log_err "README.md: missing or stale install tag (expected ${expected_tag})"
			log_err "  Run './scripts/dev/version-to.sh v${expected_ver}' to sync all active files"
			errors=$((errors + 1))
		fi
	fi

	# 7. Ride-along crates (night-cafe-mclaren-1): every [package]
	# manifest under the repo root other than the primary rides the
	# root version, discovered at run time - pgo-runner,
	# cosmostrix-fuzz, and any future crate. There is no birth-version
	# courtesy: a drifting 0.0.0 or 0.1.0 label shipping beside a
	# v100 binary is the provenance hole the lane exists to close,
	# so any other value is real drift and the next version-to.sh run
	# adopts it. Each lock must agree with its manifest. The lib
	# verifier returns the failed-check count; add it to the tally.
	local ride_errs=0
	verify_ridealong_crates "${expected_ver}" || ride_errs=$?
	errors=$((errors + ride_errs))

	# 9. CHANGELOG.md release ledger (night-improve-9, the zelynic
	# NIGHT-bump-1 verification law): the current version line must sit
	# INSIDE the '## Released' section - presence anywhere else in the
	# file (notably stranded below '## Unreleased') is the fatal class
	# this check exists to catch, so a whole-file grep would pass a
	# broken ledger. The shape probes mirror the writer: exactly one of
	# each heading, Released above Unreleased, the blank spacer under
	# the heading. A foreign-shaped ledger is reported, not failed - a
	# ledger this script did not create is not one it verifies either
	# (the shape gate owns that refusal).
	if [[ -f "${CHANGELOG}" ]]; then
		local ledger_rc ledger_uc ledger_inside
		ledger_rc="$(grep -c '^## Released$' "${CHANGELOG}" || true)"
		ledger_uc="$(grep -c '^## Unreleased$' "${CHANGELOG}" || true)"
		if [ "${ledger_rc}" -eq 0 ]; then
			log_info "CHANGELOG.md release ledger not started yet (## Released is created on the next version bump)"
		elif [ "${ledger_rc}" -eq 1 ] && [ "${ledger_uc}" -eq 1 ] && ledger_order_ok "${CHANGELOG}" && ledger_spacer_blank "${CHANGELOG}"; then
			ledger_inside="$(ledger_released_count "${CHANGELOG}" "v${expected_ver}")"
			if [ "${ledger_inside}" -eq 1 ]; then
				log_ok "CHANGELOG.md: release line v${expected_ver} recorded inside ## Released"
			else
				log_err "CHANGELOG.md: ## Released carries no v${expected_ver} line inside the section"
				log_err "  Run './scripts/dev/version-to.sh v${expected_ver}' to record it"
				errors=$((errors + 1))
			fi
		else
			log_info "CHANGELOG.md ## Released shape is not this tool's (heading count, order, or spacer off) - ledger verification skipped (the shape gate owns the refusal)"
		fi
	fi

	echo ""
	if [[ "${errors}" -eq 0 ]]; then
		log_ok "All verification checks passed"
	else
		log_err "${errors} verification check(s) failed"
		return 1
	fi
}

#
# Print summary
#
print_summary() {
	local old_ver="$1"
	local new_ver="$2"
	shift 2
	local changed_files=("$@")

	echo ""
	echo "=========================================="
	echo " Version bumped"
	echo "=========================================="
	echo "  old: ${old_ver} / v${old_ver}"
	echo "  new: ${new_ver} / v${new_ver}"
	echo ""
	echo "  Files changed:"
	for f in "${changed_files[@]}"; do
		echo "    - ${f}"
	done
	echo ""
	echo "Next:"
	echo "  cargo fmt --all"
	echo "  cargo test --all --locked"
	echo "  cargo clippy --locked --all-targets --all-features -- -D warnings"
	echo "  cargo pro-linux-amd64-v3-musl"
	echo "  target/x86_64-unknown-linux-musl/pro-linux-amd64-v3-musl/cosmostrix --doctor"
	echo "  git diff"
	echo "  git commit -m \"chore: bump version to v${new_ver}\""
	echo "  git tag v${new_ver}"
	echo "  git push origin main v${new_ver}"
	echo "=========================================="
}

#
# Pre-release version validation
#
# Accepts: X.Y.Z-<suffix> — any hyphen after the patch makes the
#         version a pre-release (single classification law shared with
#         release.yml; no channel enumeration, e.g. 50.0.0-alpha.2,
#         50.0.0-nightly.1, 50.0.0-example.1)
# Rejects: anything else (including stable X.Y.Z — use validate_version for that)
validate_prerelease_version() {
	local ver="$1"
	if [[ -z "${ver}" ]]; then
		log_err "Pre-release version argument is required"
		exit 1
	fi
	# Strip optional 'v' prefix
	if [[ "${ver}" == v* ]]; then
		ver="${ver#v}"
	fi
	if ! [[ "${ver}" =~ ^[0-9]+\.[0-9]+\.[0-9]+-[A-Za-z0-9.]+$ ]]; then
		log_err "Invalid pre-release version: ${ver}"
		log_err "Expected: X.Y.Z-<suffix> (e.g. 50.0.0-alpha.2, 50.0.0-nightly.1)"
		exit 1
	fi
}

#
# Main
#
main() {
	local CHECK_MODE=0
	local ALLOW_DIRTY=0
	local LEDGER_FORCE=0
	local TARGET_VERSION=""

	# Parse arguments
	while [[ $# -gt 0 ]]; do
		case "$1" in
		--help | -h)
			show_help
			exit 0
			;;
		--check)
			CHECK_MODE=1
			shift
			;;
		--allow-dirty)
			ALLOW_DIRTY=1
			shift
			;;
		--force)
			LEDGER_FORCE=1
			shift
			;;
		-*)
			log_err "Unknown option: $1"
			show_help
			exit 1
			;;
		*)
			if [[ -n "${TARGET_VERSION}" ]]; then
				log_err "Multiple version arguments provided: ${TARGET_VERSION} and $1"
				exit 1
			fi
			TARGET_VERSION="$1"
			shift
			;;
		esac
	done

	if [[ -z "${TARGET_VERSION}" ]]; then
		log_err "Version argument is required"
		echo ""
		show_help
		exit 1
	fi

	# Strip optional 'v' prefix BEFORE validation (e.g. v18.0.0 → 18.0.0).
	# This keeps the rest of the script dealing with the bare X.Y.Z form.
	if [[ "${TARGET_VERSION}" == v* ]]; then
		log_info "Stripped 'v' prefix — using ${TARGET_VERSION#v}"
		TARGET_VERSION="${TARGET_VERSION#v}"
	fi

	# Validate version format
	# Pre-release versions (X.Y.Z-channel.N) use a separate validator.
	if [[ "${TARGET_VERSION}" == *-* ]]; then
		validate_prerelease_version "${TARGET_VERSION}"
	else
		validate_version "${TARGET_VERSION}"
	fi

	local NEW_VER="${TARGET_VERSION}"
	local NEW_TAG="v${NEW_VER}"

	# Read current version
	local OLD_VER
	OLD_VER="$(read_current_version)"
	local OLD_TAG="v${OLD_VER}"

	log_info "Current version: ${OLD_VER} / ${OLD_TAG}"
	log_info "Target version:  ${NEW_VER} / ${NEW_TAG}"
	echo ""

	# Idempotent check — auto-sync if Cargo.toml matches but other files don't
	if [[ "${OLD_VER}" == "${NEW_VER}" ]]; then
		if verify_version "${NEW_VER}" 2>/dev/null; then
			log_info "Already at version ${NEW_VER} — all files consistent"
			exit 0
		fi
		log_info "Cargo.toml at ${NEW_VER} but other files out of sync — auto-syncing"
		echo ""
	fi

	# Check mode
	if [[ "${CHECK_MODE}" -eq 1 ]]; then
		log_info "Check mode: verifying current version matches ${NEW_VER}"
		if [[ "${OLD_VER}" != "${NEW_VER}" ]]; then
			log_err "Version mismatch: current=${OLD_VER}, expected=${NEW_VER}"
			exit 1
		fi
		# verify_version returns 0 on full sync, 1 on any desync (PKGBUILD,
		# .SRCINFO, README TAG, doc refs, etc.). Propagate its exit code so
		# CI fails fast on partial desyncs that match Cargo.toml but miss
		# other files.
		if verify_version "${NEW_VER}"; then
			exit 0
		else
			exit 1
		fi
	fi

	# Safety: check git working tree
	check_git_status

	# Track changed files
	local changed_files=()

	# 0. The CHANGELOG ledger pre-flight (night-improve-9, the zelynic
	# NIGHT-bump-1 port): the shape is checked and the release line
	# spliced before any other file is mutated - a downgrade refusal
	# fires before Cargo.toml moves, and the splice verifies itself in
	# place (see scripts/lib/changelog-ledger.sh for the three laws).
	changelog_ledger_preflight "${NEW_VER}" "${LEDGER_FORCE}"
	if [[ -f "${CHANGELOG}" ]] && ! git -C "${REPO_ROOT}" diff --quiet -- "${CHANGELOG}"; then
		changed_files+=("CHANGELOG.md")
	fi

	# 1. Update Cargo.toml
	update_cargo_toml "${OLD_VER}" "${NEW_VER}"
	changed_files+=("Cargo.toml")

	# 2. Update Cargo.lock
	update_cargo_lock "${OLD_VER}" "${NEW_VER}"
	changed_files+=("Cargo.lock")

	# 3. Update PKGBUILD (night-cafe-5: pre-releases skip the AUR —
	# aur.yml rejects hyphenated tags, pacman pkgver forbids hyphens;
	# the files keep the last stable pkgver until the next STABLE bump)
	if [[ "${NEW_VER}" == *-* ]]; then
		log_info "AUR untouched: ${NEW_VER} is a pre-release — aur.yml rejects hyphenated tags (AUR only accepts stable); PKGBUILD keeps its current stable pkgver"
	else
		update_pkgbuild "${OLD_VER}" "${NEW_VER}"
		changed_files+=("aur/cosmostrix-bin/PKGBUILD")

		# .SRCINFO
		if [[ -f "${SRCINFO}" ]]; then
			changed_files+=("aur/cosmostrix-bin/.SRCINFO")
		fi
	fi

	# 4. Update docs/examples (only the TAG= line is touched under the
	# strict-versioning policy). Only report a doc file as "changed" if
	# update_docs actually modified it — ABOUT_CI.md has no TAG= line
	# and is left untouched, so it should not appear in the summary.
	update_docs "${OLD_VER}" "${NEW_VER}"
	for f in "${DOC_FILES[@]}"; do
		if [[ -f "${f}" ]] && ! git -C "${REPO_ROOT}" diff --quiet -- "${f}"; then
			changed_files+=("$(basename "${f}")")
		fi
	done

	# 4b. Adopt every ride-along crate onto the root version
	# (night-cafe-mclaren-1): discovery is dynamic, so a new Cargo.toml
	# anywhere under the repo root is adopted by this bump with no
	# script edit. A manifest only counts as changed when the write
	# landed; a crate with no lock has nothing to report.
	update_ridealong_crates "${NEW_VER}"
	local ride_manifest ride_lock
	while IFS= read -r ride_manifest; do
		[[ -z "${ride_manifest}" ]] && continue
		if ! git -C "${REPO_ROOT}" diff --quiet -- "${ride_manifest}"; then
			changed_files+=("${ride_manifest#"${REPO_ROOT}"/}")
		fi
		ride_lock="${ride_manifest%/Cargo.toml}/Cargo.lock"
		if [[ -f "${ride_lock}" ]] && ! git -C "${REPO_ROOT}" diff --quiet -- "${ride_lock}"; then
			changed_files+=("${ride_lock#"${REPO_ROOT}"/}")
		fi
	done < <(ridealong_manifests)

	# 5. Audit workflows
	audit_workflows "${NEW_VER}"

	# 6. Run verification
	verify_version "${NEW_VER}"

	# 7. Print summary
	print_summary "${OLD_VER}" "${NEW_VER}" "${changed_files[@]}"
}

main "$@"
