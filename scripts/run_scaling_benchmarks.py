#!/usr/bin/env python3
"""
Run scaling benchmarks at 8 screen sizes and emit a JSON + Markdown summary.

Sizes: 6x6, 20x20, 40x20, 80x24, 120x40, 200x60, 320x100, 400x200.
For each size, captures:
  - avg_fps
  - total_ns_per_cell  (the key O(1) scaling metric)
  - render_ns_per_cell
  - io_ns_per_cell
  - io_share_percent   (computed: io_ms / (sim+render+io) * 100)
  - alloc_calls_per_frame
  - peak_rss (MiB, parsed from "X.X MiB" string)
  - dirty_cells_per_frame
  - logical_cells_per_frame
  - dirty_ratio_percent (dirty / logical * 100)
  - avg_cpu_percent
  - total_frames

Output:
  /home/z/my-project/scripts/scaling_results.json  — raw per-size JSON
  /home/z/my-project/scripts/scaling_results.md    — markdown table
"""
import json
import re
import subprocess
import sys
import time
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
BINARY = Path("/home/z/my-project/cosmostrix/target/release/cosmostrix")
SIZES = [
    (6, 6),
    (20, 20),
    (40, 20),
    (80, 24),
    (120, 40),
    (200, 60),
    (320, 100),
    (400, 200),
]
BENCH_DURATION = 2  # seconds per size


def parse_rss_mib(s: str) -> float:
    """Parse '3.7 MiB' or '9.2 MiB' into a float MiB."""
    m = re.match(r"([\d.]+)\s*MiB", s)
    if m:
        return float(m.group(1))
    m = re.match(r"([\d.]+)\s*KiB", s)
    if m:
        return float(m.group(1)) / 1024.0
    return 0.0


def run_one(cols: int, lines: int) -> dict:
    """Run a single benchmark and parse JSON output."""
    size_str = f"{cols}x{lines}"
    print(f"  benchmarking {size_str} ({cols*lines} cells)...", file=sys.stderr, flush=True)
    t0 = time.time()
    result = subprocess.run(
        [
            str(BINARY),
            "--benchmark",
            "--json",
            "--screen-size", size_str,
            "--bench-duration", str(BENCH_DURATION),
        ],
        capture_output=True,
        text=True,
        timeout=BENCH_DURATION * 5 + 10,
    )
    elapsed = time.time() - t0
    if result.returncode != 0:
        print(f"    FAILED (exit {result.returncode})", file=sys.stderr)
        print(f"    stderr: {result.stderr[:500]}", file=sys.stderr)
        return {"size": size_str, "cols": cols, "lines": lines, "error": result.stderr[:500]}
    try:
        data = json.loads(result.stdout)
    except json.JSONDecodeError as e:
        print(f"    JSON parse error: {e}", file=sys.stderr)
        return {"size": size_str, "cols": cols, "lines": lines, "error": f"JSON: {e}"}

    perf = data.get("performance", {})
    cell = data.get("cell_efficiency", {})
    comp = data.get("component_timing", {})
    mem = data.get("memory", {})
    alloc = data.get("allocator", {})
    cpu = data.get("cpu", {})
    timing = data.get("timing", {})

    sim_ms = comp.get("avg_sim_ms", 0)
    render_ms = comp.get("avg_render_ms", 0)
    io_ms = comp.get("avg_io_ms", 0)
    total_ms = sim_ms + render_ms + io_ms
    io_share = (io_ms / total_ms * 100) if total_ms > 0 else 0.0

    logical = cell.get("logical_cells_per_frame", 0)
    dirty = cell.get("dirty_cells_per_frame", 0)
    dirty_ratio = (dirty / logical * 100) if logical > 0 else 0.0

    peak_rss_mib = parse_rss_mib(mem.get("peak_rss", "0 MiB"))

    record = {
        "size": size_str,
        "cols": cols,
        "lines": lines,
        "cell_count": cols * lines,
        "avg_fps": perf.get("avg_fps", 0),
        "peak_fps": perf.get("peak_fps", 0),
        "avg_frame_time_ms": perf.get("avg_frame_time_ms", 0),
        "p99_frame_time_ms": perf.get("p99_frame_time_ms", 0),
        "total_ns_per_cell": cell.get("total_ns_per_cell", 0),
        "render_ns_per_cell": cell.get("render_ns_per_cell", 0),
        "io_ns_per_cell": cell.get("io_ns_per_cell", 0),
        "io_share_percent": io_share,
        "alloc_calls_per_frame": alloc.get("alloc_calls_per_frame", 0),
        "dealloc_calls_per_frame": alloc.get("dealloc_calls_per_frame", 0),
        "peak_rss_mib": peak_rss_mib,
        "avg_rss_mib": parse_rss_mib(mem.get("avg_rss", "0 MiB")),
        "logical_cells_per_frame": logical,
        "dirty_cells_per_frame": dirty,
        "dirty_ratio_percent": dirty_ratio,
        "avg_cpu_percent": cpu.get("avg_cpu_percent", 0),
        "total_frames": timing.get("total_frames", 0),
        "elapsed_s": timing.get("elapsed_s", 0),
        "wall_time_s": round(elapsed, 3),
    }
    print(
        f"    ok: {record['avg_fps']:.0f} fps, "
        f"{record['total_ns_per_cell']:.1f} ns/cell, "
        f"{record['alloc_calls_per_frame']:.2f} allocs/frame, "
        f"{peak_rss_mib:.1f} MiB",
        file=sys.stderr,
        flush=True,
    )
    return record


