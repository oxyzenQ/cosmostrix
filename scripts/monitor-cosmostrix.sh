#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
set -u
set -o pipefail

TARGET="${1:-cosmostrix}"
INTERVAL="${INTERVAL:-60}"
OUT_DIR="${OUT_DIR:-logs}"

mkdir -p "$OUT_DIR"

is_pid() {
  [[ "${1:-}" =~ ^[0-9]+$ ]]
}

resolve_pid() {
  local target="$1"

  if is_pid "$target"; then
    [[ -d "/proc/$target" ]] && echo "$target"
    return
  fi

  # Prefer newest matching process; fall back to oldest via pidof.
  pgrep -xn "$target" 2>/dev/null || pidof "$target" 2>/dev/null | awk '{print $NF}'
}

PID="$(resolve_pid "$TARGET")"

if [[ -z "${PID:-}" ]]; then
  echo "Process not found: $TARGET" >&2
  exit 1
fi

if [[ ! -r "/proc/$PID/status" ]]; then
  echo "Cannot read /proc/$PID/status. Try running as the same user or with sudo." >&2
  exit 1
fi

NAME="$(cat "/proc/$PID/comm" 2>/dev/null || echo "$TARGET")"
START_TS="$(date +%Y%m%d-%H%M%S)"
OUT="$OUT_DIR/${NAME}-resource-${PID}-${START_TS}.csv"

status_kb() {
  local key="$1"
  awk -v k="${key}:" '$1 == k { print $2; found=1; exit } END { if (!found) print 0 }' "/proc/$PID/status" 2>/dev/null
}

status_val() {
  local key="$1"
  awk -v k="${key}:" '$1 == k { print $2; found=1; exit } END { if (!found) print 0 }' "/proc/$PID/status" 2>/dev/null
}

io_val() {
  local key="$1"
  awk -v k="${key}:" '$1 == k { print $2; found=1; exit } END { if (!found) print 0 }' "/proc/$PID/io" 2>/dev/null
}

smaps_kb() {
  local key="$1"
  if [[ -r "/proc/$PID/smaps_rollup" ]]; then
    awk -v k="${key}:" '$1 == k { print $2; found=1; exit } END { if (!found) print 0 }' "/proc/$PID/smaps_rollup" 2>/dev/null
  else
    echo 0
  fi
}

fd_count() {
  if [[ -d "/proc/$PID/fd" ]]; then
    find "/proc/$PID/fd" -maxdepth 1 -type l 2>/dev/null | wc -l | tr -d ' '
  else
    echo 0
  fi
}

total_cpu_ticks() {
  awk '/^cpu / {
    total=0
    for (i=2; i<=NF; i++) total += $i
    print total
    exit
  }' /proc/stat 2>/dev/null
}

proc_cpu_ticks() {
  awk '{
    line=$0
    sub(/^.*\) /, "", line)
    split(line, a, " ")
    # after removing "pid (comm)", a[12]=utime, a[13]=stime
    print a[12] + a[13]
  }' "/proc/$PID/stat" 2>/dev/null
}

proc_faults() {
  awk '{
    line=$0
    sub(/^.*\) /, "", line)
    split(line, a, " ")
    # after removing "pid (comm)", a[8]=minflt, a[10]=majflt
    print a[8] "," a[10]
  }' "/proc/$PID/stat" 2>/dev/null
}

cpu_percent() {
  local prev_proc="$1"
  local now_proc="$2"
  local prev_total="$3"
  local now_total="$4"
  local cores="$5"

  awk \
    -v pp="$prev_proc" \
    -v np="$now_proc" \
    -v pt="$prev_total" \
    -v nt="$now_total" \
    -v cores="$cores" \
    'BEGIN {
      pd = np - pp
      td = nt - pt
      if (td > 0 && pd >= 0) printf "%.2f", (pd / td) * 100 * cores
      else printf "0.00"
    }'
}

CORES="$(nproc 2>/dev/null || echo 1)"
START_SEC="$(date +%s)"

PREV_PROC_TICKS="$(proc_cpu_ticks)"
PREV_TOTAL_TICKS="$(total_cpu_ticks)"

PREV_PROC_TICKS="${PREV_PROC_TICKS:-0}"
PREV_TOTAL_TICKS="${PREV_TOTAL_TICKS:-0}"

echo "Monitoring: name=$NAME pid=$PID interval=${INTERVAL}s output=$OUT" >&2

HEADER="timestamp,pid,elapsed_sec,cpu_pct,rss_kb,hwm_kb,vmsize_kb,rssanon_kb,rssfile_kb,pss_kb,swap_kb,threads,fd_count,minflt,majflt,voluntary_ctxt,nonvoluntary_ctxt,read_bytes,write_bytes"
echo "$HEADER" | tee "$OUT"

while [[ -d "/proc/$PID" ]] && kill -0 "$PID" 2>/dev/null; do
  TS="$(date -Is)"
  NOW_SEC="$(date +%s)"
  ELAPSED="$((NOW_SEC - START_SEC))"

  NOW_PROC_TICKS="$(proc_cpu_ticks)"
  NOW_TOTAL_TICKS="$(total_cpu_ticks)"

  NOW_PROC_TICKS="${NOW_PROC_TICKS:-0}"
  NOW_TOTAL_TICKS="${NOW_TOTAL_TICKS:-0}"

  CPU_PCT="$(cpu_percent "$PREV_PROC_TICKS" "$NOW_PROC_TICKS" "$PREV_TOTAL_TICKS" "$NOW_TOTAL_TICKS" "$CORES")"

  RSS="$(status_kb VmRSS)"
  HWM="$(status_kb VmHWM)"
  VMSIZE="$(status_kb VmSize)"
  RSSANON="$(status_kb RssAnon)"
  RSSFILE="$(status_kb RssFile)"
  THREADS="$(status_val Threads)"
  VOL_CTX="$(status_val voluntary_ctxt_switches)"
  NONVOL_CTX="$(status_val nonvoluntary_ctxt_switches)"
  FD_COUNT="$(fd_count)"
  READ_BYTES="$(io_val read_bytes)"
  WRITE_BYTES="$(io_val write_bytes)"
  PSS="$(smaps_kb Pss)"
  SWAP="$(smaps_kb Swap)"

  FAULTS="$(proc_faults)"
  MINFLT="${FAULTS%%,*}"
  MAJFLT="${FAULTS##*,}"
  MINFLT="${MINFLT:-0}"
  MAJFLT="${MAJFLT:-0}"

  printf '%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s\n' \
    "$TS" "$PID" "$ELAPSED" "$CPU_PCT" \
    "$RSS" "$HWM" "$VMSIZE" "$RSSANON" "$RSSFILE" "$PSS" "$SWAP" \
    "$THREADS" "$FD_COUNT" "$MINFLT" "$MAJFLT" \
    "$VOL_CTX" "$NONVOL_CTX" "$READ_BYTES" "$WRITE_BYTES" \
    | tee -a "$OUT"

  PREV_PROC_TICKS="$NOW_PROC_TICKS"
  PREV_TOTAL_TICKS="$NOW_TOTAL_TICKS"

  sleep "$INTERVAL"
done

echo "Process exited or disappeared: name=$NAME pid=$PID" >&2
