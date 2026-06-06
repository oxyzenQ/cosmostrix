# Endurance Testing

## Purpose

Cosmostrix is designed to run as a long-lived terminal screensaver. This document
describes the endurance testing methodology used to verify that the renderer
remains stable for sessions exceeding 24 hours without memory growth, handle
leaks, or crashes.

## Test methodology

A Cosmostrix binary is launched in headless mode with a configurable duration
cap. A lightweight sampling script reads `/proc/<pid>/status`,
`/proc/<pid>/stat`, `/proc/<pid>/statm`, `/proc/<pid>/fd`, and
`/proc/<pid>/io` at regular intervals and appends a single CSV row per sample.

Store endurance logs outside the repository root when practical, for example
`../logs/cosmostrix-resource-YYYYMMDD-HHMM.csv`. Keeping logs in a sibling
`logs/` directory avoids mixing large runtime artifacts with source files.

### Resource log format (CSV)

The current (extended) CSV format contains 19 fields per row:

```
timestamp,pid,elapsed_sec,cpu_pct,rss_kb,hwm_kb,vmsize_kb,rssanon_kb,rssfile_kb,pss_kb,swap_kb,threads,fd_count,minflt,majflt,voluntary_ctxt,nonvoluntary_ctxt,read_bytes,write_bytes
```

| Field | Description |
|---|---|
| `timestamp` | ISO 8601 timestamp with timezone offset (e.g. `2026-05-31T11:35:26+07:00`) |
| `pid` | Process ID of the running Cosmostrix instance |
| `elapsed_sec` | Seconds since the first sample (monotonic) |
| `cpu_pct` | CPU usage percentage from `/proc/<pid>/stat` |
| `rss_kb` | Resident Set Size in kB from `VmRSS` in `/proc/<pid>/status` |
| `hwm_kb` | Peak RSS (VmHWM) in kB from `/proc/<pid>/status` |
| `vmsize_kb` | Virtual memory size in kB from `VmSize` in `/proc/<pid>/status` |
| `rssanon_kb` | Anonymous RSS in kB from `RssAnon` in `/proc/<pid>/status` |
| `rssfile_kb` | File-backed RSS in kB from `RssFile` in `/proc/<pid>/status` |
| `pss_kb` | Proportional Set Size in kB from `/proc/<pid>/statm` |
| `swap_kb` | Swap usage in kB from `VmSwap` in `/proc/<pid>/status` |
| `threads` | Number of threads from `Threads` in `/proc/<pid>/status` |
| `fd_count` | Number of open file descriptors from `ls /proc/<pid>/fd \| wc -l` |
| `minflt` | Minor page faults (cumulative) from `/proc/<pid>/stat` |
| `majflt` | Major page faults (cumulative) from `/proc/<pid>/stat` |
| `voluntary_ctxt` | Voluntary context switches (cumulative) from `/proc/<pid>/stat` |
| `nonvoluntary_ctxt` | Involuntary context switches (cumulative) from `/proc/<pid>/stat` |
| `read_bytes` | Bytes read (cumulative) from `/proc/<pid>/io` |
| `write_bytes` | Bytes written (cumulative) from `/proc/<pid>/io` |

#### Legacy format

The summary script also supports the legacy 4-column format for backward
compatibility:

```
timestamp,rss_kb,fd_count,elapsed_secs
```

The script auto-detects the format based on the presence of extended columns
(`pid`, `cpu_pct`, `hwm_kb`, etc.).

### Sampling interval

The default interval is 60 seconds. For shorter test runs (e.g. 1-hour
smoke tests) a 10-second interval provides higher resolution.

### How to run

1. Build a release binary:

```bash
cargo build --release
```

2. Launch Cosmostrix in the background with a duration cap:

```bash
./target/release/cosmostrix --duration 86400 &
COSMO_PID=$!
```

3. Start sampling (run from a separate terminal or via nohup):

