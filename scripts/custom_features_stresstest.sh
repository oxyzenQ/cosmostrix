#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# Z-master-1T depth stresstest for killer features:
# colors-custom, charset-custom, scene-custom.
# Tests edge cases, invalid values, name conflicts, size limits.
# Reports PASS/FAIL per case.

set -uo pipefail

BIN="./target/release/cosmostrix"
PASS=0
FAIL=0
BUGS=()
RESULTS=()

# Helper: run a case, check no crash + optional expected pattern.
run_case() {
	local label="$1"
	local expected="${2:-}"
	shift 2
	if [ "${1:-}" = "--" ]; then
		shift
	fi
	local output
	output=$("$BIN" "$@" 2>&1 || true)
	if echo "$output" | grep -qiE "panicked|SIGSEGV|core dumped|abort"; then
		FAIL=$((FAIL + 1))
		RESULTS+=("FAIL: $label (CRASH)")
		BUGS+=("$label: CRASH/panic detected")
		return
	fi
	if [ -z "$expected" ] || echo "$output" | grep -qE "$expected"; then
		PASS=$((PASS + 1))
		RESULTS+=("PASS: $label")
	else
		FAIL=$((FAIL + 1))
		RESULTS+=("FAIL: $label (expected: '$expected')")
		BUGS+=("$label: expected '$expected' not found")
	fi
}

echo "=== Z-master-1T Depth Stresstest: Killer Features (custom) ==="
echo ""

TMPDIR_TEST="$HOME/.config/cosmostrix/stresstest_tmp"
mkdir -p "$TMPDIR_TEST"
trap 'rm -rf "$TMPDIR_TEST"' EXIT

# ── colors-custom stresstest ────────────────────────────────────────────
echo "── colors-custom ──"

# Valid custom palette (baseline)
cat >"$TMPDIR_TEST/valid.toml" <<'EOF'
[colors-custom.test]
bg = "#0a0a0a"
rain = ["#ff0000", "#00ff00", "#0000ff"]
EOF
run_case "valid custom palette" "benchmark" -- --config "$TMPDIR_TEST/valid.toml" --colors-custom test --benchmark --bench-duration 1s

# Single stop (min is 2)
cat >"$TMPDIR_TEST/single_stop.toml" <<'EOF'
[colors-custom.test]
rain = ["#ff0000"]
EOF
run_case "single rain stop (min 2)" "error|at least 2" -- --config "$TMPDIR_TEST/single_stop.toml" --colors-custom test --benchmark --bench-duration 1s

# Empty rain array
cat >"$TMPDIR_TEST/empty_rain.toml" <<'EOF'
[colors-custom.test]
rain = []
EOF
run_case "empty rain array" "error|at least 2" -- --config "$TMPDIR_TEST/empty_rain.toml" --colors-custom test --benchmark --bench-duration 1s

# Missing rain field entirely
cat >"$TMPDIR_TEST/no_rain.toml" <<'EOF'
[colors-custom.test]
bg = "#0a0a0a"
EOF
run_case "missing rain field" "error|at least 2" -- --config "$TMPDIR_TEST/no_rain.toml" --colors-custom test --benchmark --bench-duration 1s

# Invalid hex color
cat >"$TMPDIR_TEST/bad_hex.toml" <<'EOF'
[colors-custom.test]
rain = ["#gg0000", "#00ff00"]
EOF
run_case "invalid hex color" "error|invalid" -- --config "$TMPDIR_TEST/bad_hex.toml" --colors-custom test --benchmark --bench-duration 1s

# 3-char hex shorthand (#rgb)
cat >"$TMPDIR_TEST/short_hex.toml" <<'EOF'
[colors-custom.test]
rain = ["#f00", "#0f0", "#00f"]
EOF
run_case "3-char hex shorthand" "benchmark" -- --config "$TMPDIR_TEST/short_hex.toml" --colors-custom test --benchmark --bench-duration 1s

