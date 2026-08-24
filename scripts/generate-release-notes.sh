#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# ─────────────────────────────────────────────────────────────────────────────
# cosmostrix Release Note Generator — single source of truth
#
# Called by .github/workflows/release.yml ("Generate release body" step).
# Output: GitHub-flavored Markdown to stdout, designed for the body of a
# GitHub Release (softprops/action-gh-release body_path).
#
# Aesthetic: cold, silent, cosmic dragon. Zero emoji. No decoration.
#
# USAGE:
#   ./scripts/generate-release-notes.sh <RANGE> <TAG> <IS_PRERELEASE> <PREV_TAG> [LAST_STABLE_TAG]
#
#   RANGE            git log range, e.g. "v50.0.0-alpha.3..v50.0.0-beta.4".
#                    Empty string = initial release (no previous tag).
#   TAG              current release tag, e.g. "v50.0.0-beta.4"
#   IS_PRERELEASE    "true" | "false"
#   PREV_TAG         previous tag — any tag for pre-releases, the last
#                    stable tag for stable releases (workflow computes it)
#   LAST_STABLE_TAG  last STABLE tag, e.g. "v15.0.0". Optional. When set and
#                    different from PREV_TAG (pre-release builds), a dual
#                    range line is rendered: "N commits since previous
#                    build · M commits since last stable".
#
# DESIGN (owner-approved Option A, 2026-08-23 release-note audit):
#   - Stability alert: [!WARNING] (yellow/amber) for pre-releases, [!TIP]
#     (green) for stable releases. GitHub alerts are the ONLY native color
#     mechanism in release bodies — HTML color attributes are sanitized.
#   - Every achievement count is a clickable <details> toggle that expands
#     the per-commit changelog for that category — zero navigation, each
#     commit hash links to its GitHub commit page.
#   - Second-level commit-type parsing: subjects under the repo's primary
#     convention "internal research: <verb> ..." are classified by their
#     verb (fix → fix, feat/implement → feat, document → docs, ...).
#     First-level "grep ^fix" alone undercounted fix by ~19% in the
#     beta.4 audit (61 reported vs 75 real in alpha.3..beta.4).
#   - Full changelog preserved as a collapsed details block with the
#     GitHub compare link for the full-range diff view.
#   - Verification section (GPG + checksum policy) lives here too, so the
#     release body has exactly one implementation.
#
# COMMIT CLASSIFICATION:
#   1. "internal research: <verb> ..." — verb mapped:
#        fix/fixes/fixed                → fix
#        feat/feature/implement/introduce → feat
#        perf/optimize                  → perf
#        refactor                       → refactor
#        docs/doc/document              → docs
#        test/tests                     → test
#        ci                             → ci
#        build                          → build
#        chore/style                    → chore
#        anything else                  → others
#   2. Conventional commits "type(scope)!: subject" — type mapped the same
#      way; unknown types fall into "others".
#   3. Bare subjects (no recognizable prefix) → "others".
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

