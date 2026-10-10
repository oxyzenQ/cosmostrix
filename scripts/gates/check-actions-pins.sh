#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).
#
#
# COSMOSTRIX COMMIT-TIME ACTIONS-PIN HEALTH CHECK
#
# The actions-pin health contract's automatic seat (night-improve-8,
# zelynic NIGHT-improve-39/40 lineage). Two seats, one engine:
#   - gate-time, automatic: THIS CHECK - gate-keepers.sh check 20
#     (and the CI mirror, cosmic-dragon-guard.yml, which passes the
#     run's ephemeral Actions token) reads the pins' freshness
#     against upstream; current costs one line in warn/strict and
#     zero in auto, behind prints the classified table
#     (MAJOR / MINOR / PATCH), never silently
#   - on-demand, maintainer seat: scripts/ci/actions-version-sweep.sh
#     --dry-run to read, --apply to heal - the healed pins are
#     committed like any other change and this check re-reads them
#     (the same version truth and the same honesty rules, one
#     implementation; this script never duplicates that math)
#
# Advisory by default, and that is a design decision, not softness:
# the local quality gates fail closed because parity is a LOCAL fact
# this machine can always prove; pin freshness is a REMOTE fact that
# needs the network, the API's quota, and GitHub's answer right now.
# A gate that blocks a contributor on facts it cannot reach would be
# a dishonest gate (the sweep's own precedent: a push the platform
# provably refuses is never attempted). Strict contributors opt in
# with git config cosmostrix.actionsHealthCheck strict - that blocks
# the gate on a KNOWN-stale verdict, and only on a known one; every
# skip reason (offline, quota low, missing tool, overrun budget,
# indeterminate sweep) stays a skip, never a block.
#
# Speed contract: a gate run must not wait on the network twice. The
# verdict is cached under $GIT_DIR/cosmostrix/ keyed on the
# workflows' content hash (any workflow edit re-checks) with a TTL
# (upstream releases re-check even without edits). A cold check runs
# the sweep under a timeout budget; a check that overruns the budget
# skips with a note while a background refresh (--refresh-cache,
# internal) writes the cache for the next run. A cached hit costs
# zero API calls and needs no network at all.
#
# Output contract parsed from the sweep (stable since night-
# improve-8): the "sweep verdict: N healed, N kept, N skipped" line
# and the HEAL note lines "SWEEP: repo (file:line) - cur -> new ...
# HEAL". A verdict that does not parse is an honest skip, never a
# fake "current" - format drift fails safe.
#
# Modes (off | warn | strict | auto; default warn):
#   COSMOSTRIX_ACTIONS_HEALTH             one-shot, beats the git config
#   git config cosmostrix.actionsHealthCheck   repo-local, persistent
#
# Auto mode is skip-if-latest plus a contributor-carried auto-heal:
# a current verdict exits 0 with ZERO output (silence is the reward
# for healthy pins), and a clean-behind verdict (fully parsed, no
# api-unreachable/unresolvable - the rc=2 class never heals)
# re-verifies COLD (a cached behind is not a write mandate; upstream
# truth can move under a 6h-old cache), then runs the sweep's
# --apply under its own budget, stages .github/workflows/ into the
# index, and fails the gate ONCE: review git diff --cached, stage
# the heal into the pending commit (or commit it alone), re-run the
# gate - the re-run reads the healed pins as current and stays
# silent. No amend, no push: the manual flow's maintainer-review
# discipline is kept whole, only its typing is removed. A heal that
# cannot prove itself clean (API drop mid-apply, overrun,
# out-of-bounds write, unparsable result) stages NOTHING: the tree
# is restored from the pre-apply backup and the warn-mode table
# prints - remote facts never block a gate. The apply-stage proof is
# delta-shaped, not the sweep's guard_diff: at gate time the index
# legitimately carries the contributor's staged work, which
# guard_diff (a clean-tree contract) would flag by design - so the
# auto path snapshots the changed-file set and the non-workflow
# patch hash before and after the apply, and accepts the heal only
# when the sweep's own writes provably stayed inside
# .github/workflows/.
#
# Tunables (env, optional):
#   COSMOSTRIX_ACTIONS_HEALTH_TTL       cache seconds (default 21600 = 6h)
#   COSMOSTRIX_ACTIONS_HEALTH_TIMEOUT   cold-sweep budget seconds (default 30)
#   COSMOSTRIX_ACTIONS_HEAL_TIMEOUT     auto-mode apply budget seconds (default 60)
#   GITHUB_TOKEN                        authenticated API reads (the sweep
#                                       picks it up too; auto-resolved from
#                                       the machine's own credentials when
#                                       unset - gh auth token, then the git
#                                       credential helper). The estate's
#                                       worst-case sweep is ~22 calls, so
#                                       the anonymous 60/h tier fits, but a
#                                       shared egress IP can exhaust it
#                                       mid-sweep - a resolved token removes
#                                       that ceiling
#
# Usage:
#   bash scripts/gates/check-actions-pins.sh                # the check
#   bash scripts/gates/check-actions-pins.sh --refresh-cache # internal

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