```bash
CSV_PATH="endurance-24h.csv"

# Write extended CSV header
echo "timestamp,pid,elapsed_sec,cpu_pct,rss_kb,hwm_kb,vmsize_kb,rssanon_kb,rssfile_kb,pss_kb,swap_kb,threads,fd_count,minflt,majflt,voluntary_ctxt,nonvoluntary_ctxt,read_bytes,write_bytes" > "$CSV_PATH"

START_TS=$(date +%s)

while kill -0 "$COSMO_PID" 2>/dev/null; do
  NOW_TS=$(date +%s)
  ELAPSED=$(( NOW_TS - START_TS ))

  # Read /proc fields
  RSS=$(awk '/^VmRSS:/ {print $2}' "/proc/$COSMO_PID/status")
  HWM=$(awk '/^VmHWM:/ {print $2}' "/proc/$COSMO_PID/status")
  VMSIZE=$(awk '/^VmSize:/ {print $2}' "/proc/$COSMO_PID/status")
  RSSANON=$(awk '/^RssAnon:/ {print $2}' "/proc/$COSMO_PID/status")
  RSSFILE=$(awk '/^RssFile:/ {print $2}' "/proc/$COSMO_PID/status")
  VMSWAP=$(awk '/^VmSwap:/ {print $2}' "/proc/$COSMO_PID/status")
  THREADS=$(awk '/^Threads:/ {print $2}' "/proc/$COSMO_PID/status")

  # PSS from statm (field 3, in pages; page size typically 4096)
  PSS_PAGES=$(awk '{print $3}' "/proc/$COSMO_PID/statm")
  PSS=$(( PSS_PAGES * 4 ))  # kB (assuming 4 kB pages)

  # CPU and faults from stat
  STAT_DATA=$(cat "/proc/$COSMO_PID/stat")
  MINFLT=$(echo "$STAT_DATA" | awk '{print $10}')
  MAJFLT=$(echo "$STAT_DATA" | awk '{print $12}')
  VCTX=$(echo "$STAT_DATA" | awk '{print $44}')
  NVCTX=$(echo "$STAT_DATA" | awk '{print $45}')
  UTIME=$(echo "$STAT_DATA" | awk '{print $14}')
  STIME=$(echo "$STAT_DATA" | awk '{print $15}')
  # CPU pct = (utime+stime) / elapsed_ticks * 100; use jiffies (typically 100 Hz)
  CLOCKS=$(getconf CLK_TCK 2>/dev/null || echo 100)
  CPU_PCT=$(awk "BEGIN { printf \"%.2f\", ($UTIME + $STIME) / ($CLOCKS * $ELAPSED) * 100 }")

  FD_COUNT=$(ls "/proc/$COSMO_PID/fd" 2>/dev/null | wc -l)

  # I/O from /proc/<pid>/io
  IO_DATA=$(cat "/proc/$COSMO_PID/io" 2>/dev/null || echo "read_bytes: 0 write_bytes: 0")
  READ_BYTES=$(echo "$IO_DATA" | awk '/^read_bytes:/ {print $2}')
  WRITE_BYTES=$(echo "$IO_DATA" | awk '/^write_bytes:/ {print $2}')

  echo "$(date -Iseconds),$COSMO_PID,$ELAPSED,$CPU_PCT,$RSS,$HWM,$VMSIZE,$RSSANON,$RSSFILE,$PSS,$VMSWAP,$THREADS,$FD_COUNT,$MINFLT,$MAJFLT,$VCTX,$NVCTX,$READ_BYTES,$WRITE_BYTES" >> "$CSV_PATH"

  sleep 60
done
```

4. After the run, analyze with the summary script:

```bash
bash scripts/endurance-summary.sh "$CSV_PATH"
```

If logs are stored in a sibling `logs/` directory, this copy-paste command is
safe to run even when no current files exist:

```bash
bash scripts/endurance-summary.sh '../logs/cosmostrix-resource-*.csv' || true
```

### Quick 1-hour smoke test

For faster iteration, use a 1-hour run with 10-second sampling:

```bash
./target/release/cosmostrix --duration 3600 &
COSMO_PID=$!

SAMPLE_INTERVAL=10
CSV_PATH="endurance-1h.csv"
echo "timestamp,pid,elapsed_sec,cpu_pct,rss_kb,hwm_kb,vmsize_kb,rssanon_kb,rssfile_kb,pss_kb,swap_kb,threads,fd_count,minflt,majflt,voluntary_ctxt,nonvoluntary_ctxt,read_bytes,write_bytes" > "$CSV_PATH"

START_TS=$(date +%s)

while kill -0 "$COSMO_PID" 2>/dev/null; do
  NOW_TS=$(date +%s)
  ELAPSED=$(( NOW_TS - START_TS ))
  RSS=$(awk '/^VmRSS:/ {print $2}' "/proc/$COSMO_PID/status")
  FD_COUNT=$(ls "/proc/$COSMO_PID/fd" 2>/dev/null | wc -l)
  THREADS=$(awk '/^Threads:/ {print $2}' "/proc/$COSMO_PID/status")
  VMSWAP=$(awk '/^VmSwap:/ {print $2}' "/proc/$COSMO_PID/status")

  echo "$(date -Iseconds),$COSMO_PID,$ELAPSED,0.00,$RSS,$RSS,0,0,$RSS,0,$VMSWAP,$THREADS,$FD_COUNT,0,0,0,0,0,0" >> "$CSV_PATH"
  sleep "$SAMPLE_INTERVAL"
done
```

## Acceptance criteria

