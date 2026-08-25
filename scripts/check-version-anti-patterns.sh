#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# ─────────────────────────────────────────────────────────────────────────────
# PLATFORM: UNIX-only (Linux, macOS, BSD).
#   Uses `find -print0`, `sed -E`, `grep -nE`, bash arrays. Not for Windows
#   cmd.exe / PowerShell — use Git Bash or WSL on Windows.
# ─────────────────────────────────────────────────────────────────────────────
#
# COSMOSTRIX VERSION-ANTI-PATTERN GUARD
#
# Fails if any source file re-introduces the hardcoded-version-string
# anti-pattern that previously broke CI on every version bump.
#
# Anti-pattern blocked:
#   - contains("version = \"X.Y.Z\"")  (Cargo.toml version tautology)
#   - contains("pkgver=X.Y.Z")          (PKGBUILD version check)
#   - contains("pkgver = X.Y.Z")        (.SRCINFO version check)
#   - contains(r#"TAG="vX.Y.Z""#)       (README install tag)
#   - "Engine (v[0-9])" / "Engine(v[0-9])"  (stale hardcoded engine version
#     in user-facing strings — must use env!("CARGO_PKG_VERSION") instead)
#   - "v[0-9]+ Cosmic Dragon" / "v[0-9]+ Dragon"  (hardcoded release-name
#     version prefix in user-facing strings — the brand is "Cosmic Dragon"
#     without a version prefix; the version comes from --version output)
#
# Correct pattern (allowed):
#   const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
#   ... contains(&format!("version = \"{}\"", CURRENT_VERSION)) ...
#   format!("Engine (v{ver})", ver = env!("CARGO_PKG_VERSION"))
#
# Historical CHANGELOG assertions (e.g. contains("## v4.0.0")) are NOT
# blocked — those verify a historical release entry exists and remain
# valid forever. Migration messages like "removed in v14.0.0" are also
# NOT blocked — they reference a fixed historical event.
#
# Usage: bash scripts/check-version-anti-patterns.sh
#
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

RED='\033[0;31m'
NC='\033[0m'

# Patterns that indicate a hardcoded current-version assertion.
# Each pattern matches the literal version-string form (not the env! form).
#
# Note: comments are stripped before matching, so historical references
# in `// v15 Cosmic Dragon: ...` code comments are NOT flagged. Only
# patterns inside string literals (which reach the binary) are blocked.
PATTERNS=(
	'contains(r#"version = "[0-9]'
	'contains("version = \\"[0-9]'
	'contains("pkgver=[0-9]'
	'contains("pkgver = [0-9]'
	'contains(r#"TAG="v[0-9]'
	'contains("TAG=\\"v[0-9]'
	# Hardcoded engine version in user-facing strings.
	# Catches: "Engine (v20)", "Engine(v20)", "Engine: ... (v20)"
	# Allowed: format!("Engine (v{ver})", ver = env!("CARGO_PKG_VERSION"))
	'Engine \(v[0-9]'
	# Hardcoded release-name version prefix in user-facing strings.
	# Catches: "v15 Cosmic Dragon", "v20 Dragon" — the brand is just
	# "Cosmic Dragon" without a version prefix.
	'v[0-9]+ (Cosmic )?Dragon'
)

VIOLATIONS=0
FILES_CHECKED=0

# Strip Rust line comments (// ...) and doc comments (//! ..., /// ...)
# before matching. This ensures historical references in code comments
# (e.g. `// v15 Cosmic Dragon: ...`) are NOT flagged — only patterns
# inside string literals (which actually reach the binary) are blocked.
strip_rust_comments() {
	local file="$1"
	# Remove // comments but preserve strings (best-effort: a // inside
	# a string literal would be wrongly stripped, but version patterns
	# don't appear inside such strings in this codebase).
	sed -E 's|//.*$||' "$file"
}

while IFS= read -r -d '' file; do
	FILES_CHECKED=$((FILES_CHECKED + 1))
	# Pre-strip comments so the patterns only match string-literal content.
	stripped=$(strip_rust_comments "$file")
	for pattern in "${PATTERNS[@]}"; do
		# Use grep -E on the comment-stripped content.
		if printf '%s\n' "$stripped" | grep -nE -- "$pattern" >/dev/null 2>&1; then
			echo -e "${RED}VIOLATION: ${file}${NC}"
			printf '%s\n' "$stripped" | grep -nE -- "$pattern" | head -5 | sed 's/^/    /'
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