if [ $# -lt 4 ]; then
  echo "Usage: $0 <RANGE> <TAG> <IS_PRERELEASE> <PREV_TAG> [LAST_STABLE_TAG]" >&2
  echo "  RANGE:           git log range, e.g. v49.0.0..v50.0.0 (empty = initial release)" >&2
  echo "  TAG:             current release tag, e.g. v50.0.0" >&2
  echo "  IS_PRERELEASE:   true | false" >&2
  echo "  PREV_TAG:        previous tag (any for pre-releases, last stable for stable)" >&2
  echo "  LAST_STABLE_TAG: last stable tag (optional, enables the dual range line)" >&2
  exit 1
fi

RANGE="$1"
TAG="$2"
IS_PRERELEASE="$3"
PREV_TAG="$4"
LAST_STABLE="${5:-}"

# Repository URL for commit/compare links. In CI these env vars are set by
# GitHub Actions; locally they fall back to the canonical repository.
REPO_URL="${GITHUB_SERVER_URL:-https://github.com}/${GITHUB_REPOSITORY:-oxyzenQ/cosmostrix}"

# ── Section definitions ──────────────────────────────────────────────
# Order matches the owner's release-note mockup: fix first, then feat.
declare -A SECTION_ORDER

SECTION_ORDER[fix]=1
SECTION_ORDER[feat]=2
SECTION_ORDER[perf]=3
SECTION_ORDER[refactor]=4
SECTION_ORDER[docs]=5
SECTION_ORDER[test]=6
SECTION_ORDER[ci]=7
SECTION_ORDER[build]=8
SECTION_ORDER[chore]=9
SECTION_ORDER[_others]=99

# ── Commit classification (v2) ─────────────────────────────────────
# Three-stage classifier, most-trusted signal first:
#
#   1. DIFF STAT (ground truth): a commit whose changed files are ALL
#      *.md is documentation work — no subject-line guessing can beat
#      the file list.
#   2. VERB TABLE: the first word after the repo's "internal research:"
#      prefix is the author's declared intent — when it is a recognized
#      verb, trust it (an "align ... and add ..." subject stays a fix,
#      not a feat).
#   3. KEYWORD SCAN (fallback for noun-led subjects: "security ...",
#      "platform ...", "PGO ..."): scan the first 8 words of a CLEANED
#      subject (lowercased, parentheticals and path-like tokens removed)
#      for high-signal keywords. Priority fix > docs > feat > chore —
#      a commit that says "fix" is a fix even when it also produces
#      documentation.
#
# Conventional commits keep their direct mapping; "bump" and "revert"
# are now recognized instead of falling into Others.
#
# Design notes (2026-08-23, beta.5 release-note review): the v1
# classifier left 24 of 28 commits in "others" because this repo's
# subjects are noun-led as often as verb-led. The v2 stages were tuned
# against the real beta.4..beta.5 range (see the test block below).

FIX_KW='fix|fixes|fixed|bug|crash|regression|deadlock|race|corrupt|kill|trap|drift|miscount|undercount|hazard|revert'
DOCS_KW='audit|research|study|verdict|docs|documentation|changelog|identity|unlock|relock|notes|results|measurement'
FEAT_KW='add|adds|added|implement|introduce|apply|extend|support|enable|create|build|preset'
CHORE_KW='bump|pin|pinned|trim|trimmed|deps|dependencies|version|cleanup'

# Clean a subject for keyword scanning: strip the repo prefix (its
# "research" would otherwise match DOCS_KW on every commit), lowercase,
# drop parentheticals (finding IDs like "(audit LOW-2)" live there) and
# path-like tokens ("docs-audit.py" would otherwise match "audit"),
# then keep the first 8 words — secondary clauses ("... and add X")
# beyond word 8 must not reclassify the commit.
scan_text() {
    printf '%s' "$1" \
        | sed -E 's/^internal research:[[:space:]]*//' \
        | tr '[:upper:]' '[:lower:]' \
        | sed -E 's/\([^)]*\)//g' \
        | sed -E 's/[^[:space:]]*[./:][^[:space:]]*//g' \
        | tr -s '[:space:]' ' ' \
        | cut -d' ' -f1-8
}

# Map a "internal research:" verb to a section key. Extended beyond the
# conventional-commit vocabulary with the verbs this repo actually uses.
verb_to_section() {
    case "$1" in
      fix | fixes | fixed | kill | resolve | repair | align | exclude | eliminate | correct | restore)
        echo "fix" ;;
      feat | feature | implement | implements | introduce | introduces | apply | extend | enable | support | create | add | preset | masterclass)
        echo "feat" ;;
      perf | optimize | optimise | optimization)
        echo "perf" ;;
      refactor | refactors | refactored | simplify)
        echo "refactor" ;;
      docs | doc | document | documents | documented | record | write | note | audit | research | study | verify | retroactive | honest | master)
        echo "docs" ;;
      test | tests)
        echo "test" ;;
      ci)
        echo "ci" ;;
      build)
        echo "build" ;;
      chore | style | bump | pin | trim | update | refresh | clean | tidy | gate)
        echo "chore" ;;
      *)
        echo "" ;;
    esac
}

