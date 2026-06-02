# Endurance Testing

## Purpose

Cosmostrix is designed to run as a long-lived terminal screensaver. This document
describes the endurance testing methodology used to verify that the renderer
remains stable for sessions exceeding 24 hours without memory growth, handle
leaks, or crashes.

## Test methodology

A Cosmostrix binary is launched in headless mode with a 24-hour (86 400 s)
duration cap. A lightweight sampling script reads `/proc/<pid>/status` and
`/proc/<pid>/fd` at regular intervals and appends a single CSV row per sample.

### Resource log format (CSV)

Each row contains four fields:

```
timestamp,rss_kb,fd_count,elapsed_secs
```

| Field | Description |
|---|---|
| `timestamp` | Unix epoch (seconds since 1970-01-01 UTC) |
| `rss_kb` | Resident Set Size in kB from `VmRSS` in `/proc/<pid>/status` |
| `fd_count` | Number of open file descriptors from `ls /proc/<pid>/fd \| wc -l` |
| `elapsed_secs` | Seconds since the first sample |

### Sampling interval

The default interval is 60 seconds. For shorter test runs (e.g. 1-hour
smoke tests) a 10-second interval provides higher resolution.

### How to run

1. Build a release binary:

```bash
cargo build --release
```

2. Launch Cosmostrix in the background with a 24-hour cap:

```bash
./target/release/cosmostrix --duration 86400 &
COSMO_PID=$!
```

3. Start sampling (run from a separate terminal or via nohup):

```bash
# 60-second interval (default)
while kill -0 "$COSMO_PID" 2>/dev/null; do
  ts=$(date +%s)
  rss=$(awk '/^VmRSS:/ {print $2}' "/proc/$COSMO_PID/status")
  fds=$(ls "/proc/$COSMO_PID/fd" 2>/dev/null | wc -l)
  start_ts=$(head -1 "$CSV_PATH" | cut -d, -f1)
  elapsed=$(( ts - start_ts ))
  echo "${ts},${rss},${fds},${elapsed}" >> "$CSV_PATH"
  sleep 60
done
```

4. After the run, analyze with the summary script:

```bash
bash scripts/endurance-summary.sh "$CSV_PATH"
```

### Quick 1-hour smoke test

For faster iteration, use a 1-hour run with 10-second sampling:

```bash
./target/release/cosmostrix --duration 3600 &
COSMO_PID=$!

SAMPLE_INTERVAL=10
CSV_PATH="endurance-1h.csv"
echo "timestamp,rss_kb,fd_count,elapsed_secs" > "$CSV_PATH"

while kill -0 "$COSMO_PID" 2>/dev/null; do
  ts=$(date +%s)
  rss=$(awk '/^VmRSS:/ {print $2}' "/proc/$COSMO_PID/status")
  fds=$(ls "/proc/$COSMO_PID/fd" 2>/dev/null | wc -l)
  start_ts=$(head -1 "$CSV_PATH" | cut -d, -f1)
  elapsed=$(( ts - start_ts ))
  echo "${ts},${rss},${fds},${elapsed}" >> "$CSV_PATH"
  sleep "$SAMPLE_INTERVAL"
done
```

## Acceptance criteria

| Criterion | Threshold | Rationale |
|---|---|---|
| RSS growth | < 2% per hour | Permits minor heap fragmentation; rejects leaks |
| FD count | Monotonically stable or decreasing | Detects file descriptor / handle leaks |
| Crash / panic | None | Renderer must exit cleanly on duration expiry |
| Clean exit | Exit code 0 | Confirms graceful shutdown path |

### Pass/fail logic

- **PASS**: All four criteria met for the full duration.
- **FAIL**: Any single criterion violated (RSS exceeds hourly budget, fd_count
  increases by more than a transient spike, unexpected exit, or non-zero exit
  code).

A transient FD spike (e.g. ±2 handles) during a single sample is acceptable
as long as the count returns to baseline by the next sample.

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
| RSS growth % | % | PASS / FAIL |
| Start FD count | | — |
| End FD count | | — |
| FD leak detected | yes / no | PASS / FAIL |
| Crashes / panics | yes / no | PASS / FAIL |

**Notes:**

_(describe any anomalies, transient spikes, or environmental factors)_

---

## Companion script

`scripts/endurance-summary.sh` parses the CSV resource log and prints a
summary table with start/end RSS, max RSS, RSS growth percentage, FD counts,
leak detection, and elapsed time.

```bash
bash scripts/endurance-summary.sh path/to/endurance.csv
```
