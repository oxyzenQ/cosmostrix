#!/usr/bin/env bash

# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
# PLATFORM: Linux-only (reads /proc/<pid>/status and /proc/<pid>/fd —
#   the RSS and fd census need the Linux procfs; macOS/BSD have no
#   /proc with these files). CI runs it on ubuntu-24.04.

# night-frontier-Z6, S5: the determinism-anchored soak harness.
#
# One long-lived cosmostrix process renders headless for N minutes
# while the harness wraps the whole window in a determinism anchor:
# the replay oracle's digest (docs/INNOVATIONS.md entry 32) is taken
# at t=0 and again after the soak, and the two must be byte-identical
# — determinism used as the drift canary (a wall-clock-independent
# replay must not move across the window). Inside the window the
# harness censuses /proc: RSS samples, fd count, thread count.
#
# Pass law (all four, no exceptions):
#   1. digest:   t=0 and post-soak replay digests byte-identical.
#   2. rss:      linear-regression growth over the final 80% of the
#                window below 2%/hour of the median RSS (the
#                docs/ENDURANCE.md acceptance line). Verdict SHORT on
#                windows under 30 minutes — a trend needs a trend's
#                worth of data.
#   3. fd:       max-min across the census <= 2 (transient tolerance).
#   4. exit:     the soak process exits 0 and its log carries no
#                panic.
#
# The 24h horizon: the bench parser accepts up to `24h` exactly (the
# DURATION_MAX_SECS ceiling, src/cli/cli_parse.rs), so a self-hosted
# runner with a 35-day job cap can run the real 24-hour window via
# --minutes 1440. GitHub-hosted runners cap a job at 6 hours, so the
# scheduled lane (CI) defaults to a bounded leg (150 minutes) — the
# design target stays the 24h anchor, the hosted profile realizes a
# bounded leg of it, and both realize the same pass law.
#
# Usage: scripts/depth/soak_determinism.sh [--minutes N] [--bin PATH]
#        [--interval S] [--log-dir DIR]
#   --minutes N   soak window length in minutes (default 150; 1440 = the 24h anchor)
#   --bin PATH    the cosmostrix binary to soak (default target/release/cosmostrix)
#   --interval S  census sampling interval in seconds (default 15)
#   --log-dir DIR where the CSV + soak log land (default logs/)
#
# Exit codes: 0 = PASS, 1 = FAIL, 2 = usage/setup error.

set -euo pipefail

SOAK_MINUTES=150
SOAK_BIN="target/release/cosmostrix"
SOAK_INTERVAL=15
SOAK_LOG_DIR="logs"

while [[ $# -gt 0 ]]; do
	case "$1" in
		--minutes)
			SOAK_MINUTES="${2:?--minutes needs a value}"
			shift 2
			;;
		--bin)
			SOAK_BIN="${2:?--bin needs a value}"
			shift 2
			;;
		--interval)
			SOAK_INTERVAL="${2:?--interval needs a value}"
			shift 2
			;;
		--log-dir)
			SOAK_LOG_DIR="${2:?--log-dir needs a value}"
			shift 2
			;;
		*)
			echo "usage: $0 [--minutes N] [--bin PATH] [--interval S] [--log-dir DIR]" >&2
			exit 2
			;;
	esac
done

if [[ ! -x "$SOAK_BIN" ]]; then
	echo "soak-determinism: binary not found or not executable: $SOAK_BIN" >&2
	echo "soak-determinism: build it first (cargo build --profile release) or pass --bin" >&2
	exit 2
fi
if ! command -v awk >/dev/null 2>&1; then
	echo "soak-determinism: awk is required for the RSS regression" >&2
	exit 2
fi

SOAK_SECS=$((SOAK_MINUTES * 60))
mkdir -p "$SOAK_LOG_DIR"
STAMP="$(date +%Y%m%d_%H%M%S)"
CSV_PATH="$SOAK_LOG_DIR/soak_${STAMP}.csv"
LOG_PATH="$SOAK_LOG_DIR/soak_${STAMP}.log"

