# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

#!/usr/bin/env bash
set -euo pipefail

# rc-smoke.sh — Release candidate smoke tests
# Non-destructive pre-release verification. Safe to run locally or in CI.
# Usage: bash scripts/rc-smoke.sh [BINARY_PATH]

BIN="${1:-target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix}"

log() { printf '[INFO] %s\n' "$*"; }
pass() { printf '[PASS] %s\n' "$*"; }
fail() { printf '[FAIL] %s\n' "$*" >&2; exit 1; }

[[ -x "$BIN" ]] || fail "Binary not found or not executable: $BIN"

# ── Version check ─────────────────────────────────────────────────────────

log "Checking version output"
"$BIN" -V | grep -Fq "Version: v" || fail "Missing version in -V output"
pass "Version present"

# ── Info/doctor check ─────────────────────────────────────────────────────

log "Checking doctor output (v17: --info merged into --doctor)"
"$BIN" --doctor | grep -Fq "COSMOSTRIX DIAGNOSTICS REPORT" || fail "Missing doctor report header"
"$BIN" --doctor | grep -Fq "identity:" || fail "Missing identity in --doctor output"
"$BIN" --doctor | grep -Fq "gpu_usage: not_applicable" || fail "Missing gpu_usage in --doctor"
pass "Doctor report fields correct"

# ── Doctor check ─────────────────────────────────────────────────────────

log "Checking doctor environment section"
"$BIN" --doctor | grep -Fq "COSMOSTRIX DIAGNOSTICS REPORT" || fail "Missing doctor report header"
pass "Doctor report present"

# ── Benchmark check ────────────────────────────────────────────────────────

log "Checking benchmark output"
"$BIN" --benchmark | grep -Eq "avg_fps:" || fail "Missing avg_fps in benchmark"
"$BIN" --benchmark | grep -Eq "p99_frame_time:" || fail "Missing p99_frame_time in benchmark"
"$BIN" --benchmark | grep -Eq "frame_time_stability:" || fail "Missing frame_time_stability in benchmark"
"$BIN" --benchmark | grep -Eq "actual_execution: single-threaded-renderer" || fail "actual_execution should be single-threaded-renderer"
pass "Benchmark fields present and correct"

# ── Controlled-live config smoke ──────────────────────────────────────────

log "Checking controlled-live config smoke"
TMP_CL="$(mktemp)"
printf 'scene = monolith\ncolor = sun\natmosphere-mode = controlled-live\natmosphere-regime = pulse\n' > "$TMP_CL"

# v17: --info removed. Check --doctor for build/renderer fields.
"$BIN" --config "$TMP_CL" --doctor | grep -Fq "BUILD" || fail "doctor should have BUILD section"
"$BIN" --config "$TMP_CL" --doctor | grep -Fq "RENDERER" || fail "doctor should have RENDERER section"
pass "Controlled-live config smoke passed"
rm -f "$TMP_CL"

# CLI color override with controlled-live config
TMP_CL2="$(mktemp)"
printf 'scene = monolith\ncolor = cosmos\natmosphere-mode = controlled-live\natmosphere-regime = pulse\n' > "$TMP_CL2"

log "Checking CLI color override with controlled-live config"
"$BIN" --config "$TMP_CL2" --color sun --doctor | grep -Fq "variant:" || fail "doctor should show variant field"
pass "CLI color override with controlled-live config passed"
rm -f "$TMP_CL2"

# ── Disabled + non-Calm config smoke ──────────────────────────────────────

log "Checking disabled + non-Calm config smoke"
TMP_DIS="$(mktemp)"
printf 'atmosphere-mode = disabled\natmosphere-regime = pulse\n' > "$TMP_DIS"

# v17: simplified atmosphere section — check status field
"$BIN" --config "$TMP_DIS" --doctor | grep -Fq "status:" || fail "doctor should have atmosphere status field"
pass "Disabled + non-Calm config smoke passed"
rm -f "$TMP_DIS"

# ── v14 Scene catalog discoverability via --list-scenes ───────────────────

