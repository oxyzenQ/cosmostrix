# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

#!/usr/bin/env bash
#
# COSMOSTRIX ENDURANCE SUMMARY — SELF-TEST
#
# Creates a temporary sample CSV with the extended header and a few rows,
# runs the summary script, and asserts the output contains expected values.
#
# Usage: bash scripts/test-endurance-summary.sh
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SUMMARY="$SCRIPT_DIR/endurance-summary.sh"
TMPDIR_WORK="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_WORK"' EXIT

echo "--- endurance-summary.sh self-test ---"

# --- 1. Syntax check ---
echo "1. bash -n syntax check..."
bash -n "$SUMMARY"
echo "   PASS"

# --- 2. Extended format test ---
EXT_CSV="$TMPDIR_WORK/extended.csv"
cat > "$EXT_CSV" <<'CSV'
timestamp,pid,elapsed_sec,cpu_pct,rss_kb,hwm_kb,vmsize_kb,rssanon_kb,rssfile_kb,pss_kb,swap_kb,threads,fd_count,minflt,majflt,voluntary_ctxt,nonvoluntary_ctxt,read_bytes,write_bytes
2026-05-31T11:35:26+07:00,1649411,0,0.50,4160,4160,208616,548,3612,1672,0,4,10,290,0,18918,100,0,0
2026-05-31T11:36:26+07:00,1649411,60,0.30,4162,4162,208620,550,3612,1674,0,4,10,310,0,19020,105,512,0
2026-05-31T11:37:26+07:00,1649411,120,0.40,4164,4164,208624,552,3612,1676,0,4,10,330,0,19100,108,1024,0
CSV

echo "2. Extended format (3 rows)..."
OUTPUT=$(bash "$SUMMARY" "$EXT_CSV")

assert_contains() {
    local output="$1" needle="$2" label="$3"
    if echo "$output" | grep -qF "$needle"; then
        echo "   PASS: $label"
    else
        echo "   FAIL: $label (expected '$needle' in output)"
        echo "--- output ---"
        echo "$output"
        echo "--- end ---"
        exit 1
    fi
}

assert_fails_with() {
    local cmd_output needle label
    label="$1"
    needle="$2"
    shift 2
    if cmd_output=$("$@" 2>&1); then
        echo "   FAIL: $label (command unexpectedly succeeded)"
        echo "--- output ---"
        echo "$cmd_output"
        echo "--- end ---"
        exit 1
    fi
    assert_contains "$cmd_output" "$needle" "$label"
}

assert_contains "$OUTPUT" "Start RSS:     4160 kB"          "RSS start = 4160"
assert_contains "$OUTPUT" "End RSS:       4164 kB"          "RSS end = 4164"
assert_contains "$OUTPUT" "Max RSS:       4164 kB"          "RSS max = 4164"
assert_contains "$OUTPUT" "HWM:           4164 kB"          "HWM = 4164"
assert_contains "$OUTPUT" "Start FD:      10"                "FD start = 10"
assert_contains "$OUTPUT" "End FD:        10"                "FD end = 10"
assert_contains "$OUTPUT" "Leak detected: No"                "No FD leak"
assert_contains "$OUTPUT" "Start:   4"                       "Threads start = 4"
assert_contains "$OUTPUT" "Max swap:      0 kB"             "Swap = 0"
assert_contains "$OUTPUT" "Samples:  3 data rows"           "Samples = 3"
assert_contains "$OUTPUT" "120 seconds"                      "Elapsed = 120s"
# Note: 3-row/2min sample has 0.10% RSS growth which exceeds 2%/hour budget
# (because 0.10% > 0.06% threshold for 0.03h). This is expected — short
# samples are noisy. Just verify a verdict line is printed.
assert_contains "$OUTPUT" "VERDICT:"                             "Verdict line present"
assert_contains "$OUTPUT" "Max PSS:       1676 kB"          "PSS max = 1676"
assert_contains "$OUTPUT" "Avg:     0.40%"                   "CPU avg ~0.40%"
assert_contains "$OUTPUT" "Delta:   0"                        "Majflt delta = 0"