# ── The t=0 determinism anchor ────────────────────────────────────────────
# The replay oracle renders its pinned sequence in a fresh process and
# prints one folded digest line; anything that drifts the render output
# (libm state, environment, time-dependent code) shows up here.
DIGEST_T0="$("$SOAK_BIN" --bench-replay 2>/dev/null | grep -o 'replay-digest: b3:[0-9a-f]*' || true)"
if [[ -z "$DIGEST_T0" ]]; then
	echo "soak-determinism: FAIL - the t=0 anchor produced no replay digest" >&2
	exit 1
fi
echo "soak-determinism: anchor t=0: $DIGEST_T0"

# ── The soak leg (one process, the whole window) ──────────────────────────
echo "soak-determinism: launching the soak leg: ${SOAK_MINUTES}m, census every ${SOAK_INTERVAL}s"
"$SOAK_BIN" --benchmark --bench-duration "${SOAK_MINUTES}m" --bench-io \
	>"$LOG_PATH" 2>"$SOAK_LOG_DIR/soak_${STAMP}.stderr" &
SOAK_PID=$!

printf 'timestamp,elapsed_sec,rss_kb,fd_count,threads\n' >"$CSV_PATH"
SAMPLES=0
ELAPSED=0
while kill -0 "$SOAK_PID" 2>/dev/null; do
	if [[ "$ELAPSED" -gt "$((SOAK_SECS + 120))" ]]; then
		echo "soak-determinism: FAIL - the soak process outlived its window (+120s grace)" >&2
		kill "$SOAK_PID" 2>/dev/null || true
		exit 1
	fi
	RSS_KB="$(awk '/^VmRSS:/ { print $2 }' "/proc/$SOAK_PID/status" 2>/dev/null || echo 0)"
	FD_COUNT="$(ls "/proc/$SOAK_PID/fd" 2>/dev/null | wc -l || echo 0)"
	THREADS="$(awk '/^Threads:/ { print $2 }' "/proc/$SOAK_PID/status" 2>/dev/null || echo 0)"
	printf '%s,%s,%s,%s,%s\n' \
		"$(date +%Y-%m-%dT%H:%M:%S%z)" "$ELAPSED" "$RSS_KB" "$FD_COUNT" "$THREADS" \
		>>"$CSV_PATH"
	SAMPLES=$((SAMPLES + 1))
	sleep "$SOAK_INTERVAL"
	ELAPSED=$((ELAPSED + SOAK_INTERVAL))
done

wait "$SOAK_PID"
SOAK_EXIT=$?
echo "soak-determinism: soak leg finished: exit=$SOAK_EXIT samples=$SAMPLES"
echo "soak-determinism: census: $CSV_PATH"

# ── The post-soak determinism anchor ─────────────────────────────────────
DIGEST_TEND="$("$SOAK_BIN" --bench-replay 2>/dev/null | grep -o 'replay-digest: b3:[0-9a-f]*' || true)"
if [[ -z "$DIGEST_TEND" ]]; then
	echo "soak-determinism: FAIL - the post-soak anchor produced no replay digest" >&2
	exit 1
fi
echo "soak-determinism: anchor end: $DIGEST_TEND"

# ── The pass law ──────────────────────────────────────────────────────────
VERDICT="PASS"

# 1. Digest equality (the drift canary).
if [[ "$DIGEST_T0" == "$DIGEST_TEND" ]]; then
	echo "soak-determinism: digest drift: NONE (anchors identical)"
else
	echo "soak-determinism: digest drift: PRESENT - determinism broke across the window"
	VERDICT="FAIL"
fi

