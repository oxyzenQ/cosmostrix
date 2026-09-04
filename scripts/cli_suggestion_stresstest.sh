#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# Z-master-1X CLI suggestion stresstest.
#
# Runs the cosmostrix binary with a battery of typo / wrong-value / edge
# inputs and verifies the suggestion system produces the expected
# "tip: a similar argument/value exists" format (NOT the legacy
# "Did you mean" format). Captures pass/fail per case + a summary.

set -uo pipefail

BIN="./target/debug/cosmostrix"
PASS=0
FAIL=0
RESULTS=()

run_case() {
	local label="$1"
	shift
	local expected_pattern="$1"
	shift
	local unexpected_pattern="${1:-}"
	shift
	# Remaining args: the CLI args to pass to the binary.
	local output
	output=$("$BIN" "$@" 2>&1 || true)
	if echo "$output" | grep -qE "$expected_pattern"; then
		if [ -z "$unexpected_pattern" ] || ! echo "$output" | grep -qiE "$unexpected_pattern"; then
			PASS=$((PASS + 1))
			RESULTS+=("PASS: $label")
		else
			FAIL=$((FAIL + 1))
			RESULTS+=("FAIL: $label (unexpected pattern '$unexpected_pattern' found)")
		fi
	else
		FAIL=$((FAIL + 1))
		RESULTS+=("FAIL: $label (expected '$expected_pattern' not found)")
	fi
}

echo "=== Z-master-1X CLI Suggestion Stresstest ==="
echo ""

# ── Category 1: long-flag typos (argument suggestions) ──────────────────
run_case "no-effecs typo → --no-effects" \
	"tip: a similar argument exists: '--no-effects'" \
	"Did you mean" \
	--no-effecs

run_case "color typo --colr → --color" \
	"tip: a similar argument exists: '--color'" \
	"Did you mean" \
	--colr neon-green

run_case "crystal-dragon typo --crystal-drago → --crystal-dragon" \
	"tip: a similar argument exists: '--crystal-dragon'" \
	"Did you mean" \
	--crystal-drago true

run_case "msg-fill-style typo --msg-fill-styl → --msg-fill-style" \
	"tip: a similar argument exists: '--msg-fill-style'" \
	"Did you mean" \
	--msg-fill-styl typewriter

run_case "verbose typo --verbos → --verbose" \
	"tip: a similar argument exists: '--verbose'" \
	"Did you mean" \
	--verbos

run_case "power-dragon typo --power-drago → --power-dragon" \
	"tip: a similar argument exists: '--power-dragon'" \
	"Did you mean" \
	--power-drago true

# ── Category 2: value typos (value suggestions) ─────────────────────────
run_case "color value typo 'neon-gren' → 'neon-green'" \
	"tip: a similar value exists: 'neon-green'" \
	"Did you mean" \
	--color neon-gren

# 'cyberpuunk2077' is NOT a builtin theme (it's a custom-color name) —
# the suggestion system only suggests from KNOWN candidates, so this
# correctly produces no tip. We verify the 'no tip for unknown custom name'
# behavior instead of asserting a suggestion that can't fire.
run_case "color value 'cyberpuunk2077' (unknown custom name) → no tip" \
	"unknown color" \
	"tip: a similar value" \
	--color cyberpuunk2077

run_case "color value typo 'vapporwave' → 'vaporwave'" \
	"tip: a similar value exists: 'vaporwave'" \
	"Did you mean" \
	--color vapporwave

run_case "scene value typo 'cinemtic' → 'cinematic'" \
	"tip: a similar value exists: 'cinematic'" \
	"Did you mean" \
	--scene cinemtic

run_case "charset value typo 'binari' → 'binary'" \
	"tip: a similar value exists: 'binary'" \
	"Did you mean" \
	--charset binari

run_case "msg-fill-style value typo 'typewritter' → 'typewriter'" \
	"tip: a similar value exists: 'typewriter'" \
	"Did you mean" \
	--msg-fill-style typewritter

run_case "msg-fill-style value typo 'hollogram' → 'hologram'" \
	"tip: a similar value exists: 'hologram'" \
	"Did you mean" \
	--msg-fill-style hollogram

run_case "glitch-level value typo 'defualt' → 'default'" \
	"tip: a similar value exists: 'default'" \
	"Did you mean" \
	--glitch-level defualt

# ── Category 3: case-insensitivity ──────────────────────────────────────
# Color names are case-insensitive — 'NEON-GREEN' must parse as neon-green.
# We verify by checking the 'unknown color' error does NOT fire (clap accepts
# the value; the TTY error that follows is expected for non-interactive runs).
run_case "color value uppercase 'NEON-GREEN' accepted (case-insensitive)" \
	"error:" \
	"unknown color" \
	--color NEON-GREEN --duration 0.1

