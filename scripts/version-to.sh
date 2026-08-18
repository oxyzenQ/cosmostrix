#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# ─────────────────────────────────────────────────────────────────────────────
# PLATFORM: UNIX-only (Linux, macOS, BSD).
#   Uses `sed -i`, `grep -q`, `awk`, `git status --porcelain`. Not for
#   Windows cmd.exe / PowerShell — use Git Bash or WSL on Windows.
# ─────────────────────────────────────────────────────────────────────────────
#
# Cosmostrix Version Bump Helper
#
# Updates all version references consistently across the repo.
#
# USAGE:
#   ./scripts/version-to.sh <VERSION>              Bump to VERSION (stable or pre-release)
#   ./scripts/version-to.sh --check <VERSION>     Verify version is VERSION (no changes)
#   ./scripts/version-to.sh --help                Show this help
#
# EXAMPLES:
#   ./scripts/version-to.sh v50.0.0                # Stable release
#   ./scripts/version-to.sh v50.0.0-alpha.2        # Pre-release
#
# Safety:
#   - Refuses to run if git working tree has unrelated changes
#   - Does not commit, tag, or push automatically
#   - Only edits version-related files
#

set -euo pipefail

#
# Constants
#

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
readonly REPO_ROOT
readonly CARGO_TOML="${REPO_ROOT}/Cargo.toml"
readonly CARGO_LOCK="${REPO_ROOT}/Cargo.lock"
readonly PKGBUILD="${REPO_ROOT}/aur/cosmostrix-bin/PKGBUILD"
readonly SRCINFO="${REPO_ROOT}/aur/cosmostrix-bin/.SRCINFO"
readonly README="${REPO_ROOT}/README.md"
readonly ABOUT_CI="${REPO_ROOT}/docs/workflow/ABOUT_CI.md"

readonly ASSETS_DIR="${REPO_ROOT}/assets"

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

log_info()  { printf '%b[INFO]%b %s\n'  "${BLUE}" "${NC}" "$*"; }
log_ok()    { printf '%b[OK]%b %s\n'    "${GREEN}" "${NC}" "$*"; }
log_warn()  { printf '%b[WARN]%b %s\n'  "${YELLOW}" "${NC}" "$*"; }
log_err()   { printf '%b[ERROR]%b %s\n' "${RED}" "${NC}" "$*" >&2; }

#
# Usage
#
show_help() {
    cat <<'HELP'
Cosmostrix Version Bump Helper

USAGE:
    ./scripts/version-to.sh <VERSION>              Bump to VERSION (stable or pre-release)
    ./scripts/version-to.sh --check <VERSION>      Verify version is VERSION
    ./scripts/version-to.sh --help                 Show this help

EXAMPLES:
    ./scripts/version-to.sh v50.0.0               # Stable release
    ./scripts/version-to.sh v50.0.0-alpha.2       # Pre-release
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
    # users naturally type `./scripts/version-to.sh v18.0.0`. Internally
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
    ) > "${srcinfo_file}"
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
# Rename demo assets to match the new major version
#
# Demo assets follow the naming convention:
#   cosmostrix-v{MAJOR}-demo.gif
#   cosmostrix-v{MAJOR}-demo-{variant}.png
#
# When the major version changes (e.g. v20 → v25), this function
# renames all matching asset files using `git mv` so the history
# is preserved. If the major version stays the same (e.g. 20.0.0 → 20.1.0),
# no renaming is needed — the assets already have the correct prefix.
#
update_assets() {
    local old_ver="$1"
    local new_ver="$2"

    local old_major
    old_major="$(echo "${old_ver}" | sed -E 's/^([0-9]+)\..*/\1/')"
    local new_major
    new_major="$(echo "${new_ver}" | sed -E 's/^([0-9]+)\..*/\1/')"

    # Only rename when the major version changes
    if [[ "${old_major}" == "${new_major}" ]]; then
        log_info "Major version unchanged (v${old_major} → v${new_major}) — no asset rename needed"
        return 0
    fi

    local old_prefix="cosmostrix-v${old_major}-demo"
    local new_prefix="cosmostrix-v${new_major}-demo"

    if [[ ! -d "${ASSETS_DIR}" ]]; then
        log_warn "assets/ directory not found — skipping asset rename"
        return 0
    fi

    log_info "Renaming demo assets: ${old_prefix} → ${new_prefix}"

    local renamed=0
    for old_file in "${ASSETS_DIR}/${old_prefix}"*; do
        if [[ ! -e "${old_file}" ]]; then
            continue
        fi

        local basename
        basename="$(basename "${old_file}")"
        local new_basename="${basename/${old_prefix}/${new_prefix}}"
        local new_file="${ASSETS_DIR}/${new_basename}"

        if [[ -e "${new_file}" ]]; then
            log_warn "  Target already exists: ${new_basename} — skipping"
            continue
        fi

        git -C "${REPO_ROOT}" mv "${old_file}" "${new_file}"
        renamed=$((renamed + 1))
    done

    if [[ "${renamed}" -eq 0 ]]; then
        log_warn "No demo assets matching ${old_prefix}* found in assets/"
        return 0
    fi

    log_ok "Renamed ${renamed} demo asset(s) (v${old_major} → v${new_major})"
}