# Unquoted hex (starts with # = TOML comment)
cat >"$TMPDIR_TEST/unquoted.toml" <<'EOF'
[colors-custom.test]
rain = [#ff0000, "#00ff00"]
EOF
run_case "unquoted hex (# comment)" "error" -- --config "$TMPDIR_TEST/unquoted.toml" --colors-custom test --benchmark --bench-duration 1s

# 64 rain stops (max allowed)
STOPS=$(printf '"#%02x%02x%02x",' $(seq 0 4 252) $(seq 0 4 252) $(seq 0 4 252) | sed 's/,$//')
cat >"$TMPDIR_TEST/max_stops.toml" <<EOF
[colors-custom.test]
rain = [$STOPS]
EOF
run_case "64 rain stops (max)" "benchmark" -- --config "$TMPDIR_TEST/max_stops.toml" --colors-custom test --benchmark --bench-duration 1s

# 65 rain stops (over max — should be bounded/truncated)
STOPS65=$(printf '"#%02x0000",' $(seq 0 4 256) | sed 's/,$//')
cat >"$TMPDIR_TEST/over_max.toml" <<EOF
[colors-custom.test]
rain = [$STOPS65]
EOF
run_case "65 rain stops (over max)" "" -- --config "$TMPDIR_TEST/over_max.toml" --colors-custom test --benchmark --bench-duration 1s

# Nonexistent custom palette name
cat >"$TMPDIR_TEST/nonexist.toml" <<'EOF'
[colors-custom.exists]
rain = ["#ff0000", "#00ff00"]
EOF
run_case "nonexistent palette name" "error|not found|unknown" -- --config "$TMPDIR_TEST/nonexist.toml" --colors-custom nonexistent --benchmark --bench-duration 1s

# Duplicate palette names (last wins in TOML)
cat >"$TMPDIR_TEST/dup_names.toml" <<'EOF'
[colors-custom.test]
rain = ["#ff0000", "#00ff00"]

[colors-custom.test]
rain = ["#0000ff", "#ffffff"]
EOF
run_case "duplicate palette names" "" -- --config "$TMPDIR_TEST/dup_names.toml" --colors-custom test --benchmark --bench-duration 1s

# ── charset-custom stresstest ───────────────────────────────────────────
echo "── charset-custom ──"

# Valid custom charset
cat >"$TMPDIR_TEST/valid_charset.toml" <<'EOF'
[charset-custom.test]
set = "ABCDEF"
EOF
run_case "valid custom charset" "benchmark" -- --config "$TMPDIR_TEST/valid_charset.toml" --charset test --benchmark --bench-duration 1s

# Empty charset set
cat >"$TMPDIR_TEST/empty_charset.toml" <<'EOF'
[charset-custom.test]
set = ""
EOF
run_case "empty charset set" "error|empty" -- --config "$TMPDIR_TEST/empty_charset.toml" --charset test --benchmark --bench-duration 1s

# Single char charset
cat >"$TMPDIR_TEST/single_char.toml" <<'EOF'
[charset-custom.test]
set = "X"
EOF
run_case "single char charset" "benchmark" -- --config "$TMPDIR_TEST/single_char.toml" --charset test --benchmark --bench-duration 1s

# Wide chars (CJK)
cat >"$TMPDIR_TEST/wide_chars.toml" <<'EOF'
[charset-custom.test]
set = "アカサタナ"
EOF
run_case "wide CJK chars" "" -- --config "$TMPDIR_TEST/wide_chars.toml" --charset test --benchmark --bench-duration 1s

# 256 chars (max)
CHARS256=$(printf 'A%.0s' {1..256})
cat >"$TMPDIR_TEST/max_chars.toml" <<EOF
[charset-custom.test]
set = "$CHARS256"
EOF
run_case "256 chars (max)" "benchmark" -- --config "$TMPDIR_TEST/max_chars.toml" --charset test --benchmark --bench-duration 1s

# 257 chars (over max — should error)
CHARS257=$(printf 'A%.0s' {1..257})
cat >"$TMPDIR_TEST/over_max_chars.toml" <<EOF
[charset-custom.test]
set = "$CHARS257"
EOF
run_case "257 chars (over max)" "error|max|exceed" -- --config "$TMPDIR_TEST/over_max_chars.toml" --charset test --benchmark --bench-duration 1s

# Nonexistent charset name
cat >"$TMPDIR_TEST/nonexist_charset.toml" <<'EOF'
[charset-custom.exists]
set = "ABC"
EOF
run_case "nonexistent charset name" "error|not found|unknown" -- --config "$TMPDIR_TEST/nonexist_charset.toml" --charset nonexistent --benchmark --bench-duration 1s

# ── scene-custom stresstest ─────────────────────────────────────────────
echo "── scene-custom ──"

# Valid scene-custom
cat >"$TMPDIR_TEST/valid_scene.toml" <<'EOF'
[scene-custom.test]
base-scene = "cinematic"
color = "neon-green"
speed = 15
EOF
run_case "valid scene-custom" "benchmark" -- --config "$TMPDIR_TEST/valid_scene.toml" --scene-custom test --benchmark --bench-duration 1s

# Missing base-scene (should use defaults or error)
cat >"$TMPDIR_TEST/no_base.toml" <<'EOF'
[scene-custom.test]
color = "neon-green"
speed = 15
EOF
run_case "missing base-scene" "" -- --config "$TMPDIR_TEST/no_base.toml" --scene-custom test --benchmark --bench-duration 1s

# Unknown base-scene name
cat >"$TMPDIR_TEST/bad_base.toml" <<'EOF'
[scene-custom.test]
base-scene = "nonexistent_scene"
color = "neon-green"
EOF
run_case "unknown base-scene" "error|unknown" -- --config "$TMPDIR_TEST/bad_base.toml" --scene-custom test --benchmark --bench-duration 1s

# Empty scene-custom block
cat >"$TMPDIR_TEST/empty_scene.toml" <<'EOF'
[scene-custom.test]
EOF
run_case "empty scene-custom block" "" -- --config "$TMPDIR_TEST/empty_scene.toml" --scene-custom test --benchmark --bench-duration 1s

# scene-custom with both color + colors-custom (conflict)
cat >"$TMPDIR_TEST/dual_color.toml" <<'EOF'
[colors-custom.pal]
rain = ["#ff0000", "#00ff00"]

[scene-custom.test]
base-scene = "cinematic"
color = "neon-green"
colors-custom = "pal"
EOF
run_case "color + colors-custom conflict" "" -- --config "$TMPDIR_TEST/dual_color.toml" --scene-custom test --benchmark --bench-duration 1s

# scene-custom with both charset + charset-custom (conflict)
cat >"$TMPDIR_TEST/dual_charset.toml" <<'EOF'
[charset-custom.cs]
set = "ABC"

[scene-custom.test]
base-scene = "cinematic"
charset = "binary"
charset-custom = "cs"
EOF
run_case "charset + charset-custom conflict" "" -- --config "$TMPDIR_TEST/dual_charset.toml" --scene-custom test --benchmark --bench-duration 1s

# Nonexistent scene-custom name
cat >"$TMPDIR_TEST/nonexist_scene.toml" <<'EOF'
[scene-custom.exists]
base-scene = "cinematic"
EOF
run_case "nonexistent scene-custom name" "error|not found|unknown" -- --config "$TMPDIR_TEST/nonexist_scene.toml" --scene-custom nonexistent --benchmark --bench-duration 1s

# ── Cross-feature interaction ───────────────────────────────────────────
echo "── cross-feature ──"

# All 3 custom features together
cat >"$TMPDIR_TEST/all_custom.toml" <<'EOF'
[colors-custom.pal]
bg = "#0a0a0a"
rain = ["#ff0000", "#00ff00", "#0000ff"]

[charset-custom.cs]
set = "ABCDEF"

[scene-custom.test]
base-scene = "cinematic"
colors-custom = "pal"
charset-custom = "cs"
speed = 20
EOF
run_case "all 3 custom features" "benchmark" -- --config "$TMPDIR_TEST/all_custom.toml" --scene-custom test --benchmark --bench-duration 1s

# --colors-custom + --charset (CLI overrides scene)
cat >"$TMPDIR_TEST/cli_override.toml" <<'EOF'
[colors-custom.pal]
rain = ["#ff0000", "#00ff00"]

[scene-custom.test]
base-scene = "cinematic"
color = "neon-green"
EOF
run_case "CLI --colors-custom overrides scene color" "benchmark" -- --config "$TMPDIR_TEST/cli_override.toml" --scene-custom test --colors-custom pal --benchmark --bench-duration 1s

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