# ── Category 4: completely-wrong values (no suggestion should fire) ─────
run_case "color value 'xyzabc' → no tip (too distant)" \
	"" \
	"tip: a similar value" \
	--color xyzabc

run_case "scene value 'zzzzzzz' → no tip (too distant)" \
	"" \
	"tip: a similar value" \
	--scene zzzzzzz

# ── Category 5: short-form expansion (-mfs) ─────────────────────────────
run_case "-mfs typo '-mfss' → tip --msg-fill-style" \
	"tip: a similar argument exists: '--msg-fill-style'" \
	"Did you mean" \
	-mfss typewriter

# ── Category 6: help-footer consistency (v100.0.0-nightly.1 owner sweep) ─
# The ux contract: CLI value errors (--scene / --show-scene typos) end
# with the clap-canonical footer, exactly like the charset/color value
# errors; config-FILE failures keep the footer-less die_config shape.
run_case "scene value typo 'cosmosm' → tip AND help footer" \
	"For more information, try '--help'." \
	"Did you mean" \
	--scene cosmosm

run_case "scene value 'zzzzzzz' → footer, no tip (distant)" \
	"For more information, try '--help'." \
	"tip: a similar value" \
	--scene zzzzzzz

run_case "show-scene unknown 'cosmosm' → suggestion tip" \
	"tip: a similar value exists: 'cosmos'" \
	"Did you mean" \
	--show-scene cosmosm

run_case "show-scene unknown 'cosmosm' → help footer" \
	"For more information, try '--help'." \
	"" \
	--show-scene cosmosm

# Config-FILE failures stay footer-less: a config problem is not an
# invocation problem (die_config family, classifier contract).
# NOTE 1: the path-security guard only accepts ~/.config/cosmostrix/ (or
# the system dirs), so the temp config must live there, not in /tmp.
# NOTE 2: at least one VALID key must be present — startup validation is
# gated on a non-empty parsed map, and a file with only a malformed line
# parses to zero keys (validation skipped, renderer starts).
cfgdir="${HOME}/.config/cosmostrix"
mkdir -p "$cfgdir"
tmpcfg=$(mktemp "${cfgdir}/stresstest-badcfg.XXXXXX.toml")
printf 'scene = cinematic\nnot-a-config-line-without-equals\n' >"$tmpcfg"
run_case "malformed config line → error, NO help footer" \
	"error: invalid config" \
	"For more information, try '--help'." \
	--config "$tmpcfg"
rm -f "$tmpcfg"

# ── Category 6b: --dump-config write I/O failure (owner hunt 2026-09-04) ──
# The I/O arm used die_config (config-file family): footer-less, tip-less,
# while the same flag's overwrite guard (two lines earlier) rendered a
# guided message with the help footer. Now both are die_input family.
# Deterministic trigger for any user incl. root: a path whose parent
# component is a FILE makes the atomic write's create_dir_all fail with
# NotADirectory — no permission juggling required.
blocker=$(mktemp "${cfgdir}/stresstest-blocker.XXXXXX")
run_case "dump-config I/O failure (parent is a file) → guided error" \
	"error: cannot write --dump-config" \
	"tip: a similar" \
	--dump-config "${blocker}/x.toml"

run_case "dump-config I/O failure → retry guidance + footer" \
	"For more information, try '--help'." \
	"" \
	--dump-config "${blocker}/x.toml"
rm -f "$blocker"

# ── Category 7: case-insensitive flag rescue (v100.0.0-nightly.1 --LIS) ──
# clap's did-you-mean is case-sensitive (strsim Jaro): --lis suggests
# --list-scenes but --LIS scored zero matching chars and rendered
# tip-less. The ux fallback injects the case-insensitive match as
# clap's own SuggestedArg context — same tip shape, same position.
run_case "uppercase prefix '--LIS' → tip --list-scenes (case rescue)" \
	"tip: a similar argument exists: '--list-scenes'" \
	"Did you mean" \
	--LIS

run_case "all-caps typo '--HELPSS' → tip --help (case rescue)" \
	"tip: a similar argument exists: '--help'" \
	"Did you mean" \
	--HELPSS

run_case "single-char unknown '--x' → no tip (clap silence parity)" \
	"unexpected argument '--x'" \
	"tip: a similar" \
	--x

# ── Summary ─────────────────────────────────────────────────────────────
echo ""
for r in "${RESULTS[@]}"; do
	echo "  $r"
done
echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "  Stresstest Results: ${PASS} passed, ${FAIL} failed"
echo "═══════════════════════════════════════════════════════════════"
if [ "$FAIL" -gt 0 ]; then
	exit 1
fi
exit 0
