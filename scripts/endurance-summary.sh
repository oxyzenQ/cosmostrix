#!/usr/bin/env bash

# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-or-later
# =============================================================================
# COSMOSTRIX ENDURANCE SUMMARY
# =============================================================================
# Reads a CSV resource log produced by the endurance sampling loop and prints
# a summary table with memory growth, FD leak detection, CPU stats, I/O
# deltas, and elapsed time.
#
# Supports two CSV formats:
#   1. Extended (current): timestamp,pid,elapsed_sec,cpu_pct,rss_kb,...
#   2. Legacy (4-column):  timestamp,rss_kb,fd_count,elapsed_sec(s)
#
# Columns are resolved by header name, not hardcoded positions.
# Usage: scripts/endurance-summary.sh <csv_file> [csv_file ...]
# =============================================================================

set -euo pipefail

# --- Helpers ---

die() {
    echo "Error: $*" >&2
    exit 1
}

usage() {
    echo "Usage: $0 <csv_file> [csv_file ...]" >&2
    echo "  Parses Cosmostrix resource CSV logs and prints endurance summary." >&2
    echo "  Example: bash scripts/endurance-summary.sh '../logs/cosmostrix-resource-*.csv'" >&2
}

no_logs_found() {
    echo "No matching/readable Cosmostrix endurance CSV files found." >&2
    echo "" >&2
    usage
    echo "" >&2
    echo "Create logs with an endurance sampling run, then summarize them here." >&2
    echo "See docs/ENDURANCE.md for the logging method and CSV format." >&2
    exit 1
}

has_glob_chars() {
    [[ "$1" == *'*'* || "$1" == *'?'* || "$1" == *'['* ]]
}

# Find the 1-based index of a column name in a comma-separated header line.
# Usage: col_index "header_line" "column_name"
# Prints the index (1-based) or 0 if not found.
col_index() {
    local header="$1" name="$2"
    local i=1
    local IFS=','
    for field in $header; do
        if [[ "$field" == "$name" ]]; then
            echo "$i"
            return 0
        fi
        ((i++)) || true
    done
    echo "0"
}

# Extract a column by name from data lines (skip header).
# Usage: extract_col <csv_path> <header_line> <col_name> [filter_regex]
# Prints one value per line (numeric only when filter_regex provided).
extract_col() {
    local csv="$1" header="$2" name="$3" filter="${4:-}"
    local idx
    idx=$(col_index "$header" "$name")
    if [[ "$idx" -eq 0 ]]; then
        return 1  # column not found
    fi
    if [[ -n "$filter" ]]; then
        tail -n +2 "$csv" | awk -F',' "{print \$${idx}}" | grep -E "$filter" || true
    else
        tail -n +2 "$csv" | awk -F',' "{print \$${idx}}" || true
    fi
}

# Integer-safe min/max across space-separated values.
# Usage: int_min "a b c"  -> prints smallest
# Usage: int_max "a b c"  -> prints largest
int_min() {
    local first=true min=0
    for v in $1; do
        [[ -z "$v" ]] && continue
        if $first; then min="$v"; first=false; else
            (( v < min )) && min="$v"
        fi
    done
    echo "$min"
}

int_max() {
    local first=false max=0
    for v in $1; do
        [[ -z "$v" ]] && continue
        first=true
        (( v > max )) && max="$v"
    done
    $first && echo "$max" || echo "0"
}

