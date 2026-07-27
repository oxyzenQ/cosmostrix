#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# Cosmostrix Version Bump Helper
#
# Updates all version references consistently across the repo.
#
# USAGE:
#   ./scripts/version-to.sh v*<VERSION>              Bump to VERSION (stable or pre-release)
#   ./scripts/version-to.sh --check <VERSION>     Verify version is VERSION (no changes)
#   ./scripts/version-to.sh --help                Show this help
#
# EXAMPLES:
#   ./scripts/version-to.sh v25.0.0                # Stable release
#   ./scripts/version-to.sh v25.0.0-alpha.1        # Pre-release
#   ./scripts/version-to.sh bump-alpha             # Shortcut: X.Y.Z → X.Y.Z-alpha.1
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
SCRIPT_NAME="$(basename "$0")"
readonly SCRIPT_NAME
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
readonly REPO_ROOT
readonly CARGO_TOML="${REPO_ROOT}/Cargo.toml"
readonly CARGO_LOCK="${REPO_ROOT}/Cargo.lock"
readonly PKGBUILD="${REPO_ROOT}/aur/cosmostrix-bin/PKGBUILD"
readonly SRCINFO="${REPO_ROOT}/aur/cosmostrix-bin/.SRCINFO"
readonly README="${REPO_ROOT}/README.md"
readonly ABOUT_CI="${REPO_ROOT}/docs/workflow/about-ci.md"

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

Updates all stable AND pre-release version references consistently across
the repo.

USAGE:
    ./scripts/version-to.sh <VERSION>              Bump to VERSION (stable)
    ./scripts/version-to.sh bump-alpha             Bump to X.Y.Z-alpha.1 (fresh)
    ./scripts/version-to.sh bump-beta             Bump to X.Y.Z-beta.1 (fresh)
    ./scripts/version-to.sh bump-rc               Bump to X.Y.Z-rc.1 (fresh)
    ./scripts/version-to.sh bump-pre              Bump to X.Y.Z-pre.1 (fresh)
    ./scripts/version-to.sh bump-prerelease       Increment pre-release number
    ./scripts/version-to.sh --check <VERSION>     Verify version is VERSION (no changes)
    ./scripts/version-to.sh --help                Show this help

ARGUMENTS:
    <VERSION>   Target SemVer version, e.g. 2.5.0 (stable) or 25.0.0-alpha.1 (pre-release)

PRE-RELEASE COMMANDS:
    bump-alpha        Start/switch to alpha channel at .1 (X.Y.Z-alpha.1)
    bump-beta         Start/switch to beta channel at .1 (X.Y.Z-beta.1)
    bump-rc           Start/switch to rc channel at .1 (X.Y.Z-rc.1)
    bump-pre          Start/switch to pre channel at .1 (X.Y.Z-pre.1)
    bump-prerelease   Increment existing pre-release number (alpha.1 → alpha.2)
                      Requires current version to already be a pre-release.

    Pre-release versions trigger the CI pre-release pipeline:
      - GitHub Release is marked as prerelease (make_latest: false)
      - Release title appends " (Pre-Release)"
      - The "latest release" pointer stays on the last stable version
      - Changelog for a final release spans from the last STABLE tag,
        ignoring intermediate pre-release tags

EXAMPLES:
    ./scripts/version-to.sh 18.0.0               # Bump from current to 18.0.0
    ./scripts/version-to.sh v18.0.0               # Same — 'v' prefix auto-stripped
    ./scripts/version-to.sh --check 18.0.0        # Verify repo is at 18.0.0
    ./scripts/version-to.sh --check v18.0.0       # Same — 'v' prefix auto-stripped
    ./scripts/version-to.sh bump-alpha            # 25.0.0 → 25.0.0-alpha.1
    ./scripts/version-to.sh bump-prerelease        # 25.0.0-alpha.1 → 25.0.0-alpha.2
    ./scripts/version-to.sh bump-rc                # 25.0.0-alpha.2 → 25.0.0-rc.1
    ./scripts/version-to.sh 25.0.0                 # Final: 25.0.0-rc.1 → 25.0.0 (stable)