def main() -> int:
    if not BINARY.exists():
        print(f"error: release binary not found at {BINARY}", file=sys.stderr)
        print("run: cargo build --release", file=sys.stderr)
        return 1

    print(f"Running scaling benchmark across {len(SIZES)} sizes "
          f"({BENCH_DURATION}s each)...", file=sys.stderr)

    results = []
    for cols, lines in SIZES:
        record = run_one(cols, lines)
        results.append(record)

    # Save raw JSON
    raw_path = SCRIPT_DIR / "scaling_results.json"
    with raw_path.open("w") as f:
        json.dump({
            "binary": str(BINARY),
            "bench_duration_s": BENCH_DURATION,
            "sizes": SIZES,
            "results": results,
        }, f, indent=2)
    print(f"\nRaw JSON: {raw_path}", file=sys.stderr)

    # Emit Markdown table
    md_path = SCRIPT_DIR / "scaling_results.md"
    with md_path.open("w") as f:
        f.write("# Scaling Benchmark Results (raw)\n\n")
        f.write(f"Binary: `{BINARY}`  \n")
        f.write(f"Bench duration: {BENCH_DURATION}s per size  \n")
        f.write(f"Total sizes: {len(SIZES)}\n\n")
        f.write("| Size | Cells | avg_fps | total_ns/cell | render_ns/cell | "
                "io_ns/cell | io_share% | allocs/frame | peak_rss(MiB) | "
                "dirty_ratio% |\n")
        f.write("|------|-------|---------|---------------|----------------|"
                "------------|-----------|--------------|---------------|"
                "--------------|\n")
        for r in results:
            if "error" in r:
                f.write(f"| {r['size']} | {r['cell_count']} | ERROR | - | - | - | - | - | - | - |\n")
                continue
            f.write(
                f"| {r['size']} | {r['cell_count']} | "
                f"{r['avg_fps']:.0f} | "
                f"{r['total_ns_per_cell']:.1f} | "
                f"{r['render_ns_per_cell']:.1f} | "
                f"{r['io_ns_per_cell']:.1f} | "
                f"{r['io_share_percent']:.1f} | "
                f"{r['alloc_calls_per_frame']:.2f} | "
                f"{r['peak_rss_mib']:.1f} | "
                f"{r['dirty_ratio_percent']:.1f} |\n"
            )
    print(f"Markdown: {md_path}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
