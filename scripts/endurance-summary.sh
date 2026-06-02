#!/usr/bin/env bash
# =============================================================================
# COSMOSTRIX ENDURANCE SUMMARY
# =============================================================================
# Reads a CSV resource log produced by the endurance sampling loop and prints
# a summary table with memory growth, FD leak detection, and elapsed time.
#
# CSV format: timestamp,rss_kb,fd_count,elapsed_secs
# Usage: scripts/endurance-summary.sh <csv_file>
# =============================================================================

set -euo pipefail

# --- Argument handling ---

if [[ $# -lt 1 ]]; then
    echo "Usage: $0 <csv_file>" >&2
    echo "  CSV format: timestamp,rss_kb,fd_count,elapsed_secs" >&2
    exit 1
fi

CSV_PATH="$1"

if [[ ! -f "$CSV_PATH" ]]; then
    echo "Error: file not found: $CSV_PATH" >&2
    exit 1
fi

if [[ ! -r "$CSV_PATH" ]]; then
    echo "Error: file not readable: $CSV_PATH" >&2
    exit 1
fi

# Count data lines (excluding header)
DATA_LINES=$(tail -n +2 "$CSV_PATH" | grep -c '[^[:space:]]' || true)

if [[ "$DATA_LINES" -eq 0 ]]; then
    echo "Error: CSV file contains no data rows: $CSV_PATH" >&2
    exit 1
fi

# --- Extract data columns (skip header line) ---

# Read all data lines into arrays
mapfile -t RSS_VALUES < <(tail -n +2 "$CSV_PATH" | awk -F',' '{print $2}' | grep -E '^[0-9]+$')
mapfile -t FD_VALUES < <(tail -n +2 "$CSV_PATH" | awk -F',' '{print $3}' | grep -E '^[0-9]+$')
mapfile -t ELAPSED_VALUES < <(tail -n +2 "$CSV_PATH" | awk -F',' '{print $4}' | grep -E '^[0-9]+$')

if [[ ${#RSS_VALUES[@]} -eq 0 ]]; then
    echo "Error: no valid RSS data rows found in $CSV_PATH" >&2
    exit 1
fi

# --- Compute statistics ---

# Start / end RSS
START_RSS="${RSS_VALUES[0]}"
END_RSS="${RSS_VALUES[-1]}"

# Max RSS
MAX_RSS="${RSS_VALUES[0]}"
for val in "${RSS_VALUES[@]}"; do
    if [[ "$val" -gt "$MAX_RSS" ]]; then
        MAX_RSS="$val"
    fi
done

# RSS growth percentage (end vs start)
if [[ "$START_RSS" -eq 0 ]]; then
    RSS_GROWTH_PCT="N/A (start RSS is 0)"
else
    # bc for floating-point division; fall back to awk
    RSS_GROWTH_PCT=$(awk "BEGIN { printf \"%.2f\", (($END_RSS - $START_RSS) / $START_RSS) * 100 }")
fi

# FD statistics
if [[ ${#FD_VALUES[@]} -gt 0 ]]; then
    START_FD="${FD_VALUES[0]}"
    END_FD="${FD_VALUES[-1]}"

    # Max FD
    MAX_FD="${FD_VALUES[0]}"
    for val in "${FD_VALUES[@]}"; do
        if [[ "$val" -gt "$MAX_FD" ]]; then
            MAX_FD="$val"
        fi
    done

    # FD leak detection: check if fd_count trend is monotonically increasing
    # Allow transient spikes of up to 2 FDs. Leak if end exceeds start by > 2.
    FD_DIFF=$(( END_FD - START_FD ))
    if [[ "$FD_DIFF" -gt 2 ]]; then
        FD_LEAK="YES (leaked +${FD_DIFF} FDs)"
    else
        FD_LEAK="No"
    fi
else
    START_FD="N/A"
    END_FD="N/A"
    MAX_FD="N/A"
    FD_LEAK="N/A"
fi

# Elapsed time
if [[ ${#ELAPSED_VALUES[@]} -gt 0 ]]; then
    ELAPSED_SECS="${ELAPSED_VALUES[-1]}"
    ELAPSED_HOURS=$(awk "BEGIN { printf \"%.1f\", $ELAPSED_SECS / 3600 }")
    if [[ "$ELAPSED_SECS" -ge 86400 ]]; then
        ELAPSED_FMT="${ELAPSED_HOURS}h ($(( ELAPSED_SECS / 86400 ))d $(( (ELAPSED_SECS % 86400) / 3600 ))h)"
    else
        ELAPSED_FMT="${ELAPSED_HOURS}h"
    fi
else
    ELAPSED_SECS="N/A"
    ELAPSED_HOURS="N/A"
    ELAPSED_FMT="N/A"
fi

# Sample count
SAMPLE_COUNT="${#RSS_VALUES[@]}"

# --- Print summary ---

cat <<EOF
================================================================================
  Cosmostrix Endurance Summary
================================================================================
  Source:  ${CSV_PATH}
  Samples: ${SAMPLE_COUNT} data rows
================================================================================

  RSS (Resident Set Size)
  ───────────────────────────────────────────
    Start RSS:     ${START_RSS} kB
    End RSS:       ${END_RSS} kB
    Max RSS:       ${MAX_RSS} kB
    RSS growth:    ${RSS_GROWTH_PCT}%

  File Descriptors
  ───────────────────────────────────────────
    Start FD count:  ${START_FD}
    End FD count:    ${END_FD}
    Max FD count:    ${MAX_FD}
    Leak detected:   ${FD_LEAK}

  Elapsed Time
  ───────────────────────────────────────────
    Total:  ${ELAPSED_FMT} (${ELAPSED_SECS} seconds)

================================================================================
EOF

# --- Pass/fail verdict ---

PASS=true
FAIL_REASONS=""

# Check RSS growth < 2% per hour
if [[ "$RSS_GROWTH_PCT" != "N/A (start RSS is 0)" ]]; then
    # Total growth over the run; compare against 2% per hour * hours elapsed
    HOURLY_RATE=$(awk "BEGIN { printf \"%.2f\", $RSS_GROWTH_PCT / $ELAPSED_HOURS }" 2>/dev/null || echo "N/A")
    HOURLY_LIMIT=$(awk "BEGIN { printf \"%.2f\", $ELAPSED_HOURS * 2.0 }")
    GROWTH_ABS=$(( END_RSS - START_RSS ))
    THRESHOLD_ABS=$(awk "BEGIN { printf \"%.0f\", $START_RSS * $HOURLY_LIMIT / 100 }")

    if (( GROWTH_ABS > THRESHOLD_ABS )); then
        PASS=false
        FAIL_REASONS="${FAIL_REASONS}  - RSS growth ${RSS_GROWTH_PCT}% exceeds ${HOURLY_LIMIT}% budget for ${ELAPSED_HOURS}h\n"
    fi
fi

# Check FD leak
if [[ "$FD_LEAK" != "No" && "$FD_LEAK" != "N/A" ]]; then
    PASS=false
    FAIL_REASONS="${FAIL_REASONS}  - FD leak detected: ${FD_LEAK}\n"
fi

# Print verdict
if [[ "$PASS" == "true" ]]; then
    echo "  VERDICT: PASS"
else
    echo "  VERDICT: FAIL"
    echo ""
    echo "  Failure reasons:"
    echo ""
    printf "%b" "$FAIL_REASONS"
fi

echo "================================================================================"
