#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# ─────────────────────────────────────────────────────────────────────────────
# Cosmostrix Release Note Generator
#
# Aesthetic: cold, silent, cosmic dragon. Zero emoji. No decoration.
#   Sections are clickable via <details><summary> — click to expand list.
#   Unknown/uncategorized conventional-commit types are grouped under "Others".
#
# USAGE:
#   ./scripts/generate-release-notes.sh <RANGE> <TAG>
#
#   RANGE  — git log range, e.g. "v49.0.0..v50.0.0" or just "v50.0.0"
#   TAG    — current release tag, e.g. "v50.0.0"
#
# OUTPUT:
#   GitHub-flavored Markdown to stdout. Designed for the body of a
#   GitHub Release (softprops/action-gh-release).
#
# CONVENTIONAL COMMIT MAPPING:
#   feat     → Features
#   fix      → Bug Fixes
#   perf     → Performance
#   refactor → Refactor
#   docs     → Documentation
#   test     → Tests
#   ci       → CI
#   build    → Build
#   chore    → (grouped into Others)
#   style    → (grouped into Others)
#   *        → (grouped into Others)
#
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

if [ $# -lt 2 ]; then
  echo "Usage: $0 <RANGE> <TAG>" >&2
  echo "  RANGE: git log range, e.g. v49.0.0..v50.0.0" >&2
  echo "  TAG:   current release tag, e.g. v50.0.0" >&2
  exit 1
fi

RANGE="$1"
TAG="$2"

# ── Section definitions ──────────────────────────────────────────────
# Format: conventional_type|Display Title|Order
# Lower order = appears first in output.
declare -A SECTION_TITLE
declare -A SECTION_ORDER

SECTION_TITLE[feat]="Features"
SECTION_TITLE[fix]="Bug Fixes"
SECTION_TITLE[perf]="Performance"
SECTION_TITLE[refactor]="Refactor"
SECTION_TITLE[docs]="Documentation"
SECTION_TITLE[test]="Tests"
SECTION_TITLE[ci]="CI"
SECTION_TITLE[build]="Build"
SECTION_TITLE[_others]="Others"

SECTION_ORDER[feat]=1
SECTION_ORDER[fix]=2
SECTION_ORDER[perf]=3
SECTION_ORDER[refactor]=4
SECTION_ORDER[docs]=5
SECTION_ORDER[test]=6
SECTION_ORDER[ci]=7
SECTION_ORDER[build]=8
SECTION_ORDER[_others]=99

# Types that go into "Others" instead of getting their own section
OTHERS_TYPES="chore style"

# ── Collect commits ──────────────────────────────────────────────────
# Format: <short-hash>|<type>|<scope>|<subject>
# Conventional commit: type(scope): subject
# Bare commit (no conventional prefix): type=_, scope=, subject=full line
COMMITS="$(git log --no-merges --format='%h|%s' "${RANGE}" 2>/dev/null || true)"

if [ -z "$COMMITS" ]; then
  echo "## ${TAG}"
  echo ""
  echo "No commits found in range \`${RANGE}\`."
  exit 0
fi

# ── Parse and bucket ─────────────────────────────────────────────────
declare -A BUCKET
ALL_SECTIONS=()

while IFS= read -r line; do
  hash="$(echo "$line" | cut -d'|' -f1)"
  subject="$(echo "$line" | cut -d'|' -f2-)"

  # Try to parse conventional commit: type(scope): description
  if echo "$subject" | grep -qE '^[a-zA-Z]+(\([^)]*\))?!?: .+'; then
    # Extract type
    ctype="$(echo "$subject" | sed -E 's/^([a-zA-Z]+)(\([^)]*\))?!?: .+/\1/')"
    # Extract scope (may be empty)
    scope="$(echo "$subject" | sed -nE 's/^[a-zA-Z]+\(([^)]*)\)!?: .+/\1/p')"
    # Extract description (strip type, scope, !, : )
    desc="$(echo "$subject" | sed -E 's/^[a-zA-Z]+(\([^)]*\))?!?: //')"
    # Normalize type to lowercase
    ctype="$(echo "$ctype" | tr '[:upper:]' '[:lower:]')"
  else
    # Non-conventional commit — goes to Others
    ctype="_others"
    scope=""
    desc="$subject"
  fi

  # Route to Others if type is in the Others list or unrecognized
  if echo "$OTHERS_TYPES" | grep -qw "$ctype"; then
    ctype="_others"
  elif [ -z "${SECTION_TITLE[$ctype]+x}" ]; then
    ctype="_others"
  fi

  # Build entry line
  if [ -n "$scope" ]; then
    entry="- [\`${hash}\`](https://github.com/oxyzenQ/cosmostrix/commit/${hash}) **${scope}**: ${desc}"
  else
    entry="- [\`${hash}\`](https://github.com/oxyzenQ/cosmostrix/commit/${hash}) ${desc}"
  fi

  # Append to bucket
  key="$ctype"
  if [ -z "${BUCKET[$key]+x}" ]; then
    BUCKET["$key"]="$entry"
    ALL_SECTIONS+=("$key")
  else
    BUCKET["$key"]="${BUCKET[$key]}
${entry}"
  fi
done <<< "$COMMITS"

# ── Render ───────────────────────────────────────────────────────────
# Header: just the tag. No emoji. Cold.

echo "## ${TAG}"
echo ""

# Count total commits
total="$(echo "$COMMITS" | wc -l | tr -d ' ')"
echo "${total} commits since previous release."
echo ""

# Sort sections by order, then render each as a clickable <details> block
sorted_sections="$(for s in "${ALL_SECTIONS[@]}"; do
  echo "${SECTION_ORDER[$s]}|${s}"
done | sort -t'|' -k1,1n | cut -d'|' -f2-)"

while IFS= read -r section_key; do
  [ -z "$section_key" ] && continue
  title="${SECTION_TITLE[$section_key]}"
  body="${BUCKET[$section_key]}"
  count="$(echo "$body" | wc -l | tr -d ' ')"

  # Clickable <details> block — click to expand the commit list
  echo "<details>"
  echo "<summary><strong>${title}</strong> (${count})</summary>"
  echo ""
  echo "$body"
  echo ""
  echo "</details>"
  echo ""
done <<< "$sorted_sections"

# ── Assets section ───────────────────────────────────────────────────
# Static section at the bottom. The actual file links are added by
# softprops/action-gh-release, so we just put the checksum instructions.
echo "<details>"
echo "<summary><strong>Checksums</strong></summary>"
echo ""
echo "Verify downloads with SHA-512:"
echo ""
echo "\`\`\`bash"
echo "# Linux"
echo "sha512sum -c cosmostrix-bin-${TAG}-linux-amd64-v3.tar.gz.sha512sum"
echo ""
echo "# macOS"
echo "shasum -a 512 -c cosmostrix-bin-${TAG}-darwin-aarch64-native.tar.gz.sha512sum"
echo ""
echo "# Windows (Git Bash)"
echo "sha512sum -c cosmostrix-bin-${TAG}-windows-x86_64.zip.sha512sum"
echo "\`\`\`"
echo ""
echo "</details>"