VALIDATION:
    - Stable: X.Y.Z (digits only)
    - Pre-release: X.Y.Z-{alpha|beta|rc|pre}.N
    - Optional 'v' prefix is accepted and stripped (v18.0.0 → 18.0.0)
    - Rejects: 2.5, 2.5.0-stable.1, empty input

WHAT IT UPDATES:
    1. Cargo.toml (package version)
    2. Cargo.lock (via cargo metadata refresh)
    3. aur/cosmostrix-bin/PKGBUILD (pkgver=, _tag=)
    4. README.md (active version examples)
    5. docs/workflow/about-ci.md (active version examples)
    6. assets/cosmostrix-v{MAJOR}-demo* (auto git mv on major change)
    7. README.md demo img refs (auto on major change)

SAFETY:
    - Warns if git working tree is dirty (use --allow-dirty to proceed)
    - Does NOT commit, tag, or push
    - Only edits version-related fields

NEXT STEPS AFTER BUMP:
    cargo fmt --all
    cargo test --all --locked
    cargo clippy --locked --all-targets --all-features -- -D warnings
    cargo pro-linux-v3
    target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix -i
    git diff
    git commit -m "chore: bump version to vNEW"
    git tag vNEW
    git push origin main vNEW
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
    # when _tag is empty, so pre-release versions like 25.0.0-alpha.1 work
    # correctly with _tag= (tag becomes v25.0.0-alpha.1).
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
    for f in "${DOC_FILES[@]}"; do
        if [[ ! -f "${f}" ]]; then
            log_warn "Doc file not found: ${f}"
            continue
        fi

        log_info "Updating version references in $(basename "${f}")"

        # Count occurrences before
        local count_old
        count_old="$(grep -cF "${old_ver}" "${f}" 2>/dev/null || true)"
        count_old="${count_old:-0}"

        if [[ "${count_old}" -eq 0 ]]; then
            log_info "  No references to ${old_ver} in $(basename "${f}")"
            continue
        fi

        # Strategy:
        # 1. Replace bare version references (OLD -> NEW) FIRST, only in
        #    active contexts, skipping changelog/history sections.
        # 2. Replace tag references (vOLD -> vNEW) AFTER, where they appear
        #    as the current version (download URLs, examples), but NOT in
        #    changelog headings.
        #
        # Ordering matters: bare replacement MUST run before v-prefix
        #    replacement when old_ver is a prefix of new_ver (e.g.
        #    25.0.0 → 25.0.0-alpha.1). If v-prefix runs first, the bare
        #    sed then finds old_ver inside new_ver and double-replaces,
        #    producing 25.0.0-alpha.1-alpha.1.
        #
        # We use sed to skip lines starting with "### " (markdown headings)
        # which are typically changelog entries documenting a specific release.

        # Step 1: Replace bare OLD_VERSION — skip markdown headings and changelog
        # section markers. This covers download URLs, example commands,
        # versioning notes, etc. without touching historical changelog entries.
        sed -i "/^### /!s|${old_ver}|${new_ver}|g" "${f}"

        # Step 2: Replace vOLD_VERSION (with 'v' prefix) — skip markdown headings
        # Handle vOLD followed by non-version characters (not dash, digit, dot)
        sed -i -E "/^### /!s|v${old_ver}([^0-9.-])|v${new_ver}\1|g" "${f}"
        # Handle vOLD at end of line — skip markdown headings
        sed -i -E "/^### /!s|v${old_ver}$|v${new_ver}|g" "${f}"
        # Handle vOLD followed by quote characters — skip markdown headings
        sed -i "/^### /!s|v${old_ver}\"|v${new_ver}\"|g" "${f}"
        sed -i "/^### /!s|v${old_ver}'|v${new_ver}'|g" "${f}"

        log_ok "  Updated $(basename "${f}")"
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
# version — update_docs() replaces the full vX.Y.Z, which doesn't match
# the asset filenames.
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
    # The README install snippet uses TAG="v<VERSION>" — this is an active
    # version reference that must agree with Cargo.toml. Without this check,
    # a README-only desync would silently pass `version-to.sh --check` and
    # only be caught later by the Rust test suite (docs_tests::metadata).
    if [[ -f "${README}" ]]; then
        local expected_tag="TAG=\"v${expected_ver}\""
        if grep -qF "${expected_tag}" "${README}"; then
            log_ok "README.md: ${expected_tag}"
        else
            log_err "README.md: missing or stale install tag (expected ${expected_tag})"
            log_err "  Run './scripts/build.sh v${expected_ver}' to sync all active files"
            ((errors++))
        fi
    fi

    # 7. Active doc files (README, about-ci.md) must reference the current
    # version at least once — if neither vX.Y.Z nor X.Y.Z appears, the file
    # has likely been edited to remove the install example, or the version
    # bump missed it. Historical CHANGELOG.md refs are NOT scanned (only
    # DOC_FILES are checked, which exclude CHANGELOG.md).
    local doc_file
    for doc_file in "${DOC_FILES[@]}"; do
        [[ -f "${doc_file}" ]] || continue
        local doc_name
        doc_name="$(basename "${doc_file}")"
        if grep -qF "v${expected_ver}" "${doc_file}" 2>/dev/null; then
            log_ok "${doc_name}: references v${expected_ver}"
        else
            log_err "${doc_name}: no reference to v${expected_ver} found"
            log_err "  Run './scripts/build.sh v${expected_ver}' to sync"
            ((errors++))
        fi
    done

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
    echo "  target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix -i"
    echo "  git diff"
    echo "  git commit -m \"chore: bump version to v${new_ver}\""
    echo "  git tag v${new_ver}"
    echo "  git push origin main v${new_ver}"
    echo "=========================================="
}

