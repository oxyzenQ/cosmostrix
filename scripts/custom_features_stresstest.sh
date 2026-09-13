#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Optimal for Unix-like
#   systems only; not for Windows cmd.exe or PowerShell (use WSL or
#   Git Bash on Windows).
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
bg = "#0a0a0a"
rain = ["#ff0000"]
EOF
run_case "single rain stop (min 2)" "error|at least 2" -- --config "$TMPDIR_TEST/single_stop.toml" --colors-custom test --benchmark --bench-duration 1s

# Empty rain array
cat >"$TMPDIR_TEST/empty_rain.toml" <<'EOF'
[colors-custom.test]
bg = "#0a0a0a"
rain = []
EOF
run_case "empty rain array" "error|at least 2" -- --config "$TMPDIR_TEST/empty_rain.toml" --colors-custom test --benchmark --bench-duration 1s

# Missing rain field entirely
cat >"$TMPDIR_TEST/no_rain.toml" <<'EOF'
[colors-custom.test]
bg = "#0a0a0a"
EOF
run_case "missing rain field" "error|rain" -- --config "$TMPDIR_TEST/no_rain.toml" --colors-custom test --benchmark --bench-duration 1s

# Invalid hex color
cat >"$TMPDIR_TEST/bad_hex.toml" <<'EOF'
[colors-custom.test]
bg = "#0a0a0a"
rain = ["#gg0000", "#00ff00"]
EOF
run_case "invalid hex color" "error|invalid" -- --config "$TMPDIR_TEST/bad_hex.toml" --colors-custom test --benchmark --bench-duration 1s

# 3-char hex shorthand (#rgb)
cat >"$TMPDIR_TEST/short_hex.toml" <<'EOF'
[colors-custom.test]
bg = "#0a0a0a"
rain = ["#f00", "#0f0", "#00f"]
EOF
run_case "3-char hex shorthand" "benchmark" -- --config "$TMPDIR_TEST/short_hex.toml" --colors-custom test --benchmark --bench-duration 1s