classify_subject() {
  local subject="$1"
  local mdonly="${2:-0}"
  local verb scan

  # Stage 1: diff-stat ground truth — an all-*.md commit is docs work.
  if [ "$mdonly" = "1" ]; then
    echo "docs"
    return
  fi

  # Stage 2 + 3: the repo's primary convention
  # "internal research: <verb-or-noun> ...".
  if printf '%s' "$subject" | grep -qE '^internal research:[[:space:]]*[a-zA-Z]+'; then
    verb="$(printf '%s' "$subject" | sed -nE 's/^internal research:[[:space:]]*([a-zA-Z]+).*/\1/p' | tr '[:upper:]' '[:lower:]')"
    # Verb-first: the declared intent wins when it is recognized.
    local mapped
    mapped="$(verb_to_section "$verb")"
    if [ -n "$mapped" ]; then
      echo "$mapped"
      return
    fi
    # Noun-led subject: guarded keyword fallback.
    scan="$(scan_text "$subject")"
    if printf '%s' "$scan" | grep -qwE "$FIX_KW"; then echo "fix"; return; fi
    if printf '%s' "$scan" | grep -qwE "$DOCS_KW"; then echo "docs"; return; fi
    if printf '%s' "$scan" | grep -qwE "$FEAT_KW"; then echo "feat"; return; fi
    if printf '%s' "$scan" | grep -qwE "$CHORE_KW"; then echo "chore"; return; fi
    echo "_others"
    return
  fi

  # Conventional commit: type(scope)!: subject
  if printf '%s' "$subject" | grep -qE '^[a-zA-Z]+(\([^)]*\))?!?: .+'; then
    local ctype
    ctype="$(printf '%s' "$subject" | sed -E 's/^([a-zA-Z]+)(\([^)]*\))?!?: .+/\1/' | tr '[:upper:]' '[:lower:]')"
    case "$ctype" in
      fix | revert) echo "fix" ;;
      feat) echo "feat" ;;
      perf) echo "perf" ;;
      refactor) echo "refactor" ;;
      docs) echo "docs" ;;
      test) echo "test" ;;
      ci) echo "ci" ;;
      build) echo "build" ;;
      chore | style | bump) echo "chore" ;;
      *) echo "_others" ;;
    esac
    return
  fi

  # Bare subject.
  echo "_others"
}

# ── Collect commits + per-commit diff stat ──────────────────────────
COMMITS=""
declare -A MD_ONLY
if [ -n "$RANGE" ]; then
  COMMITS="$(git log --no-merges --format='%h|%s' "${RANGE}" 2>/dev/null || true)"
  # One extra pass: which commits touched ONLY *.md files (docs ground
  # truth for the classifier's stage 1). Parsed in a single git call so
  # large ranges (the v15..stable span) stay fast.
  _cur=""
  _files=0
  _md=0
  _md_flush() {
    [ -n "$_cur" ] || return 0
    if [ "$_files" -gt 0 ] && [ "$_md" -eq "$_files" ]; then
      MD_ONLY["$_cur"]=1
    else
      MD_ONLY["$_cur"]=0
    fi
  }
  while IFS= read -r line; do
    case "$line" in
      '@'*)
        _md_flush
        _cur="${line#@}"
        _files=0
        _md=0
        ;;
      '')
        ;;
      *)
        _files=$((_files + 1))
        case "$line" in
          *.md) _md=$((_md + 1)) ;;
        esac
        ;;
    esac
  done < <(git log --no-merges --name-only --format='@%h' "${RANGE}" 2>/dev/null || true)
  _md_flush
fi

# ── Header + stability alert ─────────────────────────────────────────
echo "## What's Changed"
echo ""

if [ "$IS_PRERELEASE" = "true" ]; then
  echo "> [!WARNING]"
  echo "> **Pre-release build — not a stable release. Expect bugs.**"
else
  echo "> [!TIP]"
  echo "> **Stable release.**"
fi
echo ""

# ── Initial-release handling ─────────────────────────────────────
# No previous tag / empty range: print the marker; the range summary,
# achievements, and full changelog below are all guarded on COMMITS and
# skip cleanly. The verification section at the end still renders.
if [ -z "$COMMITS" ]; then
  echo "Initial release."
  echo ""
fi

# ── Range summary line (single or dual) ──────────────────────────────
TOTAL=""
STABLE_TOTAL=""
if [ -n "$COMMITS" ]; then
  TOTAL="$(printf '%s\n' "$COMMITS" | wc -l | tr -d ' ')"

  # Dual range: pre-release builds also show the distance from the last
  # STABLE release, so testers see the incremental delta AND the big picture.
  if [ -n "$LAST_STABLE" ] && [ "$LAST_STABLE" != "$PREV_TAG" ] \
    && git rev-parse "$LAST_STABLE" >/dev/null 2>&1; then
    STABLE_TOTAL="$(git log --no-merges "${LAST_STABLE}..${TAG}" --oneline 2>/dev/null | wc -l | tr -d ' ')"
  fi

  if [ -n "$STABLE_TOTAL" ]; then
    echo "**${TOTAL} commits** since \`${PREV_TAG}\` (previous build) · **${STABLE_TOTAL} commits** since \`${LAST_STABLE}\` (last stable)"
  else
    echo "**${TOTAL} commits** since \`${PREV_TAG}\`"
  fi
  echo ""