#
# Update README demo image references
#
# README.md contains <img> tags referencing demo assets:
#   <img src="assets/cosmostrix-v{MAJOR}-demo.gif" alt="cosmostrix v{MAJOR} demo" ...>
#   <img src="assets/cosmostrix-v{MAJOR}-demo-{variant}.png" alt="cosmostrix v{MAJOR} ..." ...>
#
# This function updates both the src path and alt text to use the new
# major version prefix. It is separate from update_docs() because the
# demo asset naming convention is "v{MAJOR}" (major only), not the full
# version — update_docs() only replaces the single TAG="vX.Y.Z" line
# (strict-versioning policy) and does not touch demo asset filenames.
#
# Under the strict-versioning policy, this function is the ONLY place
# that touches README.md content other than the TAG line bumped by
# update_docs(). It only fires on a major-version change; same-major
# bumps (e.g. 25.0.0 → 25.1.0) leave the demo refs untouched.
#
update_readme_demo_refs() {
    local old_ver="$1"
    local new_ver="$2"

    local old_major
    old_major="$(echo "${old_ver}" | sed -E 's/^([0-9]+)\..*/\1/')"
    local new_major
    new_major="$(echo "${new_ver}" | sed -E 's/^([0-9]+)\..*/\1/')"

    # Only update when the major version changes (same logic as update_assets)
    if [[ "${old_major}" == "${new_major}" ]]; then
        log_info "Major version unchanged — no README demo ref update needed"
        return 0
    fi

    if [[ ! -f "${README}" ]]; then
        log_warn "README.md not found — skipping demo reference update"
        return 0
    fi

    log_info "Updating README demo refs: v${old_major} → v${new_major}"

    # Replace asset path references: cosmostrix-vOLD_MAJOR-demo → cosmostrix-vNEW_MAJOR-demo
    sed -i -E "s|cosmostrix-v${old_major}-demo|cosmostrix-v${new_major}-demo|g" "${README}"

    # Replace alt text: "cosmostrix vOLD_MAJOR" → "cosmostrix vNEW_MAJOR"
    # This covers patterns like: alt="cosmostrix v20 demo", alt="cosmostrix v20 binary charset demo"
    sed -i -E "s|cosmostrix v${old_major}|cosmostrix v${new_major}|g" "${README}"

    log_ok "README demo refs updated (v${old_major} → v${new_major})"
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
        ((errors++))
    fi

    # 2. Cargo.lock
    if [[ -f "${CARGO_LOCK}" ]]; then
        local lock_ver
        lock_ver="$(grep -A1 '^name = "cosmostrix"' "${CARGO_LOCK}" | grep '^version = "' | head -1 | sed -E 's/^version = "(.+)"/\1/')"
        if [[ "${lock_ver}" == "${expected_ver}" ]]; then
            log_ok "Cargo.lock: cosmostrix version = ${expected_ver}"
        else
            log_err "Cargo.lock: expected ${expected_ver}, got ${lock_ver}"
            ((errors++))
        fi
    fi

    # 3. PKGBUILD
    local pkg_ver
    pkg_ver="$(grep '^pkgver=' "${PKGBUILD}" | head -1 | sed 's/^pkgver=//')"
    local pkg_tag
    pkg_tag="$(grep '^_tag=' "${PKGBUILD}" | head -1 | sed 's/^_tag=//')"
    if [[ "${pkg_ver}" == "${expected_ver}" ]]; then
        log_ok "PKGBUILD: pkgver=${expected_ver}"
    else
        log_err "PKGBUILD: expected pkgver=${expected_ver}, got ${pkg_ver}"
        ((errors++))
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
        if [[ "${srcinfo_pkgver}" == "${expected_ver}" ]]; then
            log_ok ".SRCINFO: pkgver = ${expected_ver}"
        else
            log_err ".SRCINFO: expected pkgver = ${expected_ver}, got ${srcinfo_pkgver:-<missing>}"
            ((errors++))
        fi

        local expected_srcinfo
        expected_srcinfo="$(mktemp)"
        generate_srcinfo_from_pkgbuild "${PKGBUILD}" "${expected_srcinfo}"
        if diff -u "${expected_srcinfo}" "${SRCINFO}" >/dev/null; then
            log_ok ".SRCINFO: metadata matches PKGBUILD"
        else
            log_err ".SRCINFO: metadata differs from PKGBUILD"
            diff -u "${expected_srcinfo}" "${SRCINFO}" >&2 || true
            ((errors++))
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
            ((errors++))
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
            log_err "  Run './scripts/version-to.sh v${expected_ver}' to sync all active files"
            ((errors++))
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
    echo "  cargo pro-linux-v3"
    echo "  target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix --doctor"
    echo "  git diff"
    echo "  git commit -m \"chore: bump version to v${new_ver}\""
    echo "  git tag v${new_ver}"
    echo "  git push origin main v${new_ver}"
    echo "=========================================="
}