| Criterion | Threshold | Rationale |
|---|---|---|
| RSS growth | < 2% per hour | Permits minor heap fragmentation; rejects leaks |
| FD count | Monotonically stable or decreasing | Detects file descriptor / handle leaks |
| Swap | Zero throughout run | Non-zero swap indicates memory pressure |
| Crash / panic | None | Renderer must exit cleanly on duration expiry |
| Clean exit | Exit code 0 | Confirms graceful shutdown path |
| Color drift | None when `auto-color-drift` is off | Fixed colors must remain sticky |

### Color stability endurance

Starting with v3.7.0, endurance runs should also verify color stability:
if the session was started with an explicit color (e.g., `--color sun`),
the color must remain unchanged for the entire duration. This is enforced
by deterministic in-process tests that simulate many minutes of frames and
assert the `ColorScheme` never changes. See `docs/VISUAL_STABILITY.md` for
the full color stability policy and test descriptions.

To manually verify during a long endurance run, check that the `--color`
value you passed at startup is still active at the end of the run. If
`auto-color-drift` is enabled (opt-in), color changes are expected and
acceptable.

Use `-i` (info) to confirm the drift state at any time:

```bash
cosmostrix -i | rg "auto_color_drift"
# auto_color_drift: false   <- default, no autonomous drift
# auto_color_drift: true    <- opt-in, ecosystem may change color
```

### Pass/fail logic

- **PASS**: All criteria met for the full duration. Verdict: `PASS / stable`.
- **FAIL**: Any single criterion violated (RSS exceeds hourly budget, fd_count
  increases by more than a transient spike, non-zero swap detected, unexpected
  exit, or non-zero exit code).

A transient FD spike (e.g. +/-2 handles) during a single sample is acceptable
as long as the count returns to baseline by the next sample.

## Summary script output

`scripts/endurance-summary.sh` parses the CSV resource log and prints a
summary table including:

- Elapsed time (seconds + human-readable duration)
- RSS: start / end / max / HWM / growth %
- PSS: start / end / max (extended format only)
- Swap: max (extended format only)
- File descriptors: start / end / max / leak verdict
- Threads: start / end / max (extended format only)
- CPU: avg / min / max / P95 (extended format only)
- Major page faults: delta / max (extended format only)
- I/O bytes: read / write delta (extended format only)

```bash
# Extended format (current)
bash scripts/endurance-summary.sh path/to/endurance.csv

# Legacy 4-column format (backward compatible)
bash scripts/endurance-summary.sh path/to/legacy-endurance.csv

# Multiple files at once
bash scripts/endurance-summary.sh endurance-1h.csv endurance-24h.csv
```

### Header validation

The script resolves columns by header name, not hardcoded positions. Required
fields are validated before processing:

- **Extended format**: `elapsed_sec`, `rss_kb`, `fd_count`
- **Legacy format**: `rss_kb`, `fd_count`, `elapsed_sec` or `elapsed_secs`

If required fields are missing, the script exits with a clear error message
identifying the missing column.

### No logs found

If a glob does not match any readable CSV files, the summary script prints a
friendly usage message instead of a raw `file not found` error. For example:

```bash
bash scripts/endurance-summary.sh '../logs/cosmostrix-resource-*.csv' || true
```

If that reports no matching logs:

- Confirm the logs were written to the path you passed.
- Prefer a durable sibling directory such as `../logs/`.
- Use a filename pattern like `cosmostrix-resource-YYYYMMDD-HHMM.csv`.
- Run a short smoke sample first, then summarize the exact CSV path.
- Quote glob patterns in zsh so the shell does not reject unmatched patterns
  before the summary script can print its friendly no-logs message.

Malformed CSV files and missing required columns are still treated as real
errors and should be fixed rather than ignored.

## Past results

Template for recording actual endurance run results. Copy this section and
fill in after each run.

### Run — [version] — [date] — [platform]

| Item | Value |
|---|---|
| Cosmostrix version | |
| Build profile | |
| Duration target | |
| Actual duration | |
| Sampling interval | |
| Terminal size | |
| Color mode | |
| OS / kernel | |
| CPU | |
| Exit code | |

**Results:**

| Metric | Value | Pass? |
|---|---|---|
| Start RSS | kB | — |
| End RSS | kB | — |
| Max RSS | kB | — |
| HWM | kB | — |
| RSS growth % | % | PASS / FAIL |
| PSS max | kB | — |
| Swap max | kB | PASS / FAIL |
| Start FD count | | — |
| End FD count | | — |
| FD leak detected | yes / no | PASS / FAIL |
| Threads | start / end / max | — |
| CPU avg | % | — |
| Major faults delta | | — |
| Crashes / panics | yes / no | PASS / FAIL |

**Notes:**

_(describe any anomalies, transient spikes, or environmental factors)_

---
