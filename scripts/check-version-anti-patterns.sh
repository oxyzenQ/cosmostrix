#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# =============================================================================
# COSMOSTRIX VERSION-ANTI-PATTERN GUARD
# =============================================================================
# Fails if any source file re-introduces the hardcoded-version-string
# anti-pattern that previously broke CI on every version bump.
#
# Anti-pattern blocked:
#   - contains("version = \"X.Y.Z\"")  (Cargo.toml version tautology)
#   - contains("pkgver=X.Y.Z")          (PKGBUILD version check)
#   - contains("pkgver = X.Y.Z")        (.SRCINFO version check)
#   - contains(r#"TAG="vX.Y.Z""#)       (README install tag)
#
# Correct pattern (allowed):
#   const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
#   ... contains(&format!("version = \"{}\"", CURRENT_VERSION)) ...
#
# Historical CHANGELOG assertions (e.g. contains("## v4.0.0")) are NOT
# blocked — those verify a historical release entry exists and remain
# valid forever.
#
# Usage: bash scripts/check-version-anti-patterns.sh
# =============================================================================
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

# Patterns that indicate a hardcoded current-version assertion.
# Each pattern matches the literal version-string form (not the env! form).
PATTERNS=(
    'contains(r#"version = "[0-9]'
    'contains("version = \\"[0-9]'
    'contains("pkgver=[0-9]'
    'contains("pkgver = [0-9]'
    'contains(r#"TAG="v[0-9]'
    'contains("TAG=\\"v[0-9]'
)

VIOLATIONS=0
FILES_CHECKED=0

while IFS= read -r -d '' file; do
    FILES_CHECKED=$((FILES_CHECKED + 1))
    for pattern in "${PATTERNS[@]}"; do
        # Use grep -F for literal patterns that contain no regex metachars,
        # otherwise use grep -E. All our patterns contain regex metachars
        # ([0-9], ", =), so use grep -E.
        if grep -nE -- "$pattern" "$file" >/dev/null 2>&1; then
            echo -e "${RED}VIOLATION: ${file}${NC}"
            grep -nE -- "$pattern" "$file" | head -5 | sed 's/^/    /'
            VIOLATIONS=$((VIOLATIONS + 1))
        fi
    done
done < <(
    find "$REPO_ROOT/src" \
        -name '*.rs' \
        -not -path '*/target/*' \
        -print0 2>/dev/null
)

if [[ "$VIOLATIONS" -eq 0 ]]; then
    echo "OK: $FILES_CHECKED source files checked, no version-anti-pattern violations"
    exit 0
else
    echo ""
    echo -e "${RED}FAIL: $VIOLATIONS file(s) contain hardcoded version assertions${NC}"
    echo ""
    echo "Fix: replace literal version strings with env!(\"CARGO_PKG_VERSION\")."
    echo "Example:"
    echo "  // BAD  ->  assert!(cargo.contains(r#\"version = \"5.0.1\"\"#));"
    echo "  // GOOD ->  const V: &str = env!(\"CARGO_PKG_VERSION\");"
    echo "             assert!(cargo.contains(&format!(\"version = \\\"{}\\\"\", V)));"
    exit 1
fi