# The shared token resolution (env -> gh -> git credential helper)
# lives in one lib so this check and the sweep it drives never
# disagree about whose credential pays for the API reads. Resolved
# inside preflight() - the one seat that touches the network - so a
# cache hit (zero network) also costs zero probes.
# shellcheck source=scripts/lib/github_token.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/lib/github_token.sh"
SWEEP_SCRIPT="scripts/ci/actions-version-sweep.sh"
API_ROOT="https://api.github.com"
DEFAULT_TTL=21600
DEFAULT_TIMEOUT=30
DEFAULT_HEAL_TIMEOUT=60
# The caller's hourly API ceiling, filled by preflight from the same
# /rate_limit read (empty when the field does not parse). Floor
# sizing for this estate: 11 distinct action repos today, ~2 calls
# each in the worst case (releases/latest plus a tags fallback), plus
# the probe itself - 40 is roughly 1.7x that worst case, so the
# anonymous 60/h tier CAN cover a cold sweep here (zelynic's 70-call
# floor could not), and the quota messaging stays generic.
QUOTA_FLOOR=40
QUOTA_LIMIT=""

# -- output helpers -------------------------------------------------------

health() { echo "HEALTH: $*"; }
skip_check() {
	health "skipped - $*"
	exit 0
}

usage() {
	cat <<'EOF'
scripts/gates/check-actions-pins.sh - the gate-time actions-pin
health check (night-improve-8; the contract story lives in this
file's header and in gate-keepers.sh, check 20).

modes (off | warn | strict | auto, default warn):
  COSMOSTRIX_ACTIONS_HEALTH=strict bash scripts/gate-keepers.sh   one-shot
  git config cosmostrix.actionsHealthCheck strict                 repo-local

auto: current exits 0 silent; a clean-behind verdict re-verifies
  cold, heals via the sweep's --apply, stages .github/workflows/,
  and fails the gate once for review (the re-run reads the healed
  pins as current and stays silent).

tunables (env):
  COSMOSTRIX_ACTIONS_HEALTH_TTL       cache seconds (default 21600)
  COSMOSTRIX_ACTIONS_HEALTH_TIMEOUT   cold-sweep budget (default 30s)
  COSMOSTRIX_ACTIONS_HEAL_TIMEOUT     auto-apply budget (default 60s)
  GITHUB_TOKEN                        a PAT (auto-resolved from gh or
                                      the git credential helper when unset)
EOF
}

# -- environment readers --------------------------------------------------

resolve_mode() {
	local mode
	if [ -n "${COSMOSTRIX_ACTIONS_HEALTH:-}" ]; then
		mode="$COSMOSTRIX_ACTIONS_HEALTH"
	else
		mode="$(git config --get cosmostrix.actionsHealthCheck 2>/dev/null || true)"
	fi
	case "$mode" in
	off | warn | strict | auto) printf '%s' "$mode" ;;
	"") printf 'warn' ;;
	*)
		# stderr, not stdout: this function is consumed by a
		# command substitution, and a stdout note here would
		# be captured into the mode variable itself (an
		# unknown-mode note that corrupts the mode it warns
		# about - the exact fixture bug the zelynic lineage
		# caught and documented)
		health "unknown mode '${mode}' - treating as warn" >&2
		printf 'warn'
		;;
	esac
}

ttl_seconds() {
	local t="${COSMOSTRIX_ACTIONS_HEALTH_TTL:-$DEFAULT_TTL}"
	[[ $t =~ ^[0-9]+$ ]] || t="$DEFAULT_TTL"
	printf '%s' "$t"
}

timeout_seconds() {
	local t="${COSMOSTRIX_ACTIONS_HEALTH_TIMEOUT:-$DEFAULT_TIMEOUT}"
	[[ $t =~ ^[0-9]+$ ]] || t="$DEFAULT_TIMEOUT"
	printf '%s' "$t"
}

