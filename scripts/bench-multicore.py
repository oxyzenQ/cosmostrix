#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 rezky_nightky
#
# dragon-multicore experiment: A/B benchmark single-core vs multi-core
# droplet simulation at three column counts (80, 200, 400).
#
# This script is part of the experimental dragon-multicore branch.
# It is NOT shipped in releases and is NOT referenced from docs — it
# exists purely to measure whether the parallel simulation pass pays
# for its synchronization overhead.
#
# Usage:
#   # Pre-build both binaries first:
#   cargo build --release                                   # serial
#   cp target/release/cosmostrix /tmp/cosmostrix-serial
#   cargo build --release --features multicore              # multi-core
#   cp target/release/cosmostrix /tmp/cosmostrix-multicore
#   python3 scripts/bench-multicore.py
#
# Output: a Markdown table comparing avg_fps at each size, plus a
# verdict line. The table is written to stdout and appended to
# benchmark/multicore_results.md.

import json
import os
import subprocess
import sys
import time
from pathlib import Path

SERIAL_BIN = Path("/tmp/cosmostrix-serial")
MULTICORE_BIN = Path("/tmp/cosmostrix-multicore")
SIZES = [(80, 24), (200, 60), (400, 200)]
BENCH_DURATION = 3  # seconds per run
WARMUP_DURATION = 1  # we discard the first run of each (cold cache)
RUNS_PER_CONFIG = 3  # take the median of 3 to dampen noise

OUTPUT_MD = Path(__file__).resolve().parent.parent / "benchmark" / "multicore_results.md"


def run_bench(binary: Path, cols: int, lines: int, duration: int) -> dict:
    """Run one benchmark and return the parsed JSON dict."""
    cmd = [
        str(binary),
        "--benchmark",
        "--bench-duration", str(duration),
        "--json",
        "--screen-size", f"{cols}x{lines}",
    ]
    proc = subprocess.run(
        cmd, capture_output=True, text=True, check=True, timeout=duration + 30
    )
    # The JSON is the last line of stdout (progress dots come first).
    last_line = proc.stdout.strip().splitlines()[-1]
    return json.loads(last_line)