# Percentile (0-100) over space-separated numeric values.
# Simple sort-based approach; good enough for summary stats.
percentile() {
    local pct="$1"; shift
    local vals=($*)
    local n=${#vals[@]}
    if [[ "$n" -eq 0 ]]; then echo "0"; return; fi
    # Sort numerically
    local sorted
    sorted=$(printf '%s\n' "${vals[@]}" | sort -n | tr '\n' ' ')
    local sorted_arr=($sorted)
    local idx=$(( n * pct / 100 ))
    # Clamp
    (( idx < 0 )) && idx=0
    (( idx >= n )) && idx=$((n - 1))
    echo "${sorted_arr[$idx]}"
}

# Format seconds into human-readable duration.
# Usage: fmt_duration <total_seconds>
fmt_duration() {
    local total="$1"
    local d h m s
    d=$(( total / 86400 ))
    h=$(( (total % 86400) / 3600 ))
    m=$(( (total % 3600) / 60 ))
    s=$(( total % 60 ))
    local parts=""
    (( d > 0 )) && parts="${parts}${d}d "
    (( h > 0 )) && parts="${parts}${h}h "
    (( m > 0 )) && parts="${parts}${m}m "
    parts="${parts}${s}s"
    echo "$parts"
}

# --- Argument handling ---

if [[ $# -lt 1 ]]; then
    usage
    exit 1
fi

CSV_PATHS=()
for ARG in "$@"; do
    if has_glob_chars "$ARG"; then
        mapfile -t MATCHES < <(compgen -G "$ARG" || true)
        if [[ ${#MATCHES[@]} -eq 0 ]]; then
            continue
        fi
        for MATCH in "${MATCHES[@]}"; do
            CSV_PATHS+=("$MATCH")
        done
    else
        CSV_PATHS+=("$ARG")
    fi
done

if [[ ${#CSV_PATHS[@]} -eq 0 ]]; then
    no_logs_found
fi

# --- Process each CSV file ---

for CSV_PATH in "${CSV_PATHS[@]}"; do
    if [[ ! -f "$CSV_PATH" ]]; then
        die "file not found: $CSV_PATH"
    fi
    if [[ ! -r "$CSV_PATH" ]]; then
        die "file not readable: $CSV_PATH"
    fi

    DATA_LINES=$(tail -n +2 "$CSV_PATH" | grep -c '[^[:space:]]' || true)
    if [[ "$DATA_LINES" -eq 0 ]]; then
        die "CSV file contains no data rows: $CSV_PATH"
    fi

    # --- Detect format and read header ---

    HEADER=$(head -1 "$CSV_PATH")

    # Detect legacy vs extended by checking for known extended-only columns
    HAS_PID=false
    HAS_CPU=false
    HAS_HWM=false
    HAS_PSS=false
    HAS_SWAP=false
    HAS_THREADS=false
    HAS_MAJFLT=false
    HAS_READ_BYTES=false
    HAS_WRITE_BYTES=false

    # Check which columns exist
    [[ $(col_index "$HEADER" "pid")          -gt 0 ]] && HAS_PID=true
    [[ $(col_index "$HEADER" "cpu_pct")     -gt 0 ]] && HAS_CPU=true
    [[ $(col_index "$HEADER" "hwm_kb")      -gt 0 ]] && HAS_HWM=true
    [[ $(col_index "$HEADER" "pss_kb")      -gt 0 ]] && HAS_PSS=true
    [[ $(col_index "$HEADER" "swap_kb")     -gt 0 ]] && HAS_SWAP=true
    [[ $(col_index "$HEADER" "threads")     -gt 0 ]] && HAS_THREADS=true
    [[ $(col_index "$HEADER" "majflt")      -gt 0 ]] && HAS_MAJFLT=true
    [[ $(col_index "$HEADER" "read_bytes")  -gt 0 ]] && HAS_READ_BYTES=true
    [[ $(col_index "$HEADER" "write_bytes") -gt 0 ]] && HAS_WRITE_BYTES=true

    # Determine format label
    if $HAS_PID || $HAS_CPU || $HAS_HWM; then
        FORMAT="extended"
    else
        FORMAT="legacy"
    fi

    # --- Validate required fields ---

    if $HAS_PID; then
        # Extended format: required columns are elapsed_sec, rss_kb, fd_count
        REQUIRED=("elapsed_sec" "rss_kb" "fd_count")
    else
        # Legacy format: required columns are rss_kb, fd_count, and either elapsed_sec or elapsed_secs
        REQUIRED=()
        # Check rss_kb
        if [[ $(col_index "$HEADER" "rss_kb") -eq 0 ]]; then
            die "required column 'rss_kb' not found in header of $CSV_PATH"
        fi
        REQUIRED+=("rss_kb")
        # Check fd_count
        if [[ $(col_index "$HEADER" "fd_count") -eq 0 ]]; then
            die "required column 'fd_count' not found in header of $CSV_PATH"
        fi
        REQUIRED+=("fd_count")
        # Check elapsed_sec or elapsed_secs
        if [[ $(col_index "$HEADER" "elapsed_sec") -eq 0 ]] && \
           [[ $(col_index "$HEADER" "elapsed_secs") -eq 0 ]]; then
            die "required column 'elapsed_sec' or 'elapsed_secs' not found in header of $CSV_PATH"
        fi
    fi

    # --- Extract column data ---

    INT_RE='^[0-9]+$'
    FLOAT_RE='^[0-9]+(\.[0-9]+)?$'

    # Elapsed time: try elapsed_sec first, then elapsed_secs (legacy)
    ELAPSED_COL="elapsed_sec"
    if [[ $(col_index "$HEADER" "elapsed_sec") -eq 0 ]]; then
        ELAPSED_COL="elapsed_secs"
    fi
    mapfile -t ELAPSED_VALUES < <(extract_col "$CSV_PATH" "$HEADER" "$ELAPSED_COL" "$INT_RE")

    # RSS
    mapfile -t RSS_VALUES < <(extract_col "$CSV_PATH" "$HEADER" "rss_kb" "$INT_RE")
    if [[ ${#RSS_VALUES[@]} -eq 0 ]]; then
        die "no valid rss_kb data rows found in $CSV_PATH"
    fi

    # FD count
    FD_VALUES=()
    FD_RAW=$(extract_col "$CSV_PATH" "$HEADER" "fd_count" "$INT_RE" || true)
    if [[ -n "$FD_RAW" ]]; then
        mapfile -t FD_VALUES <<< "$FD_RAW"
    fi

    # CPU
    CPU_VALUES=()
    if $HAS_CPU; then
        mapfile -t CPU_VALUES < <(extract_col "$CSV_PATH" "$HEADER" "cpu_pct" "$FLOAT_RE")
    fi

    # HWM
    HWM_VALUES=()
    if $HAS_HWM; then
        mapfile -t HWM_VALUES < <(extract_col "$CSV_PATH" "$HEADER" "hwm_kb" "$INT_RE")
    fi

    # PSS
    PSS_VALUES=()
    if $HAS_PSS; then
        mapfile -t PSS_VALUES < <(extract_col "$CSV_PATH" "$HEADER" "pss_kb" "$INT_RE")
    fi

    # Swap
    SWAP_VALUES=()
    if $HAS_SWAP; then
        mapfile -t SWAP_VALUES < <(extract_col "$CSV_PATH" "$HEADER" "swap_kb" "$INT_RE")
    fi

    # Threads
    THREADS_VALUES=()
    if $HAS_THREADS; then
        mapfile -t THREADS_VALUES < <(extract_col "$CSV_PATH" "$HEADER" "threads" "$INT_RE")
    fi

    # Major page faults
    MAJFLT_VALUES=()
    if $HAS_MAJFLT; then
        mapfile -t MAJFLT_VALUES < <(extract_col "$CSV_PATH" "$HEADER" "majflt" "$INT_RE")
    fi

    # I/O bytes
    READ_VALUES=()
    WRITE_VALUES=()
    if $HAS_READ_BYTES; then
        mapfile -t READ_VALUES < <(extract_col "$CSV_PATH" "$HEADER" "read_bytes" "$INT_RE")
    fi
    if $HAS_WRITE_BYTES; then
        mapfile -t WRITE_VALUES < <(extract_col "$CSV_PATH" "$HEADER" "write_bytes" "$INT_RE")
    fi

    # --- Compute statistics ---

    SAMPLE_COUNT="${#RSS_VALUES[@]}"

    # -- Elapsed --
    if [[ ${#ELAPSED_VALUES[@]} -gt 0 ]]; then
        ELAPSED_SEC="${ELAPSED_VALUES[-1]}"
        ELAPSED_HUMAN=$(fmt_duration "$ELAPSED_SEC")
    else
        ELAPSED_SEC="N/A"
        ELAPSED_HUMAN="N/A"
    fi

    # -- RSS --
    START_RSS="${RSS_VALUES[0]}"
    END_RSS="${RSS_VALUES[-1]}"
    RSS_ALL="${RSS_VALUES[*]}"
    MAX_RSS=$(int_max "$RSS_ALL")

    if [[ "$START_RSS" -eq 0 ]]; then
        RSS_GROWTH_PCT="N/A"
        RSS_GROWTH_PCT_RAW="-1"
    else
        RSS_GROWTH_PCT=$(awk "BEGIN { printf \"%.2f\", (($END_RSS - $START_RSS) / $START_RSS) * 100 }")
        RSS_GROWTH_PCT_RAW=$(awk "BEGIN { printf \"%.4f\", (($END_RSS - $START_RSS) / $START_RSS) * 100 }")
    fi

    # -- HWM --
    HWM_MAX="N/A"
    if [[ ${#HWM_VALUES[@]} -gt 0 ]]; then
        HWM_MAX=$(int_max "${HWM_VALUES[*]}")
    fi

    # -- PSS --
    START_PSS="N/A"; END_PSS="N/A"; MAX_PSS="N/A"
    if [[ ${#PSS_VALUES[@]} -gt 0 ]]; then
        START_PSS="${PSS_VALUES[0]}"
        END_PSS="${PSS_VALUES[-1]}"
        MAX_PSS=$(int_max "${PSS_VALUES[*]}")
    fi

    # -- Swap --
    SWAP_MAX="N/A"
    if [[ ${#SWAP_VALUES[@]} -gt 0 ]]; then
        SWAP_MAX=$(int_max "${SWAP_VALUES[*]}")
    fi

    # -- File Descriptors --
    START_FD="N/A"; END_FD="N/A"; MAX_FD="N/A"; FD_LEAK="N/A"
    if [[ ${#FD_VALUES[@]} -gt 0 ]]; then
        START_FD="${FD_VALUES[0]}"
        END_FD="${FD_VALUES[-1]}"
        MAX_FD=$(int_max "${FD_VALUES[*]}")
        FD_DIFF=$(( END_FD - START_FD ))
        if [[ "$FD_DIFF" -gt 2 ]]; then
            FD_LEAK="YES (+${FD_DIFF})"
        else
            FD_LEAK="No"
        fi
    fi

    # -- Threads --
    START_THREADS="N/A"; END_THREADS="N/A"; MAX_THREADS="N/A"
    if [[ ${#THREADS_VALUES[@]} -gt 0 ]]; then
        START_THREADS="${THREADS_VALUES[0]}"
        END_THREADS="${THREADS_VALUES[-1]}"
        MAX_THREADS=$(int_max "${THREADS_VALUES[*]}")
    fi

    # -- CPU --
    CPU_AVG="N/A"; CPU_MIN="N/A"; CPU_MAX="N/A"; CPU_P95="N/A"
    if [[ ${#CPU_VALUES[@]} -gt 0 ]]; then
        # Single awk call for avg/min/max, plus sort for P95
        CPU_STATS=$(printf '%s\n' "${CPU_VALUES[@]}" | awk '
            BEGIN { min=999999999; max=-1; sum=0; n=0 }
            { sum+=$1; n++; if ($1<min) min=$1; if ($1>max) max=$1 }
            END { printf "%.2f %.2f %.2f %d", sum/n, min, max, n }
        ')
        CPU_AVG=$(echo "$CPU_STATS" | awk '{print $1}')
        CPU_MIN=$(echo "$CPU_STATS" | awk '{print $2}')
        CPU_MAX=$(echo "$CPU_STATS" | awk '{print $3}')
        CPU_N=$(echo "$CPU_STATS" | awk '{print $4}')
        CPU_P95=$(printf '%s\n' "${CPU_VALUES[@]}" | sort -n | awk -v n="$CPU_N" '
            { vals[NR]=$1 }
            END { idx=int(n*95/100); if(idx>=NR) idx=NR-1; if(idx<0) idx=0; print vals[idx+1] }
        ')
    fi

    # -- Major page faults --
    MAJFLT_DELTA="N/A"; MAJFLT_MAX="N/A"
    if [[ ${#MAJFLT_VALUES[@]} -gt 0 ]]; then
        START_MAJFLT="${MAJFLT_VALUES[0]}"
        END_MAJFLT="${MAJFLT_VALUES[-1]}"
        MAJFLT_DELTA=$(( END_MAJFLT - START_MAJFLT ))
        MAJFLT_MAX=$(int_max "${MAJFLT_VALUES[*]}")
    fi

    # -- I/O bytes --
    READ_DELTA="N/A"; WRITE_DELTA="N/A"
    if [[ ${#READ_VALUES[@]} -gt 1 ]]; then
        READ_DELTA=$(( READ_VALUES[-1] - READ_VALUES[0] ))
    fi
    if [[ ${#WRITE_VALUES[@]} -gt 1 ]]; then
        WRITE_DELTA=$(( WRITE_VALUES[-1] - WRITE_VALUES[0] ))
    fi

    # --- Print summary ---

    cat <<EOF
================================================================================
  Cosmostrix Endurance Summary
================================================================================
  Source:   ${CSV_PATH}
  Format:   ${FORMAT}
  Samples:  ${SAMPLE_COUNT} data rows
================================================================================

  Elapsed Time
  ───────────────────────────────────────────
    Total:  ${ELAPSED_HUMAN} (${ELAPSED_SEC} seconds)

  RSS (Resident Set Size)
  ───────────────────────────────────────────
    Start RSS:     ${START_RSS} kB
    End RSS:       ${END_RSS} kB
    Max RSS:       ${MAX_RSS} kB
    HWM:           ${HWM_MAX} kB
    RSS growth:    ${RSS_GROWTH_PCT}%
EOF

    # PSS section (only if available)
    if [[ "$START_PSS" != "N/A" ]]; then
        cat <<EOF

  PSS (Proportional Set Size)
  ───────────────────────────────────────────
    Start PSS:     ${START_PSS} kB
    End PSS:       ${END_PSS} kB
    Max PSS:       ${MAX_PSS} kB
EOF
    fi

    # Swap section (only if available)
    if [[ "$SWAP_MAX" != "N/A" ]]; then
        cat <<EOF

  Swap
  ───────────────────────────────────────────
    Max swap:      ${SWAP_MAX} kB
EOF
    fi

    cat <<EOF

  File Descriptors
  ───────────────────────────────────────────
    Start FD:      ${START_FD}
    End FD:        ${END_FD}
    Max FD:        ${MAX_FD}
    Leak detected: ${FD_LEAK}
EOF

    # Threads section (only if available)
    if [[ "$START_THREADS" != "N/A" ]]; then
        cat <<EOF

  Threads
  ───────────────────────────────────────────
    Start:   ${START_THREADS}
    End:     ${END_THREADS}
    Max:     ${MAX_THREADS}
EOF
    fi

    # CPU section (only if available)
    if [[ "$CPU_AVG" != "N/A" ]]; then
        cat <<EOF

  CPU Usage
  ───────────────────────────────────────────
    Avg:     ${CPU_AVG}%
    Min:     ${CPU_MIN}%
    Max:     ${CPU_MAX}%
    P95:     ${CPU_P95}%
EOF
    fi

    # Page faults section (only if available)
    if [[ "$MAJFLT_DELTA" != "N/A" ]]; then
        cat <<EOF

  Major Page Faults
  ───────────────────────────────────────────
    Delta:   ${MAJFLT_DELTA}
    Max:     ${MAJFLT_MAX}
EOF
    fi

    # I/O section (only if available)
    if [[ "$READ_DELTA" != "N/A" ]] || [[ "$WRITE_DELTA" != "N/A" ]]; then
        cat <<EOF

  I/O Bytes
  ───────────────────────────────────────────
    Read delta:    ${READ_DELTA} bytes
    Write delta:  ${WRITE_DELTA} bytes
EOF
    fi

    # --- Pass/fail verdict ---

    PASS=true
    FAIL_REASONS=""

    # Check RSS growth < 2% per hour
    if [[ "$RSS_GROWTH_PCT_RAW" != "-1" && "$ELAPSED_SEC" != "N/A" && "$ELAPSED_SEC" -gt 0 ]]; then
        ELAPSED_HOURS=$(awk "BEGIN { printf \"%.2f\", $ELAPSED_SEC / 3600 }")
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

    # Check swap (non-zero swap can indicate memory pressure)
    if [[ "$SWAP_MAX" != "N/A" && "$SWAP_MAX" -gt 0 ]]; then
        PASS=false
        FAIL_REASONS="${FAIL_REASONS}  - Non-zero swap detected: max ${SWAP_MAX} kB\n"
    fi

    # Print verdict
    if [[ "$PASS" == "true" ]]; then
        echo "  VERDICT: PASS / stable"
    else
        echo "  VERDICT: FAIL"
        echo ""
        echo "  Failure reasons:"
        echo ""
        printf "%b" "$FAIL_REASONS"
    fi

    echo "================================================================================"
done
