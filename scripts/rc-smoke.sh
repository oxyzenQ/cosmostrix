# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: MIT

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

# ── Info check ───────────────────────────────────────────────────────────

log "Checking info output"
"$BIN" -i | grep -Fq "identity:" || fail "Missing identity in -i output"
"$BIN" -i | grep -Fq "production-grade cinematic Matrix rain renderer" || fail "Missing canonical tagline"
"$BIN" -i | grep -Fq "application_mode: disabled" || fail "application_mode should be disabled by default"
"$BIN" -i | grep -Fq "effective_runtime: identity" || fail "effective_runtime should be identity by default"
"$BIN" -i | grep -Fq "shadow_risk: identity" || fail "shadow_risk should be identity by default"
"$BIN" -i | grep -Fq "config_gate: disabled" || fail "config_gate should be disabled by default"
"$BIN" -i | grep -Fq "visual_runtime: protected" || fail "visual_runtime should be protected by default"
"$BIN" -i | grep -Fq "runtime_application: identity" || fail "runtime_application should be identity by default"
pass "Default info fields correct"

# ── Doctor check ─────────────────────────────────────────────────────────

log "Checking doctor output"
"$BIN" --doctor | grep -Fq "COSMOSTRIX DIAGNOSTICS REPORT" || fail "Missing doctor report header"
pass "Doctor report present"

# ── Benchmark check ────────────────────────────────────────────────────────

log "Checking benchmark output"
"$BIN" --benchmark | grep -Eq "avg_fps:" || fail "Missing avg_fps in benchmark"
"$BIN" --benchmark | grep -Eq "p99_frame_time:" || fail "Missing p99_frame_time in benchmark"
"$BIN" --benchmark | grep -Eq "frame_time_stability:" || fail "Missing frame_time_stability in benchmark"
"$BIN" --benchmark | grep -Eq "actual_execution: single-threaded-renderer" || fail "actual_execution should be single-threaded-renderer"
"$BIN" --benchmark | grep -Eq "application_mode: disabled" || fail "Benchmark atmosphere application_mode should be disabled"
pass "Benchmark fields present and correct"

# ── Controlled-live config smoke ──────────────────────────────────────────

log "Checking controlled-live config smoke"
TMP_CL="$(mktemp)"
printf 'scene = monolith\ncolor = sun\natmosphere-mode = controlled-live\natmosphere-regime = pulse\n' > "$TMP_CL"

"$BIN" --config "$TMP_CL" -i | grep -Fq "config_gate: armed" || fail "controlled-live config should have config_gate armed"
"$BIN" --config "$TMP_CL" -i | grep -Fq "shadow_risk: whisper" || fail "controlled-live pulse should have shadow_risk whisper"
"$BIN" --config "$TMP_CL" -i | grep -Fq "visual_runtime: protected" || fail "controlled-live should have visual_runtime protected"
"$BIN" --config "$TMP_CL" -i | grep -Fq "runtime_application: identity" || fail "controlled-live should have runtime_application identity"
pass "Controlled-live config smoke passed"
rm -f "$TMP_CL"

# CLI color override with controlled-live config
TMP_CL2="$(mktemp)"
printf 'scene = monolith\ncolor = cosmos\natmosphere-mode = controlled-live\natmosphere-regime = pulse\n' > "$TMP_CL2"

log "Checking CLI color override with controlled-live config"
"$BIN" --config "$TMP_CL2" --color sun -i | grep -Fq "color: sun" || fail "CLI --color sun should force color sun"
pass "CLI color override with controlled-live config passed"
rm -f "$TMP_CL2"

# ── Disabled + non-Calm config smoke ──────────────────────────────────────

log "Checking disabled + non-Calm config smoke"
TMP_DIS="$(mktemp)"
printf 'atmosphere-mode = disabled\natmosphere-regime = pulse\n' > "$TMP_DIS"

"$BIN" --config "$TMP_DIS" -i | grep -Fq "application_mode: disabled" || fail "disabled config should have application_mode disabled"
"$BIN" --config "$TMP_DIS" -i | grep -Fq "effective_runtime: identity" || fail "disabled + pulse should have effective_runtime identity"
"$BIN" --config "$TMP_DIS" -i | grep -Fq "shadow_risk: identity" || fail "disabled + pulse should have shadow_risk identity"
"$BIN" --config "$TMP_DIS" -i | grep -Fq "config_gate: disabled" || fail "disabled config should have config_gate disabled"
pass "Disabled + non-Calm config smoke passed"
rm -f "$TMP_DIS"