log "Checking --list-scenes built-in scene discoverability"
LIST_OUT=$("$BIN" --list-scenes)
echo "$LIST_OUT" | grep -Fq "AVAILABLE SCENES" || fail "--list-scenes must show AVAILABLE SCENES section"
echo "$LIST_OUT" | grep -Fq "monolith" || fail "--list-scenes must list monolith"
echo "$LIST_OUT" | grep -Fq "storm" || fail "--list-scenes must list storm"
echo "$LIST_OUT" | grep -Fq "low-power" || fail "--list-scenes must list low-power"
echo "$LIST_OUT" | grep -Fq "hacker" || fail "--list-scenes must list hacker"
pass "Scene catalog discoverability passed"

# ── README / CHANGELOG / casing audit ────────────────────────────────────

log "Checking README guards"
[[ -f "README.md" ]] || fail "README.md not found"
grep -Fq "CHANGELOG.md" README.md || fail "README must link to CHANGELOG.md"
! grep -Eq "^#+ Release notes" README.md || fail "README must not contain release notes section"
! grep -Eq "^### v2\.[0-9]+\.[0-9]+" README.md || fail "README must not contain v2.x.x release headings"
pass "README guards passed"

log "Checking casing audit"
# Scan docs, source, and metadata for wrong-cased repo owner.
# The canonical casing has a capital Q; construct the bad pattern dynamically
# so the script itself never contains the wrong-cased literal.
_BAD_O="oxyzen""q"
_BAD_G="github.com/${_BAD_O}"
BAD_CASING=0
for SCAN_DIR in docs src; do
    if [[ -d "$SCAN_DIR" ]]; then
        if grep -rq "$_BAD_G" "$SCAN_DIR" 2>/dev/null; then
            BAD_CASING=1
        fi
    fi
done
for SCAN_FILE in README.md CHANGELOG.md Cargo.toml; do
    if [[ -f "$SCAN_FILE" ]]; then
        if grep -q "$_BAD_G" "$SCAN_FILE" 2>/dev/null; then
            BAD_CASING=1
        fi
    fi
done
if [[ "$BAD_CASING" -eq 1 ]]; then
    fail "Wrong-cased repo owner found"
fi
pass "Casing audit clean"

# ── v14 Scene-custom ecosystem RC smoke ──────────────────────────────────

log "Checking --dump-config scene-custom pointers"
DUMP_V47=$("$BIN" --dump-config)
echo "$DUMP_V47" | grep -Fq "scene-custom" || fail "--dump-config must document scene-custom namespace"
echo "$DUMP_V47" | grep -Fq "ATMOSPHERE_ENGINE" || fail "--dump-config must point to docs/ATMOSPHERE_ENGINE.md"
pass "--dump-config scene-custom pointers passed"

log "Checking unknown custom scene error mentions --list-scenes"
TMP_UP="$(mktemp)"
printf 'scene-custom.test.base = monolith\n' > "$TMP_UP"
UP_ERR=$("$BIN" --config "$TMP_UP" --scene-custom nonexistent 2>&1 || true)
echo "$UP_ERR" | grep -Fq "expected one of:" || fail "unknown custom scene error must list available names"
rm -f "$TMP_UP"
pass "Unknown custom scene error passed"

log "Checking storm atmosphere-regime remains unavailable"
TMP_STORM="$(mktemp)"
printf 'scene-custom.storm.base = monolith\nscene-custom.storm.atmosphere-regime = storm\n' > "$TMP_STORM"
STORM_ERR=$("$BIN" --config "$TMP_STORM" --scene-custom storm 2>&1 || true)
echo "$STORM_ERR" | grep -Fq "storm is unavailable" || fail "storm must be reported as unavailable"
rm -f "$TMP_STORM"
pass "Storm unavailability passed"

log "Checking default runtime and writer invariants"
"$BIN" --doctor | grep -Fq "status:" || fail "doctor must have atmosphere status"
"$BIN" --benchmark | grep -Eq "terminal_writer: single-owner" || fail "terminal_writer must be single-owner"
"$BIN" --benchmark | grep -Eq "compute_parallelism: disabled" || fail "compute_parallelism must be disabled"
pass "Default runtime and writer invariants passed"

log "All release candidate smoke checks passed"