def median(values: list[float]) -> float:
    s = sorted(values)
    n = len(s)
    if n == 0:
        return 0.0
    if n % 2 == 1:
        return s[n // 2]
    return (s[n // 2 - 1] + s[n // 2]) / 2.0


def bench_config(binary: Path, cols: int, lines: int) -> dict:
    """Run RUNS_PER_CONFIG iterations and return median result."""
    # Warmup: one extra run, discarded.
    run_bench(binary, cols, lines, WARMUP_DURATION)
    fps_samples = []
    sim_ms_samples = []
    render_ms_samples = []
    for _ in range(RUNS_PER_CONFIG):
        result = run_bench(binary, cols, lines, BENCH_DURATION)
        fps_samples.append(result["performance"]["avg_fps"])
        ct = result.get("component_timing", {})
        sim_ms_samples.append(ct.get("avg_sim_ms", 0))
        render_ms_samples.append(ct.get("avg_render_ms", 0))
    return {
        "avg_fps": median(fps_samples),
        "avg_sim_ms": median(sim_ms_samples),
        "avg_render_ms": median(render_ms_samples),
        "fps_samples": fps_samples,
    }


def fmt_fps(x: float) -> str:
    if x >= 100_000:
        return f"{x/1000:.1f}K"
    return f"{x:.1f}"


def main() -> int:
    if not SERIAL_BIN.exists():
        print(f"error: {SERIAL_BIN} not found. Build with: cargo build --release",
              file=sys.stderr)
        return 1
    if not MULTICORE_BIN.exists():
        print(f"error: {MULTICORE_BIN} not found. Build with: "
              f"cargo build --release --features multicore", file=sys.stderr)
        return 1

    print(f"dragon-multicore A/B benchmark", file=sys.stderr)
    print(f"  serial binary:     {SERIAL_BIN}", file=sys.stderr)
    print(f"  multicore binary:  {MULTICORE_BIN}", file=sys.stderr)
    print(f"  sizes:             {SIZES}", file=sys.stderr)
    print(f"  duration per run:  {BENCH_DURATION}s (+{WARMUP_DURATION}s warmup)",
          file=sys.stderr)
    print(f"  runs per config:   {RUNS_PER_CONFIG} (median)", file=sys.stderr)
    print(file=sys.stderr)

    rows = []
    for cols, lines in SIZES:
        print(f"  → {cols}x{lines} ...", file=sys.stderr, end="", flush=True)
        serial = bench_config(SERIAL_BIN, cols, lines)
        multi = bench_config(MULTICORE_BIN, cols, lines)
        speedup = multi["avg_fps"] / serial["avg_fps"] if serial["avg_fps"] > 0 else 0.0
        rows.append({
            "size": f"{cols}x{lines}",
            "cols": cols,
            "serial_fps": serial["avg_fps"],
            "multicore_fps": multi["avg_fps"],
            "speedup": speedup,
            "serial_sim_ms": serial["avg_sim_ms"],
            "multicore_sim_ms": multi["avg_sim_ms"],
            "serial_render_ms": serial["avg_render_ms"],
            "multicore_render_ms": multi["avg_render_ms"],
        })
        print(f" serial {fmt_fps(serial['avg_fps'])} fps → "
              f"multicore {fmt_fps(multi['avg_fps'])} fps "
              f"({speedup:.3f}x)", file=sys.stderr)

    # Build Markdown table.
    lines_out = []
    lines_out.append("# dragon-multicore A/B Benchmark Results")
    lines_out.append("")
    lines_out.append("<!-- SPDX-License-Identifier: GPL-3.0-only -->")
    lines_out.append("")
    lines_out.append("> Experimental branch. Numbers are machine-dependent and")
    lines_out.append("> only meaningful as a relative comparison between the two")
    lines_out.append("> binaries on the same host.")
    lines_out.append("")
    lines_out.append(f"**Host**: `{os.uname().sysname} {os.uname().machine}`")
    lines_out.append(f"**Cores**: `{os.cpu_count()}`")
    lines_out.append(f"**Bench duration**: {BENCH_DURATION}s per run, "
                     f"{RUNS_PER_CONFIG} runs (median), +{WARMUP_DURATION}s warmup")
    lines_out.append("")
    lines_out.append("| Size | Cols | Serial FPS | Multi-core FPS | Speedup | "
                     "Serial sim ms | Multi sim ms | Serial render ms | "
                     "Multi render ms |")
    lines_out.append("|------|------|------------|----------------|---------|"
                     "---------------|--------------|------------------|"
                     "------------------|")
    for r in rows:
        lines_out.append(
            f"| {r['size']} | {r['cols']} | {fmt_fps(r['serial_fps'])} | "
            f"{fmt_fps(r['multicore_fps'])} | {r['speedup']:.3f}x | "
            f"{r['serial_sim_ms']:.4f} | {r['multicore_sim_ms']:.4f} | "
            f"{r['serial_render_ms']:.4f} | {r['multicore_render_ms']:.4f} |"
        )
    lines_out.append("")

    # Verdict
    avg_speedup = sum(r["speedup"] for r in rows) / len(rows)
    if avg_speedup > 1.05:
        verdict = f"WIN — average speedup {avg_speedup:.3f}x. The parallel sim pass pays for itself."
    elif avg_speedup < 0.95:
        verdict = (f"LOSS — average speedup {avg_speedup:.3f}x. Synchronization overhead "
                   f"exceeds the parallel sim gain. The Dragon stays single-core.")
    else:
        verdict = (f"NEUTRAL — average speedup {avg_speedup:.3f}x. Within noise. "
                   f"More droplets or larger terminals may shift the balance.")
    lines_out.append(f"**Verdict**: {verdict}")
    lines_out.append("")

    OUTPUT_MD.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT_MD.write_text("\n".join(lines_out))
    print(f"\nWrote: {OUTPUT_MD}", file=sys.stderr)
    print(f"\n{verdict}", file=sys.stderr)

    # Also dump the table to stdout for shell capture.
    print("\n".join(lines_out))
    return 0


if __name__ == "__main__":
    sys.exit(main())