# ── Atmosphere preset discoverability via --list-profiles ──────────────────

log "Checking --list-profiles atmosphere preset discoverability"
LIST_OUT=$("$BIN" --list-profiles)
echo "$LIST_OUT" | grep -Fq "CONTROLLED ATMOSPHERE PRESETS" || fail "--list-profiles must show CONTROLLED ATMOSPHERE PRESETS section"
echo "$LIST_OUT" | grep -Fq "atmosphere-calm" || fail "--list-profiles must list atmosphere-calm"
echo "$LIST_OUT" | grep -Fq "atmosphere-pulse" || fail "--list-profiles must list atmosphere-pulse"
echo "$LIST_OUT" | grep -Fq "atmosphere-signal" || fail "--list-profiles must list atmosphere-signal"
echo "$LIST_OUT" | grep -Fq "atmosphere-compression" || fail "--list-profiles must list atmosphere-compression"
echo "$LIST_OUT" | grep -Fq "atmosphere-void" || fail "--list-profiles must list atmosphere-void"
echo "$LIST_OUT" | grep -Fq "atmosphere-monolith-pressure" || fail "--list-profiles must list atmosphere-monolith-pressure"
echo "$LIST_OUT" | grep -Fq "atmosphere-storm" && fail "--list-profiles must not list atmosphere-storm"
pass "Atmosphere preset discoverability passed"

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

# ── v4.7 Profile ecosystem RC smoke ──────────────────────────────────────

log "Checking --list-profiles profile ecosystem pointers"
LIST_V47=$("$BIN" --list-profiles)
echo "$LIST_V47" | grep -Fq "USER PROFILES" || fail "--list-profiles must print USER PROFILES"
echo "$LIST_V47" | grep -Fq "PROFILE_ECOSYSTEM" || fail "--list-profiles must point to docs/PROFILE_ECOSYSTEM.md"
echo "$LIST_V47" | grep -Fq "PROFILE_EXAMPLES" || fail "--list-profiles must point to docs/PROFILE_EXAMPLES.md"
pass "--list-profiles profile ecosystem pointers passed"

log "Checking --dump-config profile pointers"
DUMP_V47=$("$BIN" --dump-config)
echo "$DUMP_V47" | grep -Fq "PROFILE_EXAMPLES" || fail "--dump-config must point to docs/PROFILE_EXAMPLES.md"
echo "$DUMP_V47" | grep -Fq "ATMOSPHERE_PRESETS" || fail "--dump-config must point to atmosphere preset examples"
pass "--dump-config profile pointers passed"

log "Checking unknown profile error mentions --list-profiles"
TMP_UP="$(mktemp)"
printf 'profile.test.base = monolith\n' > "$TMP_UP"
UP_ERR=$("$BIN" --config "$TMP_UP" --profile nonexistent 2>&1 || true)
echo "$UP_ERR" | grep -Fq "expected one of:" || fail "unknown profile error must list available profiles"
rm -f "$TMP_UP"
pass "Unknown profile error passed"

log "Checking storm remains unavailable"
TMP_STORM="$(mktemp)"
printf 'profile.storm.base = monolith\nprofile.storm.atmosphere-regime = storm\n' > "$TMP_STORM"
STORM_ERR=$("$BIN" --config "$TMP_STORM" --profile storm 2>&1 || true)
echo "$STORM_ERR" | grep -Fq "storm is unavailable" || fail "storm must be reported as unavailable"
rm -f "$TMP_STORM"
pass "Storm unavailability passed"

log "Checking default runtime and writer invariants"
"$BIN" -i | grep -Fq "application_mode: disabled" || fail "default must remain disabled"
"$BIN" -i | grep -Fq "visual_runtime: protected" || fail "default must remain protected"
"$BIN" --benchmark | grep -Eq "terminal_writer: single-owner" || fail "terminal_writer must be single-owner"
"$BIN" --benchmark | grep -Eq "compute_parallelism: disabled" || fail "compute_parallelism must be disabled"
pass "Default runtime and writer invariants passed"

log "All release candidate smoke checks passed"