# Unquoted hex (starts with # = TOML comment)
cat >"$TMPDIR_TEST/unquoted.toml" <<'EOF'
[colors-custom.test]
rain = [#ff0000, "#00ff00"]
EOF
run_case "unquoted hex (# comment)" "error" -- --config "$TMPDIR_TEST/unquoted.toml" --colors-custom test --benchmark --bench-duration 1s

# 9 rain stops (max allowed — NIGHT-hunt-37: the OKLab engine resamples
# every palette to 9 perceptual samples, so 9 is the ceiling)
STOPS9=$(printf '"#%02x3333",' $(seq 1 9) | sed 's/,$//')
cat >"$TMPDIR_TEST/max_stops.toml" <<EOF
[colors-custom.test]
bg = "#0a0a0a"
rain = [$STOPS9]
EOF
run_case "9 rain stops (max)" "benchmark" -- --config "$TMPDIR_TEST/max_stops.toml" --colors-custom test --benchmark --bench-duration 1s

# 10 rain stops (over max — NIGHT-hunt-37: hard error, not a silent cap)
STOPS10=$(printf '"#%02x3333",' $(seq 1 10) | sed 's/,$//')
cat >"$TMPDIR_TEST/over_max.toml" <<EOF
[colors-custom.test]
bg = "#0a0a0a"
rain = [$STOPS10]
EOF
run_case "10 rain stops (over max)" "error|maximum is 9" -- --config "$TMPDIR_TEST/over_max.toml" --colors-custom test --benchmark --bench-duration 1s

# The owner's NIGHT-hunt-37 repro: a 67-stop rain array must be rejected
STOPS67=$(printf '"#%02x3333",' $(seq 1 67) | sed 's/,$//')
cat >"$TMPDIR_TEST/owner67.toml" <<EOF
[colors-custom.test]
bg = "#0a0a0a"
rain = [$STOPS67]
EOF
run_case "67 rain stops (owner repro)" "error|67 rain stops" -- --config "$TMPDIR_TEST/owner67.toml" --colors-custom test --benchmark --bench-duration 1s

# Missing bg field (NIGHT-hunt-37: a block must be COMPLETE)
cat >"$TMPDIR_TEST/no_bg.toml" <<'EOF'
[colors-custom.test]
rain = ["#ff0000", "#00ff00"]
EOF
run_case "missing bg field" "error|'bg'" -- --config "$TMPDIR_TEST/no_bg.toml" --colors-custom test --benchmark --bench-duration 1s

# rain + stops overload (NIGHT-hunt-37: exactly one of them)
cat >"$TMPDIR_TEST/overload.toml" <<'EOF'
[colors-custom.test]
bg = "#0a0a0a"
rain = ["#ff0000", "#00ff00"]
stops = ["#0000ff", "#ffffff"]
EOF
run_case "rain + stops overload" "error|both 'rain'" -- --config "$TMPDIR_TEST/overload.toml" --colors-custom test --benchmark --bench-duration 1s

# Nonexistent custom palette name
cat >"$TMPDIR_TEST/nonexist.toml" <<'EOF'
[colors-custom.exists]
bg = "#0a0a0a"
rain = ["#ff0000", "#00ff00"]
EOF
run_case "nonexistent palette name" "error|not found|unknown" -- --config "$TMPDIR_TEST/nonexist.toml" --colors-custom nonexistent --benchmark --bench-duration 1s

# Duplicate palette names (last wins in TOML)
cat >"$TMPDIR_TEST/dup_names.toml" <<'EOF'
[colors-custom.test]
bg = "#0a0a0a"
rain = ["#ff0000", "#00ff00"]

[colors-custom.test]
bg = "#0a0a0a"
rain = ["#0000ff", "#ffffff"]
EOF
run_case "duplicate palette names" "error|defined more than once" -- --config "$TMPDIR_TEST/dup_names.toml" --colors-custom test --benchmark --bench-duration 1s

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

# Missing set field entirely (NIGHT-hunt-37: a block must be COMPLETE —
# a [charset-custom.<name>] header without set is a hard error)
cat >"$TMPDIR_TEST/no_set.toml" <<'EOF'
[charset-custom.zen]
# set = "|" (commented out)
EOF
run_case "charset without set field" "error|'set' is missing" -- --config "$TMPDIR_TEST/no_set.toml" --charset zen --benchmark --bench-duration 1s

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

# Valid scene-custom (v80.0.0-beta.2 schema: complete six-dimension
# self-contained profile — no base-scene inheritance; always glyph rain).
cat >"$TMPDIR_TEST/valid_scene.toml" <<'EOF'
[scene-custom.test]
rain = "glyph"
color = "neon-green"
charset = "matrix"
fps = 60
speed = 15
density = 0.75
glitch-level = "subtle"
EOF
run_case "valid scene-custom" "benchmark" -- --config "$TMPDIR_TEST/valid_scene.toml" --scene-custom test --benchmark --bench-duration 1s

# Incomplete block (missing 4 of 6 dimensions) — hard error since
# v80.0.0-beta.2: incomplete blocks are rejected with the exact
# missing-dimension list.
cat >"$TMPDIR_TEST/no_base.toml" <<'EOF'
[scene-custom.test]
rain = "glyph"
color = "neon-green"
speed = 15
EOF
run_case "incomplete scene-custom (missing dimensions) → error" "error|incomplete" -- --config "$TMPDIR_TEST/no_base.toml" --scene-custom test --benchmark --bench-duration 1s

# Removed field (base-scene, deleted in v80.0.0-beta.2) — strict
# reject with the targeted removal hint.
cat >"$TMPDIR_TEST/bad_base.toml" <<'EOF'
[scene-custom.test]
rain = "glyph"
base-scene = "nonexistent_scene"
color = "neon-green"
EOF
run_case "removed base-scene field → strict reject with hint" "error|removed" -- --config "$TMPDIR_TEST/bad_base.toml" --scene-custom test --benchmark --bench-duration 1s

# Empty scene-custom block
cat >"$TMPDIR_TEST/empty_scene.toml" <<'EOF'
[scene-custom.test]
rain = "glyph"
EOF
run_case "empty scene-custom block → incomplete error" "error|incomplete" -- --config "$TMPDIR_TEST/empty_scene.toml" --scene-custom test --benchmark --bench-duration 1s

# scene-custom with both color + colors-custom (conflict) — startup
# resolves like apply_profile_overrides: color wins, palette skipped,
# and the run proceeds (verified: resolved scheme is the block color).
cat >"$TMPDIR_TEST/dual_color.toml" <<'EOF'
[colors-custom.pal]
bg = "#0a0a0a"
rain = ["#ff0000", "#00ff00"]

[scene-custom.test]
rain = "glyph"
color = "neon-green"
colors-custom = "pal"
charset = "matrix"
fps = 60
speed = 9
density = 0.75
glitch-level = "subtle"
EOF
run_case "color + colors-custom conflict → color wins, runs" "benchmark" -- --config "$TMPDIR_TEST/dual_color.toml" --scene-custom test --benchmark --bench-duration 1s

# scene-custom with both charset + charset-custom (conflict) —
# charset wins (same priority contract as the color pair).
cat >"$TMPDIR_TEST/dual_charset.toml" <<'EOF'
[charset-custom.cs]
set = "ABC"

[scene-custom.test]
rain = "glyph"
charset = "binary"
charset-custom = "cs"
color = "neon-green"
fps = 60
speed = 9
density = 0.75
glitch-level = "subtle"
EOF
run_case "charset + charset-custom conflict → charset wins, runs" "benchmark" -- --config "$TMPDIR_TEST/dual_charset.toml" --scene-custom test --benchmark --bench-duration 1s

# Nonexistent scene-custom name
cat >"$TMPDIR_TEST/nonexist_scene.toml" <<'EOF'
[scene-custom.exists]
rain = "glyph"
color = "neon-green"
charset = "matrix"
fps = 60
speed = 9
density = 0.75
glitch-level = "subtle"
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
rain = "glyph"
colors-custom = "pal"
charset-custom = "cs"
fps = 60
speed = 20
density = 0.75
glitch-level = "subtle"
EOF
run_case "all 3 custom features" "benchmark" -- --config "$TMPDIR_TEST/all_custom.toml" --scene-custom test --benchmark --bench-duration 1s

# --colors-custom + --charset (CLI overrides scene)
cat >"$TMPDIR_TEST/cli_override.toml" <<'EOF'
[colors-custom.pal]
bg = "#0a0a0a"
rain = ["#ff0000", "#00ff00"]

[scene-custom.test]
rain = "glyph"
color = "neon-green"
charset = "matrix"
fps = 60
speed = 9
density = 0.75
glitch-level = "subtle"
EOF
run_case "CLI --colors-custom overrides scene color" "benchmark" -- --config "$TMPDIR_TEST/cli_override.toml" --scene-custom test --colors-custom pal --benchmark --bench-duration 1s

# ── Z-master-3-v2: CLI/config harmony (priority contract, observable) ────
echo "── harmony: CLI flags vs config keys (resolved values) ──"

# Shared fixture: config keys set for every overridable field + all 3
# custom feature blocks. Each case then overrides via ONE CLI flag and
# asserts the RESOLVED value in the benchmark JSON report (the same
# resolution live-reload must preserve — see tests_cli_priority.rs).
cat >"$TMPDIR_TEST/harmony.toml" <<'EOF'
color = "snow"
charset = "retro"
scene = "cinematic"
speed = 20
bold = 2
shading-mode = 1

[colors-custom.pal]
bg = "#0a0a0a"
rain = "#00ff41,#00b32d,#005c17"

[charset-custom.cs2]
set = "ABCDEF"

[scene-custom.hx]
rain = "glyph"
color = "green"
charset = "matrix"
fps = 60
speed = 9
density = 0.75
glitch-level = "subtle"
EOF

# CLI --bold 0 must beat config bold = 2 (resolved: "bold":"Off").
run_case "CLI --bold beats config bold key" '"bold":"Off"' -- \
	--config "$TMPDIR_TEST/harmony.toml" --bold 0 \
	--benchmark --bench-duration 1s --json

# CLI --shading-mode 0 must beat config shading-mode = 1.
run_case "CLI --shading-mode beats config key" '"shading":"Random"' -- \
	--config "$TMPDIR_TEST/harmony.toml" --shading-mode 0 \
	--benchmark --bench-duration 1s --json

# CLI --speed 50 must beat config speed = 20.
run_case "CLI --speed beats config key" '"speed":50' -- \
	--config "$TMPDIR_TEST/harmony.toml" --speed 50 \
	--benchmark --bench-duration 1s --json

# CLI --charset cs2 (custom charset) must beat config charset = retro.
run_case "CLI --charset-custom beats config key" '"charset":"cs2"' -- \
	--config "$TMPDIR_TEST/harmony.toml" --charset cs2 \
	--benchmark --bench-duration 1s --json

# CLI --scene-custom hx must beat config scene = cinematic (resolved
# scene shows the custom scene name, not the config's builtin).
run_case "CLI --scene-custom beats config scene key" '"scene":"hx"' -- \
	--config "$TMPDIR_TEST/harmony.toml" --scene-custom hx \
	--benchmark --bench-duration 1s --json

# CLI --colors-custom pal must beat config color = snow: the palette
# branch resolves the scheme to the Green placeholder (never "snow"),
# proving the config builtin did not take over the CLI palette intent.
run_case "CLI --colors-custom beats config color key" '"color_scheme":"green"' -- \
	--config "$TMPDIR_TEST/harmony.toml" --colors-custom pal \
	--benchmark --bench-duration 1s --json

# Conflict inside one block: color + colors-custom — startup resolves
# like apply_profile_overrides (color wins, palette skipped) and the
# resolved scheme must be the block's color, not the palette placeholder.
cat >"$TMPDIR_TEST/block_conflict.toml" <<'EOF'
[colors-custom.pal]
bg = "#0a0a0a"
rain = "#ff0041,#ff6690"

[scene-custom.dual]
rain = "glyph"
color = "green"
colors-custom = "pal"
charset = "matrix"
fps = 60
speed = 9
density = 0.75
glitch-level = "subtle"
EOF
run_case "block color+colors-custom resolves like startup" '"color_scheme":"green"' -- \
	--config "$TMPDIR_TEST/block_conflict.toml" --scene-custom dual \
	--benchmark --bench-duration 1s --json

# ── NIGHT-hunt-38-supermassive: separator typos (owner repros) ─────────
# `set == "x"` used to pass --testconf SILENTLY (the parser stored
# `= "x"` as the charset — garbage glyphs). Now a malformed line with
# the targeted double-equals note.
cat >"$TMPDIR_TEST/double_eq.toml" <<'EOF'
[charset-custom.test]
set == "x"
EOF
run_case "set == typo → malformed, double '=' note" "double '='" -- --config "$TMPDIR_TEST/double_eq.toml" --testconf

# `set : "x"` (YAML/JSON habit) — malformed with the colon note.
cat >"$TMPDIR_TEST/colon_sep.toml" <<'EOF'
[charset-custom.test]
set : "x"
EOF
run_case "set : typo → malformed, colon note" "':' is not a TOML separator" -- --config "$TMPDIR_TEST/colon_sep.toml" --testconf

# Quoted =-leading value stays legal (the bug #19 invariant).
cat >"$TMPDIR_TEST/quoted_eq.toml" <<'EOF'
[charset-custom.eq]
set = "=x"
EOF
run_case "quoted set = \"=x\" stays legal" "PASS" -- --config "$TMPDIR_TEST/quoted_eq.toml" --testconf

# msg-mode = truee — used to pass testconf and print a bare runtime
# error while running. Now uniform reject on every surface.
cat >"$TMPDIR_TEST/bool_typo.toml" <<'EOF'
msg-mode = truee
EOF
run_case "msg-mode = truee → bool reject" "expected true/false" -- --config "$TMPDIR_TEST/bool_typo.toml" --testconf

# ambient == typo — used to misfire the legacy multi-field essay; now
# a syntax error at the parser level.
cat >"$TMPDIR_TEST/ambient_eq.toml" <<'EOF'
ambient.06-00 == "signal"
EOF
run_case "ambient == typo → syntax error, no legacy essay" "malformed line" -- --config "$TMPDIR_TEST/ambient_eq.toml" --testconf

# Legacy multi-field ambient — still rejected, and the essay shows the
# COMPLETE seven-field contract (no stale base-scene line).
cat >"$TMPDIR_TEST/legacy_ambient.toml" <<'EOF'
ambient.15-00 = "neon-purple, signal, speed=50, density=0.65"
EOF
run_case "legacy multi-field ambient → migration, no base-scene" "legacy multi-field" -- --config "$TMPDIR_TEST/legacy_ambient.toml" --testconf
# (the no-base-scene essay contract is asserted precisely by
#  scripts/night_h38_supermassive_testconf_repro.py case 4 and the
#  ambient unit tests — shell grep on quoted output is too fragile
#  for the multi-line essay.)

# ── NIGHT-hunt-39: entry budget (min 1, max 64) ────────────────────────
# 65 charset-custom blocks: hard error naming the count and the cap.
{
	for i in $(seq 0 64); do
		printf '[charset-custom.c%d]\nset = "x"\n\n' "$i"
	done
} >"$TMPDIR_TEST/65_charsets.toml"
run_case "65 charset-custom blocks → over cap" "65 blocks" -- --config "$TMPDIR_TEST/65_charsets.toml" --testconf

# 64 charset-custom blocks: exactly at the cap, legal.
{
	for i in $(seq 0 63); do
		printf '[charset-custom.c%d]\nset = "x"\n\n' "$i"
	done
} >"$TMPDIR_TEST/64_charsets.toml"
run_case "64 charset-custom blocks → at cap, PASS" "PASS" -- --config "$TMPDIR_TEST/64_charsets.toml" --testconf

# 65 ambient entries: hard error (was: silent 256-truncate).
{
	for i in $(seq 0 64); do
		h=$((i / 5))
		m=$((i % 5))
		printf 'ambient.%02d-%02d = "cinematic"\n' "$h" "$m"
	done
} >"$TMPDIR_TEST/65_ambient.toml"
run_case "65 ambient entries → over cap" "65 entries" -- --config "$TMPDIR_TEST/65_ambient.toml" --testconf

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