heal_timeout_seconds() {
	local t="${COSMOSTRIX_ACTIONS_HEAL_TIMEOUT:-$DEFAULT_HEAL_TIMEOUT}"
	[[ $t =~ ^[0-9]+$ ]] || t="$DEFAULT_HEAL_TIMEOUT"
	printf '%s' "$t"
}

cache_file() {
	printf '%s/cosmostrix/actions-pins.cache' "$(git rev-parse --git-dir)"
}

workflows_hash() {
	local files
	files=(.github/workflows/*.yml)
	sha256sum "${files[@]}" | sha256sum | cut -d' ' -f1
}

# -- version math (classification only; the sweep owns the truth) --------

bump_class() {
	local cur="$1" new="$2"
	local c1 c2 c3 n1 n2 n3
	IFS='.' read -r c1 c2 c3 <<<"${cur#v}"
	IFS='.' read -r n1 n2 n3 <<<"${new#v}"
	[[ ${c1:-x} =~ ^[0-9]+$ && ${n1:-x} =~ ^[0-9]+$ ]] || {
		printf 'MOVE'
		return 0
	}
	if [ "${c1:-0}" -ne "${n1:-0}" ]; then
		printf 'MAJOR'
	elif [ "${c2:-0}" -ne "${n2:-0}" ]; then
		printf 'MINOR'
	elif [ "${c3:-0}" -ne "${n3:-0}" ]; then
		printf 'PATCH'
	else
		printf 'RE-PIN'
	fi
}

age_label() {
	local age="$1"
	if [ "$age" -lt 90 ]; then
		printf 'checked just now'
	elif [ "$age" -lt 5400 ]; then
		printf 'cache %dm old' "$(((age + 59) / 60))"
	else
		printf 'cache %dh old' "$(((age + 3599) / 3600))"
	fi
}

# -- verdict rendering ----------------------------------------------------
#
# render_verdict <payload-file> <age-label> <mode>
#   returns 0 = current, 1 = behind, 2 = indeterminate

RE_HEAL='^SWEEP: ([^ ]+) \([^)]+\) - ([^ ,]+) -> ([^ ,]+)'
RE_VERDICT='^sweep verdict: ([0-9]+) healed, ([0-9]+) kept, ([0-9]+) skipped'

render_verdict() {
	local payload="$1" label="$2" mode="$3"
	local line repo cur new key uses cls
	local healed kept skipped
	local -A seen=()
	local -a keys=()
	local major_n=0 minor_n=0 patch_n=0 repin_n=0 move_n=0 pin_n=0

	healed=""
	kept=0
	skipped=0

	# Honesty first: any pin the sweep could not size up makes the
	# whole verdict unknown - "api unreachable" (quota, network) and
	# "unresolvable" (a tag that would not resolve) never read as
	# "current".
	if grep -Eq 'api unreachable|unresolvable' "$payload"; then
		health "skipped - the sweep could not size up every pin (API unreachable mid-sweep); no verdict"
		return 2
	fi

	while IFS= read -r line; do
		if [[ $line =~ $RE_HEAL ]] && [[ $line == *" HEAL"* ]]; then
			repo="${BASH_REMATCH[1]}"
			cur="${BASH_REMATCH[2]}"
			new="${BASH_REMATCH[3]}"
			key="${repo}|${cur}|${new}"
			if [ -n "${seen[$key]:-}" ]; then
				seen[$key]=$((seen[$key] + 1))
			else
				seen[$key]=1
				keys+=("$key")
			fi
		elif [[ $line =~ $RE_VERDICT ]]; then
			healed="${BASH_REMATCH[1]}"
			kept="${BASH_REMATCH[2]}"
			skipped="${BASH_REMATCH[3]}"
		fi
	done <"$payload"

	if [ -z "$healed" ]; then
		health "skipped - the sweep's verdict line did not parse; no verdict"
		return 2
	fi
	if [ "$healed" -gt 0 ] && [ "${#keys[@]}" -eq 0 ]; then
		health "skipped - the sweep's HEAL lines did not parse; no verdict"
		return 2
	fi

	if [ "$healed" -eq 0 ]; then
		if [ "$((kept + skipped))" -gt 0 ]; then
			health "CI actions pins current - ${kept} kept, ${skipped} skipped-for-cause (${label})"
		else
			health "CI actions pins current - no version pins found (${label})"
		fi
		return 0
	fi

	# The classified table: one row per distinct pin move, with the
	# pin count when the same action is pinned in many files.
	local repo_w=0 cur_w=0 new_w=0 r c n
	for key in "${keys[@]}"; do
		IFS='|' read -r r c n <<<"$key"
		[ "${#r}" -gt "$repo_w" ] && repo_w="${#r}"
		[ "${#c}" -gt "$cur_w" ] && cur_w="${#c}"
		[ "${#n}" -gt "$new_w" ] && new_w="${#n}"
	done
	if [ "$mode" = "strict" ]; then
		health "FAIL - CI actions pins are behind upstream (strict mode; the gate run is blocked):"
	else
		health "WARNING - CI actions pins are behind upstream (advisory mode; the gate run proceeds):"
	fi
	echo ""
	for key in "${keys[@]}"; do
		IFS='|' read -r r c n <<<"$key"
		uses="${seen[$key]}"
		cls="$(bump_class "$c" "$n")"
		pin_n=$((pin_n + uses))
		move_n=$((move_n + 1))
		case "$cls" in
		MAJOR) major_n=$((major_n + 1)) ;;
		MINOR) minor_n=$((minor_n + 1)) ;;
		PATCH) patch_n=$((patch_n + 1)) ;;
		RE-PIN) repin_n=$((repin_n + 1)) ;;
		esac
		printf '    %-*s  %-*s -> %-*s  [%s]' "$repo_w" "$r" "$cur_w" "$c" "$new_w" "$n" "$cls"
		[ "$uses" -gt 1 ] && printf '  (%d pins)' "$uses"
		echo ""
	done
	local repin_note=""
	[ "$repin_n" -gt 0 ] && repin_note=" (+${repin_n} SHA re-pin)"
	echo ""
	health "${pin_n} pins behind across ${move_n} moves - ${major_n} MAJOR, ${minor_n} MINOR, ${patch_n} PATCH${repin_note}"
	if [ "$mode" = "strict" ]; then
		echo ""
		echo "    heal and re-run:     scripts/ci/actions-version-sweep.sh --apply"
		echo "    or update first:     git pull (a contributor may have healed them on origin)"
		echo "    one-shot advisory:   COSMOSTRIX_ACTIONS_HEALTH=warn bash scripts/gate-keepers.sh"
		echo "    skip this check:     COSMOSTRIX_ACTIONS_HEALTH=off bash scripts/gate-keepers.sh"
	else
		echo ""
		echo "    heal locally:        scripts/ci/actions-version-sweep.sh --apply"
		echo "    or update first:     git pull (a contributor may have healed them on origin)"
		echo "    make it blocking:    git config cosmostrix.actionsHealthCheck strict"
		echo "    silence for once:    COSMOSTRIX_ACTIONS_HEALTH=off bash scripts/gate-keepers.sh"
	fi
	return 1
}

# -- the auto mode (skip-if-latest, auto-heal) ----------------------------
#
# Current is silent, a clean-behind verdict heals through a
# contributor's re-run, and every other shape keeps the warn-mode
# honesty. The write side never trusts a cached behind (a heal
# decision re-reads today's upstream truth first, cold_check) and
# never trusts a partial or out-of-bounds apply (the heal is staged
# only when the sweep provably reached its write stage and its
# writes provably stayed inside .github/workflows/, auto_heal).

# cold_check - one cold re-verify for the write decision: pre-flight,
# then the sweep under the READ budget. Echoes render_verdict's
# output (the caller decides what to show) and returns its rc, or 3
# when no verdict could be produced (unreachable, quota, overrun -
# the skip class, never a heal trigger). A verdict it did pin
# (current or behind) is written to the cache, the same read
# contract as the main cold path; rc=2 stays uncached exactly as
# there.
cold_check() {
	local payload rc
	rc=0
	preflight || rc=$?
	if [ "$rc" -eq 1 ]; then
		health "re-verify: GitHub API unreachable (offline or blocked${PREFLIGHT_NOTE:+ - ${PREFLIGHT_NOTE}}); pins unchecked"
		return 3
	elif [ "$rc" -eq 2 ]; then
		if [ -n "$QUOTA_LIMIT" ] && [ "$QUOTA_LIMIT" -lt "$QUOTA_FLOOR" ]; then
			if [ -n "${GITHUB_TOKEN:-}" ]; then
				health "re-verify: API ceiling ${QUOTA_LIMIT}/h under the resolved token can never cover the ${QUOTA_FLOOR}-call cold-sweep floor"
			else
				health "re-verify: API ceiling ${QUOTA_LIMIT}/h can never cover the ${QUOTA_FLOOR}-call cold-sweep floor - no token resolved (export GITHUB_TOKEN, gh auth login, or store a github.com credential)"
			fi
		else
			health "re-verify: GitHub API quota below ${QUOTA_FLOOR} calls; retry after the quota resets"
		fi
		return 3
	fi
	payload="$(mktemp /tmp/cosmostrix-health-reverify.XXXXXX)"
	if timeout "$(timeout_seconds)" bash "$SWEEP_SCRIPT" --dry-run >"$payload" 2>&1; then
		rc=0
		render_verdict "$payload" "checked just now" warn || rc=$?
		if [ "$rc" -ne 2 ]; then
			write_cache "$payload"
		fi
		rm -f "$payload"
		return "$rc"
	fi
	rm -f "$payload"
	health "re-verify: the sweep overran its $(timeout_seconds)s budget; refreshing in the background"
	mkdir -p "$(dirname "$(cache_file)")"
	nohup bash "$REPO_ROOT/scripts/gates/check-actions-pins.sh" --refresh-cache >/dev/null 2>&1 &
	return 3
}

# auto_cached_verdict <payload-file> <age-label> - the cache-hit seat.
# Current: silent. Indeterminate: the skip note. Clean-behind: today's
# truth first - a cached behind is a read, not a write mandate.
auto_cached_verdict() {
	local payload="$1" label="$2" out rc cold_rc cold_out
	rc=0
	out="$(render_verdict "$payload" "$label" warn)" || rc=$?
	rm -f "$payload"
	if [ "$rc" -eq 0 ]; then
		exit 0
	fi
	if [ "$rc" -eq 2 ]; then
		printf '%s\n' "$out"
		exit 0
	fi
	health "auto: the cache says behind - re-verifying cold before any heal"
	cold_rc=0
	cold_out="$(cold_check)" || cold_rc=$?
	case "$cold_rc" in
	0)
		health "auto: the re-verify says current - the cached behind was stale; nothing to heal"
		exit 0
		;;
	2)
		printf '%s\n' "$cold_out"
		exit 0
		;;
	1) auto_heal "$cold_out" ;;
	*)
		printf '%s\n' "$cold_out"
		printf '%s\n' "$out"
		exit 0
		;;
	esac
}

# auto_cold_verdict <payload-file> - the cold seat: the verdict is
# already today's truth, so current is silent (the verdict still
# lands in the cache for the next run), indeterminate prints its
# skip note, and clean-behind heals immediately.
auto_cold_verdict() {
	local payload="$1" out rc
	rc=0
	out="$(render_verdict "$payload" "checked just now" warn)" || rc=$?
	if [ "$rc" -eq 2 ]; then
		printf '%s\n' "$out"
		rm -f "$payload"
		exit 0
	fi
	write_cache "$payload"
	rm -f "$payload"
	if [ "$rc" -eq 0 ]; then
		exit 0
	fi
	auto_heal "$out"
}

# auto_heal <warn-shape output for the fallback> - the write seat.
#
# Applies the sweep under its own budget (COSMOSTRIX_ACTIONS_HEAL_TIMEOUT,
# default 60s), stages .github/workflows/ into the index, and fails
# the gate ONCE: the contributor reviews `git diff --cached` and
# re-runs; the re-run reads the healed pins as current and stays
# silent. No amend, no push - the manual flow's maintainer-review
# discipline is kept whole, only its typing removed.
#
# The sweep's own guard_diff cannot arbitrate here: it compares the
# whole tree against HEAD, and at gate time the index legitimately
# carries the contributor's staged work - guard_diff flags that BY
# DESIGN (its contract is the manual heal on a clean tree). This
# path substitutes the same discipline in delta form: pre/post
# snapshots prove the sweep's own writes stayed inside
# .github/workflows/ (the newly-changed set plus the non-workflow
# patch hash), the workflow files are backed up so every failure
# shape restores the tree exactly, and the heal is staged only when
# the sweep provably reached its write stage - the verdict line and
# guard_diff's own FAIL marker both print only after apply_edits
# ran, which separates a completed apply from a sweep that died
# mid-run (timeout, network) and must never be staged.
#
# Exits: 1 = healed and staged (the gate fails once on purpose);
# 0 = fallback (advisory table) or the verdict moved to current.
auto_heal() {
	local warn_table="$1"
	local backup pre_list post_list heal_out
	local rc heals
	local delta offenders nonwf_pre nonwf_post applied

	backup="$(mktemp -d /tmp/cosmostrix-heal-backup.XXXXXX)"
	cp .github/workflows/*.yml "$backup/"
	pre_list="$(mktemp /tmp/cosmostrix-heal-pre.XXXXXX)"
	post_list="$(mktemp /tmp/cosmostrix-heal-post.XXXXXX)"
	heal_out="$(mktemp /tmp/cosmostrix-heal-apply.XXXXXX)"

	# Nested helpers (bash dynamic scope: they read this frame's
	# locals - the backup, the delta, the fallback table).
	heal_cleanup() {
		rm -rf "$backup"
		rm -f "$pre_list" "$post_list" "$heal_out"
	}
	heal_abort() {
		local reason="$1"
		# Restore the pre-apply tree exactly: workflows from the
		# backup (a contributor's unstaged workflow edits ride back
		# too); out-of-bounds files were clean pre-apply by delta
		# definition, so checkout returns them untouched.
		cp "$backup"/*.yml .github/workflows/ 2>/dev/null || true
		[ -z "$offenders" ] || printf '%s\n' "$offenders" | xargs -r git checkout -- 2>/dev/null || true
		heal_cleanup
		health "auto-heal aborted - ${reason}; nothing staged, the advisory verdict follows"
		printf '%s\n' "$warn_table"
		exit 0
	}
	heal_moved() {
		local moved_rc=0 moved_out
		moved_out="$(render_verdict "$heal_out" "checked just now" warn)" || moved_rc=$?
		heal_cleanup
		if [ "$moved_rc" -eq 0 ]; then
			health "auto: the apply re-read upstream and found the pins current - the dry-run's verdict was stale, nothing to heal"
			exit 0
		fi
		printf '%s\n' "$moved_out"
		printf '%s\n' "$warn_table"
		exit 0
	}
	heal_succeed() {
		if ! git add .github/workflows/; then
			heal_abort "staging the healed workflows failed (git add)"
		fi
		heal_cleanup
		health "auto-heal: ${heals} pin(s) healed into the index - this gate run was stopped ONCE on purpose"
		health "review git diff --cached, land the heal with the pending commit (or alone), then re-run the gate"
		health "the re-run reads the healed pins as current and stays silent"
		health "prefer the manual lane instead: COSMOSTRIX_ACTIONS_HEALTH=warn, then scripts/ci/actions-version-sweep.sh --apply"
		exit 1
	}

	{
		git diff --name-only
		git diff --cached --name-only
	} | sort -u >"$pre_list"
	nonwf_pre="$(git diff -- . ':(exclude).github/workflows/' | sha256sum | cut -d' ' -f1)"

	rc=0
	timeout "$(heal_timeout_seconds)" bash "$SWEEP_SCRIPT" --apply >"$heal_out" 2>&1 || rc=$?

	{
		git diff --name-only
		git diff --cached --name-only
	} | sort -u >"$post_list"
	nonwf_post="$(git diff -- . ':(exclude).github/workflows/' | sha256sum | cut -d' ' -f1)"
	delta="$(comm -13 "$pre_list" "$post_list")"
	offenders="$(printf '%s' "$delta" | grep -Ev '^\.github/workflows/[A-Za-z0-9_.-]+\.yml$' || true)"
	heals="$(grep -c ' HEAL' "$heal_out" || true)"
	heals="${heals:-0}"
	applied=""
	if grep -q '^sweep verdict:' "$heal_out"; then
		applied="verdict"
	elif grep -q '^FAIL: tracked files outside' "$heal_out"; then
		applied="guard"
	fi

	if grep -Eq 'api unreachable|unresolvable' "$heal_out"; then
		heal_abort "the sweep could not size up every pin mid-apply (API unreachable)"
	elif [ -z "$applied" ]; then
		heal_abort "the apply never reached its write stage (budget $(heal_timeout_seconds)s, network, or an early crash)"
	elif [ "$heals" -eq 0 ]; then
		# A completed apply with zero heals: upstream moved - the
		# apply's own payload is the verdict now, not the dry's.
		heal_moved
	elif [ -n "$offenders" ]; then
		heal_abort "the sweep's writes left .github/workflows/ (the offenders were restored)"
	elif [ "$nonwf_pre" != "$nonwf_post" ]; then
		heal_abort "the non-workflow tree changed under the apply"
	elif [ -z "$delta" ]; then
		heal_abort "the apply completed but the tree shows no pin changes"
	else
		heal_succeed
	fi
}

# -- network pre-flight ---------------------------------------------------
#
# preflight: 0 = API reachable with quota, 1 = unreachable,
# 2 = quota below the floor. /rate_limit does not count against the
# core quota; the floor (40) covers a cold sweep's worst case (the
# estate's 11 distinct action repos at a releases/latest read plus a
# tags fallback each). The ceiling rides the same response, so the
# skip notes can name the real numbers instead of guessing.

preflight() {
	local body status remaining limit
	# Resolve the machine's own token on the one path that pays for
	# the network (env -> gh -> git credential helper, never a new
	# standing secret) and hand it to every child the sweep spawns;
	# a cache hit never reaches here.
	if [ -z "${GITHUB_TOKEN:-}" ]; then
		GITHUB_TOKEN="$(resolve_github_token)"
		if [ -n "$GITHUB_TOKEN" ]; then
			export GITHUB_TOKEN
		fi
	fi
	PREFLIGHT_NOTE=""
	local args=(-sS --connect-timeout 5 --max-time 8 -w '\n%{http_code}')
	args+=("${API_ROOT}/rate_limit")
	if [ -n "${GITHUB_TOKEN:-}" ]; then
		args+=(-H "Authorization: Bearer ${GITHUB_TOKEN}")
	fi
	body="$(curl "${args[@]}" 2>/dev/null || true)"
	status="${body##*$'\n'}"
	status="${status:-000}"
	# The unreachable note names the shape: "no HTTP answer" is a
	# network fact, HTTP 401 names a rejected token, any other code
	# names the middlebox - a bare "offline or blocked" leaves the
	# owner guessing which one.
	if [ "$status" = "000" ]; then
		PREFLIGHT_NOTE="no HTTP answer - connection failed, DNS, or a blocking middlebox"
	elif [ "$status" = "401" ] && [ -n "${GITHUB_TOKEN:-}" ]; then
		PREFLIGHT_NOTE="HTTP 401 - the resolved token was rejected (check GITHUB_TOKEN, gh auth, or the git credential helper)"
	elif [ "$status" != "200" ]; then
		PREFLIGHT_NOTE="HTTP ${status}"
	fi
	[ "$status" = "200" ] || return 1
	remaining="$(printf '%s' "${body%$'\n'*}" | jq -r '.resources.core.remaining // empty' 2>/dev/null || true)"
	[[ $remaining =~ ^[0-9]+$ ]] || return 0
	limit="$(printf '%s' "${body%$'\n'*}" | jq -r '.resources.core.limit // empty' 2>/dev/null || true)"
	if [[ $limit =~ ^[0-9]+$ ]]; then
		QUOTA_LIMIT="$limit"
	fi
	[ "$remaining" -ge "$QUOTA_FLOOR" ] || return 2
	return 0
}

# -- cache ----------------------------------------------------------------
#
# One file: a "epoch<TAB>workflows-hash" meta line, then the sweep
# payload. Writes are atomic (tmp + mv) so a background refresh
# never hands a reader a half-written verdict.

write_cache() {
	local payload="$1" epoch hash tmp
	epoch="$(date +%s)"
	hash="$(workflows_hash)"
	tmp="$(cache_file).tmp.$$"
	mkdir -p "$(dirname "$(cache_file)")"
	{
		printf '%s\t%s\n' "$epoch" "$hash"
		cat "$payload"
	} >"$tmp"
	mv "$tmp" "$(cache_file)"
}

# -- the background refresh (internal) ------------------------------------

refresh_cache() {
	local payload rc
	payload="$(mktemp /tmp/cosmostrix-health-refresh.XXXXXX)"
	trap 'rm -f "$payload"' EXIT
	preflight || exit 0
	rc=0
	bash "$SWEEP_SCRIPT" --dry-run >"$payload" 2>&1 || rc=$?
	if [ "$rc" -eq 0 ] && ! grep -Eq 'api unreachable|unresolvable' "$payload"; then
		write_cache "$payload"
	fi
	exit 0
}

# -- main -----------------------------------------------------------------

main() {
	local mode hash epoch c_hash age payload rc
	case "${1:-}" in
	--refresh-cache) refresh_cache ;;
	-h | --help)
		usage
		exit 0
		;;
	"") ;;
	*)
		echo "unknown argument: $1 (this gate takes none; --refresh-cache is internal)" >&2
		exit 2
		;;
	esac

	# Guards: every one of these is a skip, never a failure - a
	# contributor's gate run never dies because a health probe could
	# not run.
	command -v git >/dev/null 2>&1 || skip_check "git not found"
	git rev-parse --git-dir >/dev/null 2>&1 || skip_check "not inside a git work tree"
	local tools_missing=""
	for tool in curl jq sha256sum timeout; do
		command -v "$tool" >/dev/null 2>&1 || tools_missing="${tools_missing} ${tool}"
	done
	[ -z "$tools_missing" ] || skip_check "missing tools:${tools_missing} (the check needs curl, jq, sha256sum, timeout)"
	[ -f "$SWEEP_SCRIPT" ] || skip_check "the sweep engine ${SWEEP_SCRIPT} is not in this tree"
	shopt -s nullglob
	local workflows=(.github/workflows/*.yml)
	shopt -u nullglob
	[ "${#workflows[@]}" -gt 0 ] || skip_check "no .github/workflows/*.yml in this tree"

	mode="$(resolve_mode)"
	[ "$mode" = "off" ] && exit 0

	# Cache hit: same workflows content, within the TTL. Costs zero
	# API calls and no network.
	hash="$(workflows_hash)"
	if [ -f "$(cache_file)" ]; then
		local meta
		meta="$(head -n 1 "$(cache_file)" 2>/dev/null || true)"
		epoch="${meta%%$'\t'*}"
		c_hash="${meta##*$'\t'}"
		if [[ $epoch =~ ^[0-9]+$ ]] && [ "$c_hash" = "$hash" ]; then
			age=$(($(date +%s) - epoch))
			if [ "$age" -lt "$(ttl_seconds)" ]; then
				payload="$(mktemp /tmp/cosmostrix-health-hit.XXXXXX)"
				trap 'rm -f "$payload"' EXIT
				tail -n +2 "$(cache_file)" >"$payload"
				if [ "$mode" = "auto" ]; then
					auto_cached_verdict "$payload" "$(age_label "$age")"
				fi
				rc=0
				render_verdict "$payload" "$(age_label "$age")" "$mode" || rc=$?
				if [ "$rc" -eq 1 ] && [ "$mode" = "strict" ]; then
					exit 1
				fi
				exit 0
			fi
		fi
	fi

	# Cold check: pre-flight, then the sweep under a budget.
	rc=0
	preflight || rc=$?
	if [ "$rc" -eq 1 ]; then
		skip_check "GitHub API unreachable (offline or blocked${PREFLIGHT_NOTE:+ - ${PREFLIGHT_NOTE}}); pins unchecked"
	elif [ "$rc" -eq 2 ]; then
		if [ -n "$QUOTA_LIMIT" ] && [ "$QUOTA_LIMIT" -lt "$QUOTA_FLOOR" ]; then
			if [ -n "${GITHUB_TOKEN:-}" ]; then
				skip_check "API ceiling ${QUOTA_LIMIT}/h under the resolved token can never cover the ${QUOTA_FLOOR}-call cold-sweep floor"
			else
				skip_check "API ceiling ${QUOTA_LIMIT}/h can never cover the ${QUOTA_FLOOR}-call cold-sweep floor - no token resolved (export GITHUB_TOKEN, gh auth login, or store a github.com credential in the git credential helper)"
			fi
		fi
		skip_check "GitHub API quota below ${QUOTA_FLOOR} calls; pins unchecked (retry after the quota resets${GITHUB_TOKEN:+ under the resolved token})"
	fi

	payload="$(mktemp /tmp/cosmostrix-health-cold.XXXXXX)"
	trap 'rm -f "$payload"' EXIT
	if timeout "$(timeout_seconds)" bash "$SWEEP_SCRIPT" --dry-run >"$payload" 2>&1; then
		rc=0
		if [ "$mode" = "auto" ]; then
			auto_cold_verdict "$payload"
		fi
		render_verdict "$payload" "checked just now" "$mode" || rc=$?
		if [ "$rc" -eq 2 ]; then
			exit 0
		fi
		write_cache "$payload"
		if [ "$rc" -eq 1 ] && [ "$mode" = "strict" ]; then
			exit 1
		fi
		exit 0
	fi
	# Overran the budget (or the sweep itself failed): skip loudly,
	# refresh in the background so the next run reads a verdict.
	health "skipped - the sweep overran its $(timeout_seconds)s budget; refreshing in the background, the next run reads the verdict"
	mkdir -p "$(dirname "$(cache_file)")"
	nohup bash "$REPO_ROOT/scripts/gates/check-actions-pins.sh" --refresh-cache >/dev/null 2>&1 &
	exit 0
}

main "$@"
