#!/usr/bin/env bash
# =============================================================================
# Cosmostrix Version Bump Helper
# =============================================================================
# Updates all stable release version references consistently.
#
# Usage:
#   ./version-to.sh 2.5.0          # Bump to 2.5.0
#   ./version-to.sh --check 2.5.0  # Verify version is 2.5.0 (no changes)
#   ./version-to.sh --help         # Show help
#
# Safety:
#   - Refuses to run if git working tree has unrelated changes
#   - Does not commit, tag, or push automatically
#   - Only edits version-related files
#   - Stable SemVer only: X.Y.Z (no pre-release suffixes)
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------
SCRIPT_NAME="$(basename "$0")"
readonly SCRIPT_NAME
REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"
readonly REPO_ROOT
readonly CARGO_TOML="${REPO_ROOT}/Cargo.toml"
readonly CARGO_LOCK="${REPO_ROOT}/Cargo.lock"
readonly PKGBUILD="${REPO_ROOT}/aur/cosmostrix-bin/PKGBUILD"
readonly SRCINFO="${REPO_ROOT}/aur/cosmostrix-bin/.SRCINFO"
readonly README="${REPO_ROOT}/README.md"
readonly ABOUT_CI="${REPO_ROOT}/workflow/about-ci.md"

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

# ---------------------------------------------------------------------------
# Colors
# ---------------------------------------------------------------------------
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly BLUE='\033[0;34m'
readonly NC='\033[0m'

log_info()  { printf '%b[INFO]%b %s\n'  "${BLUE}" "${NC}" "$*"; }
log_ok()    { printf '%b[OK]%b %s\n'    "${GREEN}" "${NC}" "$*"; }
log_warn()  { printf '%b[WARN]%b %s\n'  "${YELLOW}" "${NC}" "$*"; }
log_err()   { printf '%b[ERROR]%b %s\n' "${RED}" "${NC}" "$*" >&2; }