# 2. RSS plateau: linear regression over the final 80% of samples
#    (the first 20% carries allocator warm-up), growth in %/hour of
#    the median, against the ENDURANCE 2%/hour line.
RSS_VERDICT="SHORT"
if [[ "$SAMPLES" -ge 1 && "$SOAK_MINUTES" -ge 30 ]]; then
	read -r MEDIAN_RSS GROWTH_PERC_HOUR <<<"$(awk -F, -v interval="$SOAK_INTERVAL" '
		NR > 1 && $3 > 0 { t[NR-2]=$2; y[NR-2]=$3; n++ }
		END {
			if (n < 8) { print 0, 0; exit }
			# Drop the first 20% (allocator warm-up), regress the rest.
			keep = int(n * 0.8)
			off = n - keep
			st = 0; sy = 0
			for (i = off; i < n; i++) { st += t[i]; sy += y[i] }
			mt = st / keep; my = sy / keep
			num = 0; den = 0
			for (i = off; i < n; i++) {
				num += (t[i] - mt) * (y[i] - my)
				den += (t[i] - mt) * (t[i] - mt)
			}
			if (den == 0) { print 0, 0; exit }
			slope_per_sec = num / den
			# Median of the regressed window.
			for (a = off; a < n; a++) for (b = a + 1; b < n; b++)
				if (y[b] < y[a]) { tmp = y[a]; y[a] = y[b]; y[b] = tmp }
			mid = int(off + keep / 2)
			med = (keep % 2 == 1) ? y[mid] : (y[mid - 1] + y[mid]) / 2
			if (med <= 0) { print 0, 0; exit }
			growth_kb_per_hour = slope_per_sec * 3600
			printf "%.4f %.4f\n", med, (growth_kb_per_hour / med) * 100
		}' "$CSV_PATH")"
	true  # MEDIAN_RSS + GROWTH_PERC_HOUR parsed from the awk above
	if awk -v g="$GROWTH_PERC_HOUR" 'BEGIN { exit !(g < 2.0) }'; then
		RSS_VERDICT="PASS"
	else
		RSS_VERDICT="FAIL"
		VERDICT="FAIL"
	fi
	echo "soak-determinism: rss growth: ${GROWTH_PERC_HOUR}%/h of ${MEDIAN_RSS} kB median - $RSS_VERDICT (law: < 2%/h)"
else
	echo "soak-determinism: rss growth: SHORT - windows under 30 minutes cannot pin a trend (sample count: $SAMPLES)"
fi

# 3. fd census: flat within the transient tolerance.
FD_VERDICT="SHORT"
if [[ "$SAMPLES" -ge 4 ]]; then
	read -r FD_MIN FD_MAX <<<"$(awk -F, 'NR > 1 && $4 > 0 {
		if (min == "" || $4 < min) min = $4
		if (max == "" || $4 > max) max = $4
	} END { print min + 0, max + 0 }' "$CSV_PATH")"
	FD_SPAN=$((FD_MAX - FD_MIN))
	if [[ "$FD_SPAN" -le 2 ]]; then
		FD_VERDICT="PASS"
	else
		FD_VERDICT="FAIL"
		VERDICT="FAIL"
	fi
	echo "soak-determinism: fd census: min=$FD_MIN max=$FD_MAX span=$FD_SPAN - $FD_VERDICT (law: span <= 2)"
else
	echo "soak-determinism: fd census: SHORT - not enough samples to pin flatness"
fi

# 4. Exit code + panic scan.
if [[ "$SOAK_EXIT" -eq 0 ]]; then
	EXIT_VERDICT="PASS"
else
	EXIT_VERDICT="FAIL"
	VERDICT="FAIL"
fi
if grep -qi 'panicked at' "$LOG_PATH" "$SOAK_LOG_DIR/soak_${STAMP}.stderr" 2>/dev/null; then
	echo "soak-determinism: panic scan: PRESENT - the soak log carries a panic"
	EXIT_VERDICT="FAIL"
	VERDICT="FAIL"
else
	echo "soak-determinism: panic scan: CLEAN"
fi
echo "soak-determinism: exit: $SOAK_EXIT - $EXIT_VERDICT"

echo "soak-determinism: verdict: $VERDICT"
[[ "$VERDICT" == "PASS" ]]
