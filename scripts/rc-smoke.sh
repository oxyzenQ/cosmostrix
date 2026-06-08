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

# ── README / CHANGELOG / casing audit ────────────────────────────────────

log "Checking README guards"
[[ -f "README.md" ]] || fail "README.md not found"
grep -Fq "CHANGELOG.md" README.md || fail "README must link to CHANGELOG.md"
! grep -Eq "^#+ Release notes" README.md || fail "README must not contain release notes section"
! grep -Eq "^### v2\.[0-9]+\.[0-9]+" README.md || fail "README must not contain v2.x.x release headings"
pass "README guards passed"

log "Checking casing audit"
# Scan docs, source, and metadata for wrong-cased repo owner.
# The canonical casing is github.com/oxyzenQ (capital Q).
BAD_CASING=0
for SCAN_DIR in docs src; do
    if [[ -d "$SCAN_DIR" ]]; then
        if grep -rq "github.com/oxyzenq" "$SCAN_DIR" 2>/dev/null; then
            BAD_CASING=1
        fi
    fi
done
for SCAN_FILE in README.md CHANGELOG.md Cargo.toml; do
    if [[ -f "$SCAN_FILE" ]]; then
        if grep -q "github.com/oxyzenq" "$SCAN_FILE" 2>/dev/null; then
            BAD_CASING=1
        fi
    fi
done
if [[ "$BAD_CASING" -eq 1 ]]; then
    fail "Wrong-cased repo owner found"
fi
pass "Casing audit clean"

log "All release candidate smoke checks passed"
