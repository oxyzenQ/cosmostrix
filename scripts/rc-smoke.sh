#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# ─────────────────────────────────────────────────────────────────────────────
# PLATFORM: UNIX-only (Linux, macOS, BSD).
#   Spawns the cosmostrix binary, pipes through `grep -Fq`, uses `mktemp`.
#   Not for Windows cmd.exe / PowerShell — use Git Bash or WSL on Windows.
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

# rc-smoke.sh — Release candidate smoke tests
# Non-destructive pre-release verification. Safe to run locally or in CI.
# Usage: bash scripts/rc-smoke.sh [BINARY_PATH]

BIN="${1:-target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix}"

log() { printf '[INFO] %s\n' "$*"; }
pass() { printf '[PASS] %s\n' "$*"; }
fail() { printf '[FAIL] %s\n' "$*" >&2; exit 1; }

[[ -x "$BIN" ]] || fail "Binary not found or not executable: $BIN"

# v30 (2026-08-05): cosmostrix enforces a strict config-path whitelist
# (~/.config/cosmostrix/, /etc/cosmostrix/, etc.). mktemp creates files in
# /tmp which is rejected. We use a per-run subdirectory inside the user's
# config dir instead, and clean it up on exit via a trap.
RC_SMOKE_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/cosmostrix/rc-smoke.$$"
mkdir -p "$RC_SMOKE_DIR"
trap 'rm -rf "$RC_SMOKE_DIR"' EXIT

# ── Version check ─────────────────────────────────────────────────────────

log "Checking version output"
# v30 (2026-08-05): version_report() emits "cosmostrix: v<version>" as the
# first line. The historical "Version: v" pattern was from a pre-v30 format.
"$BIN" -V | grep -Fq "cosmostrix: v" || fail "Missing 'cosmostrix: v' header in -V output"
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

# ── Standard config + doctor smoke ────────────────────────────────────────
# Replaces the old "controlled-live config smoke" block. The atmosphere-mode
# and atmosphere-regime config keys were eliminated at commit 07b44b5 (Dragon
# Hunt v2 Phase 6 Tier E item 31 — atmosphere engine subsystem fully removed).
# A standard valid config is now used to verify --doctor reads it cleanly.

log "Checking standard config + doctor smoke"
TMP_CL="$RC_SMOKE_DIR/cl.toml"
printf 'scene = monolith\ncolor = sun\n' > "$TMP_CL"

# v17: --info removed. Check --doctor for build/renderer fields.
"$BIN" --config "$TMP_CL" --doctor | grep -Fq "BUILD" || fail "doctor should have BUILD section"
"$BIN" --config "$TMP_CL" --doctor | grep -Fq "RENDERER" || fail "doctor should have RENDERER section"
pass "Standard config + doctor smoke passed"

# CLI color override with standard config
TMP_CL2="$RC_SMOKE_DIR/cl2.toml"
printf 'scene = monolith\ncolor = cosmos\n' > "$TMP_CL2"

log "Checking CLI color override with standard config"
"$BIN" --config "$TMP_CL2" --color sun --doctor | grep -Fq "variant:" || fail "doctor should show variant field"
pass "CLI color override with standard config passed"

# ── Eliminated atmosphere keys are rejected ───────────────────────────────
# v30 (2026-08-05, atmosphere elimination): atmosphere-mode and
# atmosphere-regime config keys must be rejected with a clear migration
# message. Replaces the old "Disabled + non-Calm config smoke" block which
# tested the now-deleted atmosphere runtime path.

log "Checking eliminated atmosphere-mode/regime keys are rejected"
TMP_DIS="$RC_SMOKE_DIR/dis.toml"
printf 'atmosphere-mode = disabled\natmosphere-regime = pulse\n' > "$TMP_DIS"

DIS_ERR="$("$BIN" --config "$TMP_DIS" --testconf 2>&1 || true)"
echo "$DIS_ERR" | grep -Fq "unknown key" || fail "atmosphere-mode/atmosphere-regime must be rejected as unknown keys by --testconf"
pass "Eliminated atmosphere keys rejection passed"

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
# v30 (2026-08-05, atmosphere elimination): the historical ATMOSPHERE_ENGINE.md
# reference was removed from --dump-config because the doc was archived to
# docs/archive/specs/. The dump must NOT advertise the dead atmosphere
# subsystem as if it were a live feature.
! echo "$DUMP_V47" | grep -Fq "adaptive-custom.0" || fail "--dump-config must not advertise the eliminated adaptive-custom.* keys as a working example"
pass "--dump-config scene-custom pointers passed"

log "Checking unknown custom scene error mentions --list-scenes"
TMP_UP="$RC_SMOKE_DIR/up.toml"
printf 'scene-custom.test.base = monolith\n' > "$TMP_UP"
UP_ERR=$("$BIN" --config "$TMP_UP" --scene-custom nonexistent 2>&1 || true)
echo "$UP_ERR" | grep -Fq "expected one of:" || fail "unknown custom scene error must list available names"
pass "Unknown custom scene error passed"

log "Checking storm scene-custom atmosphere-regime field is rejected"
# v30 (2026-08-05, atmosphere elimination): the atmosphere-regime field on
# scene-custom blocks was removed along with the atmosphere engine. A user
# who still has it in their config must get a clear rejection (unknown key).
TMP_STORM="$RC_SMOKE_DIR/storm.toml"
printf 'scene-custom.storm.base = monolith\nscene-custom.storm.atmosphere-regime = storm\n' > "$TMP_STORM"
STORM_ERR=$("$BIN" --config "$TMP_STORM" --testconf 2>&1 || true)
echo "$STORM_ERR" | grep -Fq "unknown key" || fail "scene-custom.<name>.atmosphere-regime must be rejected as an unknown key (field eliminated with atmosphere engine)"
pass "Storm atmosphere-regime rejection passed"

log "Checking default runtime and writer invariants"
# v30 (2026-08-05, atmosphere elimination): the --doctor "status:" line was
# part of the atmosphere diagnostic section, which was removed. Replaced
# with a check on RENDERER + CAPACITY, which are the live post-elimination
# diagnostic sections every --doctor run must emit. The benchmark
# `compute_parallelism: disabled` field was also removed (it was part of
# the atmosphere diagnostic block); only `terminal_writer: single-owner`
# and `actual_execution: single-threaded-renderer` remain as the live
# writer/parallelism contract.
"$BIN" --doctor | grep -Fq "RENDERER" || fail "doctor must have RENDERER section"
"$BIN" --doctor | grep -Fq "CAPACITY" || fail "doctor must have CAPACITY section"
"$BIN" --benchmark | grep -Eq "terminal_writer: single-owner" || fail "terminal_writer must be single-owner"
"$BIN" --benchmark | grep -Eq "actual_execution: single-threaded-renderer" || fail "actual_execution must be single-threaded-renderer"
pass "Default runtime and writer invariants passed"

log "All release candidate smoke checks passed"
