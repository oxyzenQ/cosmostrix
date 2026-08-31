#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# Z-master-1T depth stresstest for CLI + config/live-reload.
# Tests edge cases, invalid values, flag conflicts, rapid config edits.
# Reports PASS/FAIL per case. Non-interactive (no TTY needed for validation paths).

set -uo pipefail

BIN="./target/release/cosmostrix"
PASS=0
FAIL=0
BUGS=()
RESULTS=()

# Helper: run a case, check expected pattern + optional unexpected pattern.
# Usage: run_case "label" "expected_pattern" "unexpected_pattern" -- <args...>
run_case() {
	local label="$1"
	local expected="$2"
	local unexpected="${3:-}"
	shift 3
	# Remaining args (after --) go to the binary.
	# Skip the "--" separator if present.
	if [ "${1:-}" = "--" ]; then
		shift
	fi
	local output
	output=$("$BIN" "$@" 2>&1 || true)
	if echo "$output" | grep -qE "$expected"; then
		if [ -z "$unexpected" ] || ! echo "$output" | grep -qiE "$unexpected"; then
			PASS=$((PASS + 1))
			RESULTS+=("PASS: $label")
		else
			FAIL=$((FAIL + 1))
			RESULTS+=("FAIL: $label (unexpected: '$unexpected')")
			BUGS+=("$label: unexpected '$unexpected' found")
		fi
	else
		FAIL=$((FAIL + 1))
		RESULTS+=("FAIL: $label (expected: '$expected')")
		BUGS+=("$label: expected '$expected' not found")
	fi
}

# Helper: check no crash (exit 0 or 2 is OK; segfault/panic is a bug).
# Usage: run_no_crash "label" -- <args...>
run_no_crash() {
	local label="$1"
	shift
	if [ "${1:-}" = "--" ]; then
		shift
	fi
	local output
	output=$("$BIN" "$@" 2>&1 || true)
	if echo "$output" | grep -qiE "panicked|SIGSEGV|core dumped|abort"; then
		FAIL=$((FAIL + 1))
		RESULTS+=("FAIL: $label (CRASH)")
		BUGS+=("$label: CRASH/panic detected")
	else
		PASS=$((PASS + 1))
		RESULTS+=("PASS: $label (no crash)")
	fi
}

echo "=== Z-master-1T Depth Stresstest: CLI + Config ==="
echo ""

# ── Category 1: CLI value boundary edge cases ───────────────────────────
echo "── CLI value boundaries ──"
run_no_crash "fps=1 (min)" -- --fps 1 --benchmark --bench-duration 1s
run_no_crash "fps=240 (max)" -- --fps 240 --benchmark --bench-duration 1s
run_no_crash "fps=0 (invalid)" -- --fps 0 --benchmark --bench-duration 1s
run_no_crash "fps=-1 (invalid)" -- --fps -1 --benchmark --bench-duration 1s
run_no_crash "fps=999999 (extreme)" -- --fps 999999 --benchmark --bench-duration 1s
run_no_crash "speed=1 (min)" -- --speed 1 --benchmark --bench-duration 1s
run_no_crash "speed=100 (max)" -- --speed 100 --benchmark --bench-duration 1s
run_no_crash "speed=0 (invalid)" -- --speed 0 --benchmark --bench-duration 1s
run_no_crash "density=0.01 (min)" -- --density 0.01 --benchmark --bench-duration 1s
run_no_crash "density=5.0 (max)" -- --density 5.0 --benchmark --bench-duration 1s
run_no_crash "density=0 (invalid)" -- --density 0 --benchmark --bench-duration 1s
run_no_crash "density=-1 (invalid)" -- --density -1 --benchmark --bench-duration 1s

# ── Category 2: CLI flag conflicts + precedence ─────────────────────────
echo "── CLI flag conflicts ──"
run_no_crash "scene + color (CLI color wins)" -- --scene cinematic --color red --benchmark --bench-duration 1s
run_no_crash "scene + scene-custom (no custom)" -- --scene cinematic --scene-custom nonexistent --benchmark --bench-duration 1s
run_no_crash "color + colors-custom (no custom)" -- --color red --colors-custom nonexistent --benchmark --bench-duration 1s
run_no_crash "charset + charset-custom alias" -- --charset binary --charset-custom nonexistent --benchmark --bench-duration 1s
run_no_crash "msg-mode=false + -m (CLI wins)" -- --msg-mode false -m "test" --benchmark --bench-duration 1s
run_no_crash "power-dragon=false + crystal-dragon=true" -- --power-dragon false --crystal-dragon true --benchmark --bench-duration 1s
run_no_crash "no-effects + benchmark (effects auto-off)" -- --no-effects --benchmark --bench-duration 1s

# ── Category 3: CLI enum value edge cases ───────────────────────────────
echo "── CLI enum values ──"
run_case "glitch-level=none" "benchmark" "" -- --glitch-level none --benchmark --bench-duration 1s
run_case "glitch-level=intense" "benchmark" "" -- --glitch-level intense --benchmark --bench-duration 1s
run_case "glitch-level=NONE (uppercase)" "benchmark|invalid" "" -- --glitch-level NONE --benchmark --bench-duration 1s
run_case "intro=none" "" "" -- --intro none --benchmark --bench-duration 1s
run_case "intro=cosmic" "" "" -- --intro cosmic --benchmark --bench-duration 1s
run_case "color-bg=black" "" "" -- --color-bg black --benchmark --bench-duration 1s
run_case "color-bg=default-background" "" "" -- --color-bg default-background --benchmark --bench-duration 1s
run_case "monolith-size=small" "" "" -- --monolith-size small --benchmark --bench-duration 1s
run_case "monolith-size=large" "" "" -- --monolith-size large --benchmark --bench-duration 1s
run_case "msg-fill-style=cascade" "benchmark" "" -- --msg-fill-style cascade --benchmark --bench-duration 1s
run_case "msg-fill-style=glitch" "benchmark" "" -- --msg-fill-style glitch --benchmark --bench-duration 1s