#
# Pre-release version validation
#
# Accepts: X.Y.Z-alpha.N, X.Y.Z-beta.N, X.Y.Z-rc.N, X.Y.Z-pre.N, X.Y.Z-nightly.N
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
    if ! [[ "${ver}" =~ ^[0-9]+\.[0-9]+\.[0-9]+-(alpha|beta|rc|pre|nightly)\.[0-9]+$ ]]; then
        log_err "Invalid pre-release version: ${ver}"
        log_err "Expected: X.Y.Z-{alpha|beta|rc|pre|nightly}.N (e.g. 50.0.0-alpha.2, 50.0.0-nightly.1)"
        exit 1
    fi
}

#
# Main
#
main() {
    local CHECK_MODE=0
    local ALLOW_DIRTY=0
    local TARGET_VERSION=""

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --help|-h)
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

    # 1. Update Cargo.toml
    update_cargo_toml "${OLD_VER}" "${NEW_VER}"
    changed_files+=("Cargo.toml")

    # 2. Update Cargo.lock
    update_cargo_lock "${OLD_VER}" "${NEW_VER}"
    changed_files+=("Cargo.lock")

    # 3. Update PKGBUILD
    update_pkgbuild "${OLD_VER}" "${NEW_VER}"
    changed_files+=("aur/cosmostrix-bin/PKGBUILD")

    # .SRCINFO
    if [[ -f "${SRCINFO}" ]]; then
        changed_files+=("aur/cosmostrix-bin/.SRCINFO")
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

    # 5. Rename demo assets (only on major version change)
    update_assets "${OLD_VER}" "${NEW_VER}"
    # Track assets directory if files were renamed
    if [[ -d "${ASSETS_DIR}" ]]; then
        # Check if any renamed files exist (git mv already staged them)
        local new_major
        new_major="$(echo "${NEW_VER}" | sed -E 's/^([0-9]+)\..*/\1/')"
        if ls "${ASSETS_DIR}/cosmostrix-v${new_major}-demo"* >/dev/null 2>&1; then
            changed_files+=("assets/")
        fi
    fi

    # 6. Update README demo image refs (only on major version change)
    update_readme_demo_refs "${OLD_VER}" "${NEW_VER}"

    # 7. Audit workflows
    audit_workflows "${NEW_VER}"

    # 8. Run verification
    verify_version "${NEW_VER}"

    # 9. Print summary
    print_summary "${OLD_VER}" "${NEW_VER}" "${changed_files[@]}"
}

main "$@"