#
# Pre-release version support
#
# These helpers compute pre-release version targets from the current
# stable version in Cargo.toml. They enable the CI pre-release pipeline:
#   1. bump-alpha  → X.Y.Z-alpha.1  (first alpha from stable X.Y.Z)
#   2. bump-prerelease → X.Y.Z-alpha.2  (increment the pre-release number)
#   ... or bump-beta, bump-rc for channel switches.
#
# Pre-release versions use the SemVer build metadata format:
#   X.Y.Z-alpha.N
#   X.Y.Z-beta.N
#   X.Y.Z-rc.N
#   X.Y.Z-pre.N
#
# The CI release workflow (release.yml) detects these suffixes and marks
# the GitHub Release as a pre-release (make_latest: false) so the
# "latest release" pointer stays on the last stable version.
#

# Validate a pre-release version string.
# Accepts: X.Y.Z-alpha.N, X.Y.Z-beta.N, X.Y.Z-rc.N, X.Y.Z-pre.N
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
    if ! [[ "${ver}" =~ ^[0-9]+\.[0-9]+\.[0-9]+-(alpha|beta|rc|pre)\.[0-9]+$ ]]; then
        log_err "Invalid pre-release version: ${ver}"
        log_err "Expected: X.Y.Z-{alpha|beta|rc|pre}.N (e.g. 25.0.0-alpha.1)"
        exit 1
    fi
}

