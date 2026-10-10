#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).
#
#
# COSMOSTRIX CI ACTIONS VERSION SWEEP
#
# The engine of the actions-pin health contract (night-improve-8,
# zelynic NIGHT-improve-39/40 lineage): every `uses:` reference in
# the CI estate is read against the upstream release surface, and a
# stale pin heals itself. The estate's policy is "GitHub Actions
# float on their major tag" (docs/workflow/ABOUT_CI.md, Dependency
# version policy, owner decision 2026-08-30): minor and patch
# releases arrive through the moving tag automatically, and a new
# major is exactly the one-character review this sweep automates
# while keeping the human review seat (the healed diff is committed
# by a person, never pushed by a machine).
#
# The sweep contract, per uses: reference:
#   - a major-tag pin (@v6) bumps to the latest release's major tag
#     (v7) when a newer major exists; the moving major tag already
#     carries minor and patch, so a pin under the latest major is
#     CURRENT by policy, never "behind";
#   - an exact tag pin (@v6.0.1) is a deliberate pin: under the
#     latest major it stays (the moving tag carries the fixes), and
#     across a major boundary it heals to the latest release's exact
#     tag (v7.0.1) - the author's chosen form is preserved;
#   - a pin AHEAD of the latest release (a pre-release major, a
#     fork's own tag): a deliberate pin wins over the sweep
#     (reported, left alone).
#
# What the sweep never touches - the honesty rules:
#   - local actions (./...), docker:// refs, branch-style refs
#     (dtolnay/rust-toolchain@stable): not version pins, out of
#     scope by construction;
#   - SHA pins (40-hex refs): the estate's policy is moving tags,
#     so a SHA pin is a deliberate human decision - reported as
#     SKIP, healed by a human, never by a guess;
#   - a repo with neither a stable release nor semver tags:
#     reported as SKIP with the reason;
#   - anything outside .github/workflows/: the diff guard fails the
#     heal before the tree can be committed (the dependency-only-
#     diff discipline, applied to the CI surface).
#
# Version truth: GitHub releases/latest (stable releases only, no
# prereleases) with a /tags fallback for release-less repos. The
# per-repo answer is cached for the run: N workflow files quoting
# the same action cost one API read, not N.
#
# Usage:
#   scripts/ci/actions-version-sweep.sh --dry-run   # report only
#   scripts/ci/actions-version-sweep.sh --apply     # edit workflows
#
# --apply heals the workflow files in place and then proves, via
# guard_diff, that the heal only ever touched .github/workflows/.
# What happens next is deliberately manual: the maintainer reviews,
# stages, and commits the healed pins like any other change (the
# auto seat of check-actions-pins.sh stages them and stops the gate
# once for the same review).
#
# Environment:
#   GITHUB_TOKEN      optional; authenticated API reads (5000 req/h
#                     with the owner PAT, 60 req/h anonymous - the
#                     estate's worst case sweep is ~22 calls, so an
#                     anonymous run fits, but a shared egress IP can
#                     exhaust the anonymous ceiling mid-sweep). An
#                     unset GITHUB_TOKEN is auto-resolved from the
#                     machine's own credentials before the first API
#                     call - gh auth token, then the git credential
#                     helper for github.com, the same secret that
#                     pushes; no new standing secret is created
#                     (scripts/lib/github_token.sh, one
#                     implementation shared with the commit-time
#                     health check).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

# Resolve a token from the machine's own credentials (env -> gh ->
# git credential helper) so a machine that pushes fine never burns
# the anonymous ceiling mid-sweep. One lib, one implementation,
# shared with the commit-time health check; a machine offering no
# source stays anonymous, honestly.
# shellcheck source=scripts/lib/github_token.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/lib/github_token.sh"
GITHUB_TOKEN="${GITHUB_TOKEN:-$(resolve_github_token)}"
if [ -n "$GITHUB_TOKEN" ]; then
	export GITHUB_TOKEN
fi

API_ROOT="https://api.github.com"
MODE="dry-run"

# -- reporting ------------------------------------------------------------

HEALED=0
KEPT=0
SKIPPED=0

note() { echo "SWEEP: $*"; }
keep() { echo "KEEP:  $*"; }
skip() { echo "SKIP:  $*"; }