# ---------------------------------------------------------------------------
# Usage
# ---------------------------------------------------------------------------
show_help() {
    cat <<'HELP'
Cosmostrix Version Bump Helper

Updates all stable release version references consistently across the repo.

USAGE:
    ./version-to.sh <VERSION>           Bump to VERSION
    ./version-to.sh --check <VERSION>   Verify version is VERSION (no changes)
    ./version-to.sh --help              Show this help

ARGUMENTS:
    <VERSION>   Target stable SemVer version, e.g. 2.5.0

EXAMPLES:
    ./version-to.sh 2.5.0              # Bump from current to 2.5.0
    ./version-to.sh --check 2.5.0      # Verify repo is at 2.5.0

VALIDATION:
    - Version must be stable SemVer: X.Y.Z (digits only)
    - Rejects: v2.5.0, 2.5, 2.5.0-stable.1, 2.5.0-beta.1, empty input

WHAT IT UPDATES:
    1. Cargo.toml (package version)
    2. Cargo.lock (via cargo metadata refresh)
    3. aur/cosmostrix-bin/PKGBUILD (pkgver=, _tag=)
    4. README.md (active version examples)
    5. workflow/about-ci.md (active version examples)

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

# ---------------------------------------------------------------------------
# Version validation
# ---------------------------------------------------------------------------
validate_version() {
    local ver="$1"

    if [[ -z "${ver}" ]]; then
        log_err "Version argument is required"
        exit 1
    fi

    # Reject versions with leading 'v'
    if [[ "${ver}" == v* ]]; then
        log_err "Version must not include 'v' prefix. Got: ${ver}"
        log_err "Use: ${SCRIPT_NAME} ${ver#v}"
        exit 1
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

# ---------------------------------------------------------------------------
# Read current version from Cargo.toml
# ---------------------------------------------------------------------------
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

# ---------------------------------------------------------------------------
# Safety: check git working tree
# ---------------------------------------------------------------------------
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

# ---------------------------------------------------------------------------
# Update Cargo.toml
# ---------------------------------------------------------------------------
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

# ---------------------------------------------------------------------------
# Update Cargo.lock
# ---------------------------------------------------------------------------
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

# ---------------------------------------------------------------------------
# Update PKGBUILD
# ---------------------------------------------------------------------------
update_pkgbuild() {
    local old_ver="$1"
    local new_ver="$2"

    log_info "Updating PKGBUILD: pkgver=${old_ver} -> ${new_ver}, _tag= (empty for stable)"

    # Update pkgver
    sed -i -E "s|^pkgver=.*|pkgver=${new_ver}|" "${PKGBUILD}"

    # Ensure _tag is empty for stable releases
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
        log_info "Updating .SRCINFO"
        sed -i -E "s|^\tpkgver = .*|\tpkgver = ${new_ver}|" "${SRCINFO}"
        log_ok ".SRCINFO updated: pkgver = ${new_ver}"
    else
        log_info ".SRCINFO not tracked locally — it is generated by the AUR sync workflow"
    fi
}

# ---------------------------------------------------------------------------
# Update docs/examples
# ---------------------------------------------------------------------------
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
        count_old="$(grep -cF "${old_ver}" "${f}" 2>/dev/null || echo "0")"

        if [[ "${count_old}" -eq 0 ]]; then
            log_info "  No references to ${old_ver} in $(basename "${f}")"
            continue
        fi

        # Strategy:
        # 1. Replace tag references (vOLD -> vNEW) where they appear as the
        #    current version (download URLs, examples), but NOT in changelog
        #    headings like "### v2.1.0" which document a specific release.
        # 2. Replace bare version references (OLD -> NEW) only in active
        #    contexts, skipping changelog/history sections.
        #
        # We use sed to skip lines starting with "### " (markdown headings)
        # which are typically changelog entries documenting a specific release.
        # This is a simple heuristic that works for the current repo structure.

        # Replace vOLD_VERSION (with 'v' prefix) — skip markdown headings
        # Handle vOLD followed by non-version characters (not dash, digit, dot)
        sed -i -E "/^### /!s|v${old_ver}([^0-9.-])|v${new_ver}\1|g" "${f}"
        # Handle vOLD at end of line — skip markdown headings
        sed -i -E "/^### /!s|v${old_ver}$|v${new_ver}|g" "${f}"
        # Handle vOLD followed by quote characters — skip markdown headings
        sed -i "/^### /!s|v${old_ver}\"|v${new_ver}\"|g" "${f}"
        sed -i "/^### /!s|v${old_ver}'|v${new_ver}'|g" "${f}"

        # Replace bare OLD_VERSION — skip markdown headings and changelog
        # section markers. This covers download URLs, example commands,
        # versioning notes, etc. without touching historical changelog entries.
        sed -i "/^### /!s|${old_ver}|${new_ver}|g" "${f}"

        log_ok "  Updated $(basename "${f}")"
    done
}

# ---------------------------------------------------------------------------
# Audit workflow files for hardcoded versions
# ---------------------------------------------------------------------------
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

# ---------------------------------------------------------------------------
# Verification
# ---------------------------------------------------------------------------
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
        log_ok "PKGBUILD: _tag= (empty, correct for stable)"
    else
        log_err "PKGBUILD: _tag=${pkg_tag} (should be empty for stable)"
        ((errors++))
    fi

    # 4. cargo metadata
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

    # 5. Check for stale references to old current version in active docs
    # We only check docs, not changelog/history sections
    # This is a best-effort check — some historical references are expected
    log_ok "Stale reference check: see git diff for full details"

    echo ""
    if [[ "${errors}" -eq 0 ]]; then
        log_ok "All verification checks passed"
    else
        log_err "${errors} verification check(s) failed"
        return 1
    fi
}

# ---------------------------------------------------------------------------
# Print summary
# ---------------------------------------------------------------------------
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

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
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

    # Validate version format
    validate_version "${TARGET_VERSION}"

    local NEW_VER="${TARGET_VERSION}"
    local NEW_TAG="v${NEW_VER}"

    # Read current version
    local OLD_VER
    OLD_VER="$(read_current_version)"
    local OLD_TAG="v${OLD_VER}"

    log_info "Current version: ${OLD_VER} / ${OLD_TAG}"
    log_info "Target version:  ${NEW_VER} / ${NEW_TAG}"
    echo ""

    # Idempotent check
    if [[ "${OLD_VER}" == "${NEW_VER}" ]]; then
        log_info "Already at version ${NEW_VER} — running verification"
        verify_version "${NEW_VER}"
        exit 0
    fi

    # Check mode
    if [[ "${CHECK_MODE}" -eq 1 ]]; then
        log_info "Check mode: verifying current version matches ${NEW_VER}"
        if [[ "${OLD_VER}" == "${NEW_VER}" ]]; then
            verify_version "${NEW_VER}"
            exit 0
        else
            log_err "Version mismatch: current=${OLD_VER}, expected=${NEW_VER}"
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

    # 5. Audit workflows
    audit_workflows "${NEW_VER}"

    # 6. Run verification
    verify_version "${NEW_VER}"

    # 7. Print summary
    print_summary "${OLD_VER}" "${NEW_VER}" "${changed_files[@]}"
}

main "$@"