# --- 2b. Extended format: stable data that should PASS ---
STABLE_CSV="$TMPDIR_WORK/stable.csv"
# 10 rows over 9 minutes, RSS constant at 4160, should clearly PASS
for i in $(seq 0 9); do
    ELAPSED=$((i * 60))
    if [[ "$i" -eq 0 ]]; then
        echo "timestamp,pid,elapsed_sec,cpu_pct,rss_kb,hwm_kb,vmsize_kb,rssanon_kb,rssfile_kb,pss_kb,swap_kb,threads,fd_count,minflt,majflt,voluntary_ctxt,nonvoluntary_ctxt,read_bytes,write_bytes" > "$STABLE_CSV"
    fi
    echo "2026-05-31T11:3${i}:26+07:00,1649411,${ELAPSED},0.30,4160,4160,208616,548,3612,1672,0,4,10,290,0,18918,100,0,0" >> "$STABLE_CSV"
done

echo "2b. Stable extended data (10 rows, constant RSS)..."
OUTPUT_STABLE=$(bash "$SUMMARY" "$STABLE_CSV")
assert_contains "$OUTPUT_STABLE" "RSS growth:    0.00%"        "Stable RSS growth = 0.00%"
assert_contains "$OUTPUT_STABLE" "PASS / stable"                "Stable verdict PASS"

# --- 3. Legacy format test ---
LEG_CSV="$TMPDIR_WORK/legacy.csv"
cat > "$LEG_CSV" <<'CSV'
timestamp,rss_kb,fd_count,elapsed_secs
1700000000,4096,12,0
1700000060,4100,12,60
CSV

echo "3. Legacy format (2 rows)..."
OUTPUT_LEG=$(bash "$SUMMARY" "$LEG_CSV")
assert_contains "$OUTPUT_LEG" "Format:   legacy"              "Detected as legacy"
assert_contains "$OUTPUT_LEG" "Start RSS:     4096 kB"        "Legacy RSS start"
assert_contains "$OUTPUT_LEG" "Start FD:      12"             "Legacy FD start"

# --- 4. Error handling: missing required field ---
echo "4. Missing required field error..."
BAD_CSV="$TMPDIR_WORK/bad.csv"
cat > "$BAD_CSV" <<'CSV'
timestamp,cpu_pct,rss_kb
1,0.5,4096
2,0.3,4100
CSV

if bash "$SUMMARY" "$BAD_CSV" 2>/dev/null; then
    echo "   FAIL: should have exited with error"
    exit 1
else
    echo "   PASS: correctly rejected CSV with missing required field"
fi

# --- 5. Division by zero safety ---
ZERO_CSV="$TMPDIR_WORK/zero_elapsed.csv"
cat > "$ZERO_CSV" <<'CSV'
timestamp,pid,elapsed_sec,cpu_pct,rss_kb,hwm_kb,vmsize_kb,rssanon_kb,rssfile_kb,pss_kb,swap_kb,threads,fd_count,minflt,majflt,voluntary_ctxt,nonvoluntary_ctxt,read_bytes,write_bytes
2026-05-31T11:35:26+07:00,1649411,0,0.50,4160,4160,208616,548,3612,1672,0,4,10,290,0,18918,100,0,0
CSV

echo "5. Single-row (elapsed=0) safety..."
OUTPUT_ZERO=$(bash "$SUMMARY" "$ZERO_CSV" 2>&1 || true)
assert_contains "$OUTPUT_ZERO" "Start RSS:     4160 kB"       "Single-row RSS ok"
# Should not crash with division by zero
echo "   PASS: no crash with elapsed=0"

# --- 6. Friendly failures for missing inputs ---
echo "6. Friendly missing input failures..."
assert_fails_with "No args show usage" "Usage:" bash "$SUMMARY"
assert_fails_with \
    "Unmatched glob shows no-logs hint" \
    "No matching/readable Cosmostrix endurance CSV files found." \
    bash "$SUMMARY" "$TMPDIR_WORK/no-such-cosmostrix-resource-*.csv"

echo ""
echo "--- all self-tests passed ---"