usage() {
	cat <<'EOF'
scripts/ci/actions-version-sweep.sh - the CI actions version sweep
(night-improve-8; the full design notes live in this file's header).

modes:
  --dry-run   report the verdict table, change nothing
  --apply     heal the workflow files in place (no git ops)
EOF
}

# -- GitHub API helpers ---------------------------------------------------

API_STATUS="000"

# api_get <path> <out_var> - GET $API_ROOT<path>, storing the body
# in the NAMED variable and the HTTP status in API_STATUS (000 means
# the call never completed - a network failure, distinct from any
# server answer). The body rides a printf -v instead of stdout so
# the status global survives: an api_get inside a $( ) subshell
# would silently eat the API_STATUS assignment, and every caller
# here reads the status right after the call. The retry contract is
# the repo's NIGHT-dinner-1 shape: a GitHub-side 5xx or a mid-sweep
# network blip re-attempts instead of producing a half-sweep.
api_get() {
	local path="$1" outvar="$2" raw status
	local args=(
		-sS --retry 3 --retry-all-errors --retry-delay 5
		--connect-timeout 15 --max-time 60
		-w '\n%{http_code}' "${API_ROOT}${path}"
	)
	if [ -n "${GITHUB_TOKEN:-}" ]; then
		args+=(-H "Authorization: Bearer ${GITHUB_TOKEN}")
	fi
	raw="$(curl "${args[@]}" 2>/dev/null || true)"
	status="${raw##*$'\n'}"
	API_STATUS="${status:-000}"
	printf -v "$outvar" '%s' "${raw%$'\n'*}"
}

# Per-repo caches: the latest stable version tag, and a verdict for
# repos the sweep cannot size up.
declare -A LATEST_TAG=()
declare -A REPO_VERDICT=()

# query_repo <repo> - fill LATEST_TAG (or REPO_VERDICT with the
# honest reason it stayed empty). releases/latest is primary; the
# newest semver tag is the fallback for release-less repos.
query_repo() {
	local repo="$1" body tag
	if [ -n "${LATEST_TAG[$repo]:-}" ] || [ -n "${REPO_VERDICT[$repo]:-}" ]; then
		return 0
	fi
	api_get "/repos/${repo}/releases/latest" body
	if [ "$API_STATUS" = "200" ]; then
		tag="$(printf '%s' "$body" | jq -r '.tag_name // empty')"
		if [ -n "$tag" ] && is_semver "$tag"; then
			LATEST_TAG[$repo]="$tag"
			return 0
		fi
	fi
	if [ "$API_STATUS" = "404" ] || [ "$API_STATUS" = "200" ]; then
		api_get "/repos/${repo}/tags?per_page=100" body
		if [ "$API_STATUS" = "200" ]; then
			tag="$(printf '%s' "$body" | jq -r '.[].name' |
				grep -E '^v?[0-9]+(\.[0-9]+)*$' |
				sort -V | tail -n 1 || true)"
			if [ -n "$tag" ]; then
				LATEST_TAG[$repo]="$tag"
				return 0
			fi
			REPO_VERDICT[$repo]="no releases and no semver tags"
			return 0
		fi
	fi
	REPO_VERDICT[$repo]="api unreachable (status ${API_STATUS})"
}

# -- version math ---------------------------------------------------------

is_semver() {
	printf '%s' "$1" | grep -Eq '^v?[0-9]+(\.[0-9]+)*$'
}

version_tuple() {
	local v="${1#v}"
	local maj min patch
	IFS='.' read -r maj min patch <<<"$v"
	printf '%s %s %s' "${maj:-0}" "${min:-0}" "${patch:-0}"
}

version_cmp() {
	local a_maj a_min a_pat b_maj b_min b_pat
	read -r a_maj a_min a_pat <<<"$(version_tuple "$1")"
	read -r b_maj b_min b_pat <<<"$(version_tuple "$2")"
	if [ "$a_maj" -lt "$b_maj" ]; then
		printf '%s\n' -1
	elif [ "$a_maj" -gt "$b_maj" ]; then
		printf '%s\n' 1
	elif [ "$a_min" -lt "$b_min" ]; then
		printf '%s\n' -1
	elif [ "$a_min" -gt "$b_min" ]; then
		printf '%s\n' 1
	elif [ "$a_pat" -lt "$b_pat" ]; then
		printf '%s\n' -1
	elif [ "$a_pat" -gt "$b_pat" ]; then
		printf '%s\n' 1
	else
		printf '%s\n' 0
	fi
}

major_of() {
	local maj _min _pat
	read -r maj _min _pat <<<"$(version_tuple "$1")"
	printf '%s' "$maj"
}

# -- the sweep ------------------------------------------------------------

declare -a EDIT_FILE=()
declare -a EDIT_LINE=()
declare -a EDIT_NEW=()

# The per-line edit ledger is grouped by file at apply time; every
# entry is a whole-line replacement built from the original line.
record_edit() {
	EDIT_FILE+=("$1")
	EDIT_LINE+=("$2")
	EDIT_NEW+=("$3")
}