fi

# ── Parse and bucket commits ─────────────────────────────────────────
declare -A BUCKET
declare -A COUNT
ALL_SECTIONS=()

while IFS= read -r line; do
  [ -z "$line" ] && continue
  hash="$(printf '%s' "$line" | cut -d'|' -f1)"
  subject="$(printf '%s' "$line" | cut -d'|' -f2-)"
  key="$(classify_subject "$subject" "${MD_ONLY[$hash]:-0}")"

  # Entry display text: strip process prefixes, keep the human part.
  display="$subject"
  if printf '%s' "$subject" | grep -qE '^internal research:[[:space:]]*'; then
    display="$(printf '%s' "$subject" | sed -E 's/^internal research:[[:space:]]*//')"
  elif printf '%s' "$subject" | grep -qE '^[a-zA-Z]+(\([^)]*\))?!?: .+'; then
    scope="$(printf '%s' "$subject" | sed -nE 's/^[a-zA-Z]+\(([^)]*)\)!?: .+/\1/p')"
    desc="$(printf '%s' "$subject" | sed -E 's/^[a-zA-Z]+(\([^)]*\))?!?: //')"
    if [ -n "$scope" ]; then
      display="**${scope}**: ${desc}"
    else
      display="$desc"
    fi
  fi

  entry="- [\`${hash}\`](${REPO_URL}/commit/${hash}) ${display}"

  if [ -z "${BUCKET[$key]+x}" ]; then
    BUCKET["$key"]="$entry"
    ALL_SECTIONS+=("$key")
    COUNT["$key"]=1
  else
    BUCKET["$key"]="${BUCKET[$key]}
${entry}"
    COUNT["$key"]=$((COUNT[$key] + 1))
  fi
done <<< "$COMMITS"

# ── Total achievements: per-category clickable details ───────────────
if [ -n "$COMMITS" ]; then
  echo "> [!NOTE]"
  echo "> **Total achievements** — click a category to expand its changelog."
  echo ""

  sorted_sections="$(for s in "${ALL_SECTIONS[@]}"; do
    echo "${SECTION_ORDER[$s]}|${s}"
  done | sort -t'|' -k1,1n | cut -d'|' -f2-)"

  while IFS= read -r section_key; do
    [ -z "$section_key" ] && continue
    body="${BUCKET[$section_key]}"
    count="${COUNT[$section_key]}"

    echo "<details>"
    echo "<summary><strong>${section_key/_others/others} × ${count}</strong></summary>"
    echo ""
    echo "$body"
    echo ""
    echo "</details>"
    echo ""
  done <<< "$sorted_sections"

  # ── Full changelog with compare link ─────────────────────────────
  echo "<details>"
  echo "<summary><strong>Full changelog</strong> · ${TOTAL} commits · [compare view](${REPO_URL}/compare/${RANGE})</summary>"
  echo ""
  git log --no-merges --pretty=format:'- %s (%h)' "${RANGE}"
  echo ""
  echo ""
  echo "</details>"
  echo ""
fi

# ── Verification ─────────────────────────────────────────────────────
echo "## Verification"
echo ""
echo "Every archive includes a GPG detached signature (\`.asc\`) and three checksum files (SHA-512, BLAKE2b, SHAKE256)."
echo ""
echo "### GPG signature"
echo ""
echo '```bash'
echo "gpg --keyserver keyserver.ubuntu.com --recv-keys 47A50AEF4B65AAC2"
echo "gpg --verify cosmostrix-${TAG#v}-linux-amd64-v3-gnu.tar.gz.asc"
echo '```'
echo ""
echo "Expected: \`Good signature from \"Rezky Cahya Sahputra (cosmic dragon)\"\`"
echo ""
echo "Full verification instructions: [docs/VERIFY_RELEASE.md](docs/VERIFY_RELEASE.md)"