# Compute the pre-release target version from the current Cargo.toml version.
# Args:
#   $1 = channel (alpha|beta|rc|pre)
# Output: target version string (e.g. 25.0.0-alpha.1)
# Exit 1 on error (e.g. current version is already a pre-release of a different channel)
compute_prerelease_target() {
    local channel="$1"
    local current
    current="$(read_current_version)"

    # Strip optional 'v' prefix from current
    local bare="${current#v}"

    # If current is already a pre-release of the SAME channel, increment the number.
    if [[ "${bare}" =~ ^([0-9]+\.[0-9]+\.[0-9]+)-${channel}\.([0-9]+)$ ]]; then
        local base="${BASH_REMATCH[1]}"
        local num="${BASH_REMATCH[2]}"
        local new_num=$((num + 1))
        echo "${base}-${channel}.${new_num}"
        return 0
    fi

    # If current is a pre-release of a DIFFERENT channel, error — use the
    # specific channel command (bump-alpha/bump-beta/bump-rc) to switch.
    if [[ "${bare}" == *-* ]]; then
        log_err "Current version ${bare} is already a pre-release of a different channel."
        log_err "To switch channels, run: ./scripts/version-to.sh bump-${channel}"
        log_err "(This resets the pre-release number to 1 for the new channel.)"
        exit 1
    fi

    # Current is stable X.Y.Z — start a new pre-release at .1
    if ! [[ "${bare}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        log_err "Current Cargo.toml version is not stable SemVer: ${bare}"
        log_err "Cannot compute pre-release target from a non-stable base."
        exit 1
    fi

    echo "${bare}-${channel}.1"
}

# Compute a fresh pre-release target (always .1, even if already in a channel).
# Used by bump-alpha/bump-beta/bump-rc to switch channels or start fresh.
compute_fresh_prerelease_target() {
    local channel="$1"
    local current
    current="$(read_current_version)"
    local bare="${current#v}"

    # Extract the stable base (X.Y.Z) from either stable or pre-release current.
    local base
    if [[ "${bare}" =~ ^([0-9]+\.[0-9]+\.[0-9]+) ]]; then
        base="${BASH_REMATCH[1]}"
    else
        log_err "Cannot extract stable base from current version: ${bare}"
        exit 1
    fi

    echo "${base}-${channel}.1"
}

#
# Main
#
main() {
    local CHECK_MODE=0
    local ALLOW_DIRTY=0
    local TARGET_VERSION=""

    # Pre-release subcommand detection.
    # These commands compute the target version from the current Cargo.toml
    # version + the requested channel, then fall through to the normal
    # apply pipeline. They enable the CI pre-release workflow.
    if [[ $# -ge 1 ]]; then
        case "$1" in
            bump-alpha)
                shift
                TARGET_VERSION="$(compute_fresh_prerelease_target "alpha")"
                log_info "bump-alpha: target = ${TARGET_VERSION}"
                ;;
            bump-beta)
                shift
                TARGET_VERSION="$(compute_fresh_prerelease_target "beta")"
                log_info "bump-beta: target = ${TARGET_VERSION}"
                ;;
            bump-rc)
                shift
                TARGET_VERSION="$(compute_fresh_prerelease_target "rc")"
                log_info "bump-rc: target = ${TARGET_VERSION}"
                ;;
            bump-pre)
                shift
                TARGET_VERSION="$(compute_fresh_prerelease_target "pre")"
                log_info "bump-pre: target = ${TARGET_VERSION}"
                ;;
            bump-prerelease)
                # Increment the existing pre-release number (same channel).
                # Detects the current channel from Cargo.toml and bumps N → N+1.
                shift
                local current
                current="$(read_current_version)"
                local bare="${current#v}"
                local channel=""
                if [[ "${bare}" =~ ^[0-9]+\.[0-9]+\.[0-9]+-(alpha|beta|rc|pre)\.[0-9]+$ ]]; then
                    channel="${BASH_REMATCH[1]}"
                else
                    log_err "bump-prerelease requires current version to be a pre-release."
                    log_err "Current: ${bare}"
                    log_err "Use: ./scripts/version-to.sh bump-alpha (or bump-beta / bump-rc)"
                    exit 1
                fi
                TARGET_VERSION="$(compute_prerelease_target "${channel}")"
                log_info "bump-prerelease: ${current} → ${TARGET_VERSION}"
                ;;
        esac
    fi

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

    # 4. Update docs/examples
    update_docs "${OLD_VER}" "${NEW_VER}"
    for f in "${DOC_FILES[@]}"; do
        if [[ -f "${f}" ]]; then
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