sweep() {
	local file lineno content value action_part ref
	local owner name repo
	while IFS=: read -r file lineno content; do
		[ -n "${file:-}" ] || continue
		# Only `uses:` lines carry action references; the regex keeps
		# the optional list dash and captures the ref token plus the
		# trailing comment.
		if ! [[ $content =~ ^[[:space:]]*(-[[:space:]]+)?uses:[[:space:]]*([^[:space:]#]+)(.*)$ ]]; then
			continue
		fi
		value="${BASH_REMATCH[2]}"
		action_part="${value%@*}"
		ref="${value##*@}"
		if [[ $action_part == .* || $action_part == docker:* ]]; then
			skip "${value} - local/docker reference, not a version pin"
			SKIPPED=$((SKIPPED + 1))
			continue
		fi
		IFS='/' read -r owner name _rest <<<"$action_part"
		repo="${owner}/${name}"
		if ! [[ $repo =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]]; then
			skip "${value} - unparsable action reference"
			SKIPPED=$((SKIPPED + 1))
			continue
		fi

		if [[ $ref =~ ^[0-9a-fA-F]{40}$ ]]; then
			skip "${repo} - SHA pin: the estate floats on major tags by policy, a SHA pin is a deliberate human decision"
			SKIPPED=$((SKIPPED + 1))
		elif [[ $ref =~ ^v[0-9]+(\.[0-9]+)*$ ]]; then
			sweep_tag_pin "$file" "$lineno" "$content" \
				"$repo" "$action_part" "$ref" "$value"
		else
			skip "${repo}@${ref} - branch-style ref, not a version pin"
			SKIPPED=$((SKIPPED + 1))
		fi
	done < <(grep -nE '^[[:space:]]*(-[[:space:]]+)?uses:' .github/workflows/*.yml)
}

# sweep_tag_pin - the estate's own pin shape: a major tag (@v6) or an
# exact tag (@v6.0.1). Minor and patch ride the moving tag; only a
# newer major needs the sweep's help.
sweep_tag_pin() {
	local file="$1" lineno="$2" content="$3" repo="$4" action_part="$5"
	local ref="$6" value="$7"
	local latest cmp new_ref new_line

	query_repo "$repo"
	if [ -n "${REPO_VERDICT[$repo]:-}" ]; then
		skip "${repo} - ${REPO_VERDICT[$repo]}"
		SKIPPED=$((SKIPPED + 1))
		return 0
	fi
	latest="${LATEST_TAG[$repo]}"
	cmp="$(version_cmp "$ref" "$latest")"
	if [ "$cmp" = "1" ]; then
		keep "${repo}@${ref} - pinned ahead of latest release ${latest}; a deliberate pin wins over the sweep"
		KEPT=$((KEPT + 1))
		return 0
	fi
	if [ "$(major_of "$ref")" = "$(major_of "$latest")" ]; then
		keep "${repo}@${ref} - the moving tag already carries ${latest}"
		KEPT=$((KEPT + 1))
		return 0
	fi
	# Preserve the author's form: an exact tag pin goes exact, a
	# major-only tag pin goes major-only.
	if [[ $ref == *.* ]]; then
		new_ref="v${latest#v}"
	else
		new_ref="v$(major_of "$latest")"
	fi
	new_line="${content/${value}/${action_part}@${new_ref}}"
	record_edit "$file" "$lineno" "$new_line"
	note "${repo} (${file#./}:${lineno}) - ${ref} -> ${new_ref} HEAL"
	HEALED=$((HEALED + 1))
}

# -- apply / guard --------------------------------------------------------

apply_edits() {
	local i j f idx lines
	declare -A seen=()
	for i in "${!EDIT_FILE[@]}"; do
		f="${EDIT_FILE[$i]}"
		[ -z "${seen[$f]:-}" ] || continue
		seen[$f]=1
		lines=()
		mapfile -t lines <"$f"
		for j in "${!EDIT_FILE[@]}"; do
			[ "${EDIT_FILE[$j]}" = "$f" ] || continue
			idx=$((EDIT_LINE[j] - 1))
			lines[idx]="${EDIT_NEW[$j]}"
		done
		printf '%s\n' "${lines[@]}" >"$f"
	done
}

# guard_diff - the sweep may only move workflow files. Any tracked
# change elsewhere means the sweep (or the tree) is not what this
# script believes it is; fail before a commit can exist. Tracked
# changes only: an untracked file is never the sweep's doing
# (apply_edits writes only to workflow files), and a dirty local
# tree is its owner's business, not this guard's.
guard_diff() {
	local offender
	offender="$({
		git diff --name-only
		git diff --cached --name-only
	} |
		sort -u |
		grep -Ev '^\.github/workflows/[A-Za-z0-9_.-]+\.yml$' || true)"
	if [ -n "$offender" ]; then
		echo "FAIL: tracked files outside .github/workflows/ changed:"
		echo "$offender"
		exit 1
	fi
}

# -- main -----------------------------------------------------------------

case "${1:-}" in
--dry-run) MODE="dry-run" ;;
--apply) MODE="apply" ;;
-h | --help)
	usage
	exit 0
	;;
"") ;;
*)
	echo "unknown mode: $1" >&2
	usage >&2
	exit 2
	;;
esac

sweep
if [ "$MODE" != "dry-run" ] && [ "${#EDIT_FILE[@]}" -gt 0 ]; then
	apply_edits
fi

# The heal is proven sweep-only before the tree is handed to a
# human commit (the maintainer stages and commits it themselves).
if [ "$MODE" = "apply" ] && [ "${#EDIT_FILE[@]}" -gt 0 ]; then
	guard_diff
fi

echo ""
echo "sweep verdict: ${HEALED} healed, ${KEPT} kept, ${SKIPPED} skipped (mode: ${MODE})"