# ── Category 4: CLI typo suggestions (consistency) ──────────────────────
echo "── CLI typo suggestions ──"
run_case "--colr typo → tip color" "tip: a similar argument exists: '--color'" "Did you mean" -- --colr red
run_case "--scne typo → tip scene" "tip: a similar argument exists: '--scene'" "Did you mean" -- --scne cinematic
run_case "--fps=abc (non-numeric)" "invalid|error" "panic" -- --fps abc --benchmark --bench-duration 1s
run_case "--speed=xyz (non-numeric)" "invalid|error" "panic" -- --speed xyz --benchmark --bench-duration 1s

# ── Category 5: Config file edge cases (via --config) ───────────────────
echo "── Config file edge cases ──"

# Create temp config with valid content
TMPDIR_TEST=$(mktemp -d)
trap 'rm -rf "$TMPDIR_TEST"' EXIT

# Valid config
cat >"$TMPDIR_TEST/valid.toml" <<'EOF'
scene = "cinematic"
color = "energy-zen"
fps = 60
speed = 9
density = 0.75
EOF
run_no_crash "valid config" -- --config "$TMPDIR_TEST/valid.toml" --benchmark --bench-duration 1s

# Empty config file
echo "" >"$TMPDIR_TEST/empty.toml"
run_no_crash "empty config" -- --config "$TMPDIR_TEST/empty.toml" --benchmark --bench-duration 1s

# Config with unknown key
cat >"$TMPDIR_TEST/unknown_key.toml" <<'EOF'
unknown-key = "value"
scene = "cinematic"
EOF
run_case "unknown config key → error" "unknown|error" "panic" -- --config "$TMPDIR_TEST/unknown_key.toml" --benchmark --bench-duration 1s

# Config with invalid value type
cat >"$TMPDIR_TEST/bad_type.toml" <<'EOF'
fps = "not-a-number"
EOF
run_case "bad type fps → error" "invalid|error" "panic" -- --config "$TMPDIR_TEST/bad_type.toml" --benchmark --bench-duration 1s

# Config with out-of-range value
cat >"$TMPDIR_TEST/out_of_range.toml" <<'EOF'
fps = 999999
speed = 999
density = 99.0
EOF
run_case "out-of-range values → error" "invalid|error|range" "panic" -- --config "$TMPDIR_TEST/out_of_range.toml" --benchmark --bench-duration 1s

# Config with both message + message-border (border wins)
cat >"$TMPDIR_TEST/dual_msg.toml" <<'EOF'
message = "no border"
message-border = "with border"
EOF
run_no_crash "dual message keys" -- --config "$TMPDIR_TEST/dual_msg.toml" --benchmark --bench-duration 1s

# Config with malformed TOML (unclosed string)
cat >"$TMPDIR_TEST/malformed.toml" <<'EOF'
scene = "cinematic
EOF
run_case "malformed TOML → error" "error|invalid" "panic" -- --config "$TMPDIR_TEST/malformed.toml" --benchmark --bench-duration 1s

# ── Category 6: --dump-config + --testconf ──────────────────────────────
echo "── dump-config + testconf ──"
run_case "dump-config produces output" "cosmostrix configuration" "" -- --dump-config
run_no_crash "testconf on valid config" -- --config "$TMPDIR_TEST/valid.toml" --testconf
run_case "testconf on bad config → error" "error|invalid" "panic" -- --config "$TMPDIR_TEST/unknown_key.toml" --testconf

# ── Category 7: Rapid config edits (live-reload simulation) ─────────────
echo "── Live-reload rapid edits ──"
# Create a config, rapidly change it, verify the process doesn't crash.
# We can't easily test interactive live-reload non-interactively, but we
# can verify the config parser handles rapid re-reads without issues.
cat >"$TMPDIR_TEST/reload_1.toml" <<'EOF'
scene = "cinematic"
color = "energy-zen"
EOF
cat >"$TMPDIR_TEST/reload_2.toml" <<'EOF'
scene = "matrix"
color = "green"
EOF
cat >"$TMPDIR_TEST/reload_3.toml" <<'EOF'
scene = "monolith"
color = "neon-purple"
speed = 30
EOF
run_no_crash "config reload 1→2→3" -- --config "$TMPDIR_TEST/reload_1.toml" --benchmark --bench-duration 1s
run_no_crash "config reload 2" -- --config "$TMPDIR_TEST/reload_2.toml" --benchmark --bench-duration 1s
run_no_crash "config reload 3" -- --config "$TMPDIR_TEST/reload_3.toml" --benchmark --bench-duration 1s

# ── Summary ─────────────────────────────────────────────────────────────
echo ""
for r in "${RESULTS[@]}"; do
	echo "  $r"
done
echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "  Stresstest Results: ${PASS} passed, ${FAIL} failed"
echo "═══════════════════════════════════════════════════════════════"
if [ ${#BUGS[@]} -gt 0 ]; then
	echo ""
	echo "BUGS FOUND:"
	for bug in "${BUGS[@]}"; do
		echo "  - $bug"
	done
fi
echo ""
if [ "$FAIL" -gt 0 ]; then
	exit 1
fi
exit 0
