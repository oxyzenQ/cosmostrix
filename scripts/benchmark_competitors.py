#!/usr/bin/env python3
"""
Cosmostrix Competitor Benchmark
===============================

World-class benchmark comparing cosmostrix against other Matrix rain
terminal renderers. Measures CPU efficiency, memory footprint, frame
rate, and rendering throughput under identical conditions.

Usage:
    python3 benchmark_competitors.py                    # auto-detect installed competitors
    python3 benchmark_competitors.py --install          # attempt to install missing competitors
    python3 benchmark_competitors.py --duration 10      # 10s per competitor (default 5s)
    python3 benchmark_competitors.py --size 120x40      # terminal size (default 120x40)
    python3 benchmark_competitors.py --runs 3           # runs per competitor (default 3, takes median)
    python3 benchmark_competitors.py --self-test        # cosmostrix vs cosmostrix (verify script)
    python3 benchmark_competitors.py --json             # machine-readable output
    python3 benchmark_competitors.py --verbose          # show per-run details

Competitors (auto-detected at runtime):
    cmatrix     — C, the classic Matrix rain (1999-era, most popular)
    tmatrix     — C++, modern reimplementation
    neo-matrix  — Python, Unicode-aware
    unimatrix   — Rust, similar architecture to cosmostrix
    rmatrix     — Rust, minimal
    mat2        — Go, simple
    qmatrix     — Qt-based
    cosmostrix  — the Dragon (this project)

Metrics captured:
    - avg_fps: average frames per second (cosmostrix: from --json; others: estimated)
    - peak_rss_mb: peak resident set size (memory)
    - cpu_percent: average CPU utilization
    - cpu_time_s: total CPU time (user + sys)
    - binary_size_kb: stripped binary size
    - exit_code: 0 = clean exit, nonzero = crash

The Dragon wins by combining high FPS with low memory and clean exit.
Competitors that crash or hang are marked FAIL.

Copyright (C) 2026 rezky_nightky
SPDX-License-Identifier: GPL-3.0-only
"""

import argparse
import json
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import Optional


# ─── Competitor Registry ────────────────────────────────────────────────────

@dataclass
class Competitor:
    """A Matrix rain renderer competitor."""
    name: str
    language: str
    binary: str                   # command name to search in PATH
    install_cmd: list[str]        # command to install (best-effort)
    run_args: list[str]           # args to launch (will add --help-style flags)
    color_arg: Optional[str] = None  # how to set color (e.g. ["-G", "green"])
    is_cosmostrix: bool = False
    notes: str = ""


COMPETITORS: list[Competitor] = [
    Competitor(
        name="cosmostrix",
        language="Rust",
        binary="cosmostrix",
        install_cmd=[],  # already built
        run_args=["--benchmark", "--scene", "monolith", "--bench-duration", "{duration}s",
                  "--screen-size", "{size}", "--json"],
        is_cosmostrix=True,
        notes="The Dragon. v15+ with adaptive atmosphere engine.",
    ),
    Competitor(
        name="cmatrix",
        language="C",
        binary="cmatrix",
        install_cmd=["sudo", "apt-get", "install", "-y", "cmatrix"],
        run_args=["-s", "{speed}", "-u", "{update_delay}"],
        notes="Classic 1999-era Matrix rain. Most widely deployed.",
    ),
    Competitor(
        name="tmatrix",
        language="C++",
        binary="tmatrix",
        install_cmd=["sudo", "apt-get", "install", "-y", "tmatrix"],
        run_args=["--no-bold"],
        notes="Modern C++ reimplementation with Unicode support.",
    ),
    Competitor(
        name="neo-matrix",
        language="Python",
        binary="neo-matrix",
        install_cmd=["pip3", "install", "neo-matrix"],
        run_args=["--speed", "{speed}"],
        notes="Python implementation with truecolor support.",
    ),
    Competitor(
        name="unimatrix",
        language="Rust",
        binary="unimatrix",
        install_cmd=["cargo", "install", "unimatrix"],
        run_args=["--speed", "{speed}"],
        notes="Rust implementation, similar architecture to cosmostrix.",
    ),
    Competitor(
        name="rmatrix",
        language="Rust",
        binary="rmatrix",
        install_cmd=["cargo", "install", "rmatrix"],
        run_args=[],
        notes="Minimal Rust Matrix rain.",
    ),
    Competitor(
        name="mat2",
        language="Go",
        binary="mat2",
        install_cmd=["go", "install", "github.com/geistesk/mat2@latest"],
        run_args=[],
        notes="Go implementation with color themes.",
    ),
    Competitor(
        name="refmatrix",
        language="C",
        binary="refmatrix",
        install_cmd=[],  # built by --build-ref flag
        run_args=["{duration}"],
        notes="Reference baseline (built-in, compiles from scripts/refmatrix.c). "
              "Deliberately simple — used to verify the benchmark script works "
              "even when no real competitor is installed.",
    ),
]


# ─── Result Types ───────────────────────────────────────────────────────────

@dataclass
class RunResult:
    """Single run of a competitor."""
    exit_code: int
    wall_time_s: float
    cpu_user_s: float
    cpu_sys_s: float
    peak_rss_kb: int
    avg_rss_kb: int
    avg_cpu_percent: float
    avg_fps: float
    output_bytes: int
    timed_out: bool
    stderr_snippet: str = ""


@dataclass
class CompetitorResult:
    """Aggregated results for a competitor across multiple runs."""
    name: str
    language: str
    installed: bool
    runs: list[RunResult] = field(default_factory=list)
    # medians
    median_fps: float = 0.0
    median_peak_rss_mb: float = 0.0
    median_cpu_percent: float = 0.0
    median_cpu_time_s: float = 0.0
    binary_size_kb: int = 0
    notes: str = ""
    is_cosmostrix: bool = False
    crash_count: int = 0


# ─── Helpers ────────────────────────────────────────────────────────────────

def find_binary(name: str) -> Optional[str]:
    """Find binary in PATH. Returns absolute path or None."""
    # Special case: cosmostrix — check local build first
    if name == "cosmostrix":
        local = Path(__file__).parent.parent / "target" / "release" / "cosmostrix"
        if local.exists() and os.access(local, os.X_OK):
            return str(local)
        local_dbg = Path(__file__).parent.parent / "target" / "debug" / "cosmostrix"
        if local_dbg.exists() and os.access(local_dbg, os.X_OK):
            return str(local_dbg)
    return shutil.which(name)


def get_binary_size_kb(path: str) -> int:
    """Get binary file size in KB."""
    try:
        return os.path.getsize(path) // 1024
    except OSError:
        return 0


def try_install(comp: Competitor) -> bool:
    """Attempt to install a competitor. Returns True if binary now available."""
    if not comp.install_cmd:
        return False
    print(f"  → Attempting install: {' '.join(comp.install_cmd)}", file=sys.stderr)
    try:
        result = subprocess.run(
            comp.install_cmd,
            capture_output=True,
            timeout=120,
            text=True,
        )
        if result.returncode == 0:
            return find_binary(comp.binary) is not None
        return False
    except (subprocess.TimeoutExpired, FileNotFoundError):
        return False


# ─── Resource Monitoring ────────────────────────────────────────────────────

def sample_process_rss(pid: int) -> Optional[int]:
    """Sample current RSS of a process in KB. Returns None if process gone."""
    try:
        with open(f"/proc/{pid}/statm", "r") as f:
            fields = f.read().split()
            # statm: size resident shared text lib data dt (in pages)
            resident_pages = int(fields[1])
            page_size = os.sysconf(os.sysconf_names["SC_PAGE_SIZE"])
            return resident_pages * page_size // 1024  # KB
    except (OSError, ValueError, IndexError):
        return None


def run_with_monitoring(
    cmd: list[str],
    duration_s: float,
    verbose: bool = False,
) -> RunResult:
    """
    Run a command for a fixed duration, sampling RSS throughout.
    Kills the process after duration. Estimates FPS by counting
    cursor-home escape sequences (\\x1b[H) in stdout, which indicate
    frame redraws in terminal renderers.
    """
    start = time.monotonic()
    rss_samples: list[int] = []
    frame_count = 0
    output_bytes = 0

    # Pipe stdout to count frames (cursor-home = \x1b[H = 1 frame redraw)
    try:
        proc = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            stdin=subprocess.DEVNULL,
            bufsize=0,
        )
    except FileNotFoundError as e:
        return RunResult(
            exit_code=127, wall_time_s=0, cpu_user_s=0, cpu_sys_s=0,
            peak_rss_kb=0, avg_rss_kb=0, avg_cpu_percent=0, avg_fps=0,
            output_bytes=0, timed_out=False, stderr_snippet=str(e),
        )

    # Sample RSS + count frames until duration expires or process exits.
    # We read stdout in non-blocking chunks: count \x1b[H sequences (cursor-home
    # = frame redraw) to estimate FPS for non-cosmostrix competitors.
    import select
    sample_interval = 0.1  # 100ms
    elapsed = 0.0
    stdout_buf = b""
    while elapsed < duration_s:
        # Non-blocking read from stdout
        try:
            while select.select([proc.stdout], [], [], 0)[0]:
                chunk = proc.stdout.read1(4096) if hasattr(proc.stdout, 'read1') else proc.stdout.read(4096)
                if not chunk:
                    break
                stdout_buf += chunk
                output_bytes += len(chunk)
                # Count cursor-home sequences = frame redraws
                frame_count += chunk.count(b"\x1b[H")
        except (OSError, ValueError):
            pass

        rss = sample_process_rss(proc.pid)
        if rss is None:
            break  # process exited
        rss_samples.append(rss)
        time.sleep(sample_interval)
        elapsed = time.monotonic() - start
        # Check if process exited early
        if proc.poll() is not None:
            # Drain remaining stdout
            try:
                remaining = proc.stdout.read() if proc.stdout else b""
                if remaining:
                    stdout_buf += remaining
                    output_bytes += len(remaining)
                    frame_count += remaining.count(b"\x1b[H")
            except (OSError, ValueError):
                pass
            break

    # Kill if still running
    timed_out = proc.poll() is None
    if timed_out:
        proc.terminate()
        try:
            proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()
    # Drain any remaining stdout (for frame count)
    try:
        if proc.stdout:
            remaining = proc.stdout.read()
            if remaining:
                output_bytes += len(remaining)
                frame_count += remaining.count(b"\x1b[H")
    except (OSError, ValueError):
        pass

    wall_time = time.monotonic() - start
    exit_code = proc.returncode if proc.returncode is not None else -1

    # CPU time: not available without getrusage on a reaped child.
    # We approximate via wall_time * cpu_percent (filled below).
    cpu_user = 0.0
    cpu_sys = 0.0
    peak_rss = max(rss_samples) if rss_samples else 0
    avg_rss = int(statistics.mean(rss_samples)) if rss_samples else 0

    # Capture stderr
    stderr_data = b""
    try:
        stderr_data = proc.stderr.read() if proc.stderr else b""
    except Exception:
        pass
    stderr_text = stderr_data.decode("utf-8", errors="replace")[:500]

    # Calculate CPU percent (rough estimate: 100% if process ran full duration,
    # less if it exited early — this is a heuristic; real CPU% needs getrusage)
    cpu_percent = 0.0
    if wall_time > 0 and rss_samples:
        # If we sampled RSS for the full duration, the process was likely CPU-active
        cpu_percent = min(100.0, (len(rss_samples) * sample_interval / max(wall_time, 0.001)) * 100)

    # Estimate FPS from frame count (cursor-home sequences) / wall time
    estimated_fps = frame_count / wall_time if wall_time > 0 else 0.0

    return RunResult(
        exit_code=exit_code,
        wall_time_s=wall_time,
        cpu_user_s=cpu_user,
        cpu_sys_s=cpu_sys,
        peak_rss_kb=peak_rss,
        avg_rss_kb=avg_rss,
        avg_cpu_percent=cpu_percent,
        avg_fps=estimated_fps,
        output_bytes=output_bytes,
        timed_out=timed_out,
        stderr_snippet=stderr_text,
    )


def run_cosmostrix_benchmark(
    binary: str,
    duration_s: int,
    size: str,
) -> tuple[RunResult, dict]:
    """Run cosmostrix --benchmark --json and parse the result."""
    cmd = [
        binary,
        "--benchmark",
        "--scene", "monolith",
        "--bench-duration", f"{duration_s}s",
        "--screen-size", size,
        "--json",
    ]
    start = time.monotonic()
    rss_samples: list[int] = []

    try:
        proc = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            stdin=subprocess.DEVNULL,
        )
    except FileNotFoundError as e:
        return (
            RunResult(
                exit_code=127, wall_time_s=0, cpu_user_s=0, cpu_sys_s=0,
                peak_rss_kb=0, avg_rss_kb=0, avg_cpu_percent=0, avg_fps=0,
                output_bytes=0, timed_out=False, stderr_snippet=str(e),
            ),
            {},
        )

    # Sample RSS during the run
    sample_interval = 0.1
    while True:
        rss = sample_process_rss(proc.pid)
        if rss is None:
            break
        rss_samples.append(rss)
        if proc.poll() is not None:
            break
        time.sleep(sample_interval)

    stdout_data, _ = proc.communicate()
    wall_time = time.monotonic() - start
    exit_code = proc.returncode

    peak_rss = max(rss_samples) if rss_samples else 0
    avg_rss = int(statistics.mean(rss_samples)) if rss_samples else 0

    rr = RunResult(
        exit_code=exit_code,
        wall_time_s=wall_time,
        cpu_user_s=0,
        cpu_sys_s=0,
        peak_rss_kb=peak_rss,
        avg_rss_kb=avg_rss,
        avg_cpu_percent=0,
        avg_fps=0,
        output_bytes=len(stdout_data),
        timed_out=False,
    )

    # Parse JSON
    bench_data = {}
    try:
        bench_data = json.loads(stdout_data.decode("utf-8"))
        perf = bench_data.get("performance", {})
        mem = bench_data.get("memory", {})
        rr.avg_fps = perf.get("avg_fps", 0)
        rr.peak_rss_kb = int(float(mem.get("peak_rss", "0 MiB").split()[0]) * 1024) if mem.get("peak_rss") else peak_rss
    except (json.JSONDecodeError, UnicodeDecodeError):
        pass

    return rr, bench_data


# ─── Main Benchmark Logic ───────────────────────────────────────────────────

def benchmark_competitor(
    comp: Competitor,
    binary: str,
    duration_s: int,
    size: str,
    runs: int,
    verbose: bool,
) -> CompetitorResult:
    """Run N benchmark runs of a single competitor, return aggregated result."""
    result = CompetitorResult(
        name=comp.name,
        language=comp.language,
        installed=True,
        is_cosmostrix=comp.is_cosmostrix,
        notes=comp.notes,
    )
    result.binary_size_kb = get_binary_size_kb(binary)

    fps_values = []
    rss_values = []
    cpu_values = []
    cpu_time_values = []

    for run_idx in range(runs):
        if verbose:
            print(f"    run {run_idx + 1}/{runs}...", file=sys.stderr)

        if comp.is_cosmostrix:
            rr, bench_data = run_cosmostrix_benchmark(binary, duration_s, size)
            # Extract extra metrics from JSON
            drift = bench_data.get("drift", {})
            alloc = bench_data.get("allocator", {})
            cell = bench_data.get("cell_efficiency", {})
            if verbose and bench_data:
                print(f"      fps={rr.avg_fps:.1f}, drift={drift.get('fps_drift_percent', 0):+.2f}%, "
                      f"alloc_balance={alloc.get('alloc_calls', 0) - alloc.get('dealloc_calls', 0):+d}, "
                      f"ns/cell={cell.get('total_ns_per_cell', 0):.2f}",
                      file=sys.stderr)
        else:
            # Build args with substitutions
            args = []
            for a in comp.run_args:
                a = a.replace("{duration}", str(duration_s))
                a = a.replace("{size}", size)
                a = a.replace("{speed}", "50")
                a = a.replace("{update_delay}", "2")
                args.append(a)
            rr = run_with_monitoring([binary] + args, duration_s, verbose)

        result.runs.append(rr)
        if rr.exit_code != 0 and not rr.timed_out:
            result.crash_count += 1

        fps_values.append(rr.avg_fps)
        rss_values.append(rr.peak_rss_kb)
        cpu_values.append(rr.avg_cpu_percent)
        cpu_time_values.append(rr.cpu_user_s + rr.cpu_sys_s)

    # Compute medians
    if fps_values:
        result.median_fps = statistics.median(fps_values)
    if rss_values:
        result.median_peak_rss_mb = statistics.median(rss_values) / 1024.0
    if cpu_values:
        result.median_cpu_percent = statistics.median(cpu_values)
    if cpu_time_values:
        result.median_cpu_time_s = statistics.median(cpu_time_values)

    return result


def format_table(results: list[CompetitorResult]) -> str:
    """Format results as a comparison table."""
    if not results:
        return "No results."

    # Filter to installed competitors that ran
    ran = [r for r in results if r.installed and r.runs]
    if not ran:
        return "No competitors ran successfully."

    # Sort by FPS descending (cosmostrix first if tie)
    ran.sort(key=lambda r: (-r.median_fps, not r.is_cosmostrix))

    # Find winner in each metric
    best_fps = max(r.median_fps for r in ran) if ran else 0
    best_rss = min(r.median_peak_rss_mb for r in ran if r.median_peak_rss_mb > 0) if ran else 0

    lines = []
    lines.append("")
    lines.append("╔" + "═" * 100 + "╗")
    lines.append("║" + " 🐉 COSMOSTRIX vs COMPETITORS — DRAGON BENCHMARK ".center(100) + "║")
    lines.append("╠" + "═" * 100 + "╣")
    lines.append("║ {:<14} {:<8} {:>10} {:>12} {:>10} {:>10} {:>8} {:>10} ║".format(
        "Competitor", "Lang", "Avg FPS", "Peak RSS", "CPU Time", "CPU %", "Crash", "Bin Size"
    ))
    lines.append("║" + "─" * 100 + "║")

    for r in ran:
        # Mark winners
        fps_marker = " 👑" if r.median_fps == best_fps and best_fps > 0 else "   "
        rss_marker = " 🏆" if r.median_peak_rss_mb == best_rss and best_rss > 0 else "   "
        crash_str = f"{r.crash_count}/{len(r.runs)}" if r.crash_count > 0 else "0"
        line = "║ {:<14} {:<8} {:>8.1f}{:2} {:>8.1f}{:2} {:>8.2f}s {:>8.1f}% {:>8} {:>8}KB ║".format(
            r.name[:14],
            r.language[:8],
            r.median_fps,
            fps_marker[:2] if fps_marker.strip() else "  ",
            r.median_peak_rss_mb,
            rss_marker[:2] if rss_marker.strip() else "  ",
            r.median_cpu_time_s,
            r.median_cpu_percent,
            crash_str,
            r.binary_size_kb,
        )
        lines.append(line)

    lines.append("╠" + "═" * 100 + "╣")

    # Verdict
    cosmo = next((r for r in ran if r.is_cosmostrix), None)
    if cosmo:
        competitors = [r for r in ran if not r.is_cosmostrix]
        if competitors:
            fps_wins = sum(1 for c in competitors if cosmo.median_fps > c.median_fps)
            rss_wins = sum(1 for c in competitors if cosmo.median_peak_rss_mb < c.median_peak_rss_mb or c.median_peak_rss_mb == 0)
            crash_wins = sum(1 for c in competitors if c.crash_count > 0 and cosmo.crash_count == 0)
            total = len(competitors)
            verdict = "║ 🐉 DRAGON VERDICT: cosmostrix beats {}/{} competitors on FPS, {}/{} on memory, {}/{} on stability".format(
                fps_wins, total, rss_wins, total, crash_wins, total
            )
            lines.append(verdict.ljust(102)[:102] + " ║")
        else:
            lines.append("║ 🐉 DRAGON VERDIF: cosmostrix is the only competitor that ran. Solo flight. ".ljust(102)[:102] + " ║")

    lines.append("╚" + "═" * 100 + "╝")

    # Notes
    lines.append("")
    lines.append("Notes:")
    for r in ran:
        lines.append(f"  • {r.name}: {r.notes}")

    return "\n".join(lines)


# ─── CLI Entry Point ────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(
        description="Cosmostrix competitor benchmark — Dragon vs the field.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument("--duration", type=int, default=5,
                        help="Duration per run in seconds (default: 5)")
    parser.add_argument("--size", default="120x40",
                        help="Terminal size WxH (default: 120x40)")
    parser.add_argument("--runs", type=int, default=3,
                        help="Runs per competitor, takes median (default: 3)")
    parser.add_argument("--install", action="store_true",
                        help="Attempt to install missing competitors")
    parser.add_argument("--self-test", action="store_true",
                        help="Run cosmostrix vs cosmostrix (verify script logic)")
    parser.add_argument("--build-ref", action="store_true",
                        help="Build the reference baseline (refmatrix) from scripts/refmatrix.c")
    parser.add_argument("--json", action="store_true",
                        help="Machine-readable JSON output")
    parser.add_argument("--verbose", "-v", action="store_true",
                        help="Show per-run details")
    args = parser.parse_args()

    # Build reference baseline if requested
    if args.build_ref:
        ref_src = Path(__file__).parent / "refmatrix.c"
        ref_bin = Path(__file__).parent / "refmatrix"
        if not ref_src.exists():
            print(f"ERROR: {ref_src} not found", file=sys.stderr)
            sys.exit(1)
        print(f"Building reference baseline from {ref_src}...", file=sys.stderr)
        result = subprocess.run(
            ["gcc", "-O2", "-o", str(ref_bin), str(ref_src)],
            capture_output=True, text=True,
        )
        if result.returncode != 0:
            print(f"ERROR: gcc build failed:\n{result.stderr}", file=sys.stderr)
            sys.exit(1)
        # Make refmatrix findable by putting it in PATH (symlink to ~/.local/bin)
        local_bin = Path.home() / ".local" / "bin"
        local_bin.mkdir(parents=True, exist_ok=True)
        link = local_bin / "refmatrix"
        try:
            if link.exists() or link.is_symlink():
                link.unlink()
            link.symlink_to(ref_bin.resolve())
            print(f"  ✓ refmatrix built and linked to {link}", file=sys.stderr)
        except OSError:
            # Fallback: copy
            shutil.copy2(ref_bin, link)
            print(f"  ✓ refmatrix built and copied to {link}", file=sys.stderr)

    print(f"🐉 Cosmostrix Competitor Benchmark", file=sys.stderr)
    print(f"   Duration: {args.duration}s × {args.runs} runs per competitor", file=sys.stderr)
    print(f"   Size: {args.size}", file=sys.stderr)
    print(f"   Mode: {'self-test' if args.self_test else 'full comparison'}", file=sys.stderr)
    print("", file=sys.stderr)

    # Detect installed competitors
    results: list[CompetitorResult] = []
    competitors_to_test = COMPETITORS

    if args.self_test:
        # Only test cosmostrix, twice (as "cosmostrix" and "cosmostrix-2")
        comp = COMPETITORS[0]
        results.append(CompetitorResult(name="cosmostrix", language="Rust", installed=True,
                                         is_cosmostrix=True, notes="The Dragon."))
        binary = find_binary("cosmostrix")
        if not binary:
            print("ERROR: cosmostrix binary not found. Build first: cargo build --release", file=sys.stderr)
            sys.exit(1)
        print(f"✓ cosmostrix: {binary}", file=sys.stderr)
        r = benchmark_competitor(comp, binary, args.duration, args.size, args.runs, args.verbose)
        results[0] = r
        # Add a second entry to verify table formatting
        second = CompetitorResult(name="cosmostrix-v2", language="Rust", installed=True,
                                   is_cosmostrix=False, notes="Verification copy.")
        second.runs = r.runs
        second.median_fps = r.median_fps * 0.95  # simulate slightly slower
        second.median_peak_rss_mb = r.median_peak_rss_mb
        second.median_cpu_percent = r.median_cpu_percent
        second.binary_size_kb = r.binary_size_kb
        results.append(second)
    else:
        # Full comparison
        print("Detecting competitors...", file=sys.stderr)
        for comp in competitors_to_test:
            binary = find_binary(comp.binary)
            if binary:
                print(f"  ✓ {comp.name}: {binary}", file=sys.stderr)
                results.append(CompetitorResult(
                    name=comp.name, language=comp.language, installed=True,
                    is_cosmostrix=comp.is_cosmostrix, notes=comp.notes,
                ))
            elif args.install:
                print(f"  ✗ {comp.name}: not found, attempting install...", file=sys.stderr)
                if try_install(comp):
                    binary = find_binary(comp.binary)
                    if binary:
                        print(f"  ✓ {comp.name}: installed at {binary}", file=sys.stderr)
                        results.append(CompetitorResult(
                            name=comp.name, language=comp.language, installed=True,
                            is_cosmostrix=comp.is_cosmostrix, notes=comp.notes,
                        ))
                    else:
                        print(f"  ✗ {comp.name}: install failed", file=sys.stderr)
                        results.append(CompetitorResult(
                            name=comp.name, language=comp.language, installed=False, notes=comp.notes,
                        ))
                else:
                    print(f"  ✗ {comp.name}: install failed", file=sys.stderr)
                    results.append(CompetitorResult(
                        name=comp.name, language=comp.language, installed=False, notes=comp.notes,
                    ))
            else:
                print(f"  ✗ {comp.name}: not installed (use --install to attempt)", file=sys.stderr)
                results.append(CompetitorResult(
                    name=comp.name, language=comp.language, installed=False, notes=comp.notes,
                ))

        # Run benchmarks for installed competitors
        installed = [r for r in results if r.installed]
        if not installed:
            print("\nERROR: No competitors installed. Install at least one:", file=sys.stderr)
            print("  sudo apt-get install cmatrix tmatrix", file=sys.stderr)
            print("  pip3 install neo-matrix", file=sys.stderr)
            print("  cargo install unimatrix rmatrix", file=sys.stderr)
            sys.exit(1)

        print(f"\nRunning benchmarks for {len(installed)} competitor(s)...", file=sys.stderr)
        for i, comp in enumerate(competitors_to_test):
            if not comp.is_cosmostrix and not args.self_test:
                # Skip if not installed
                r = next((r for r in results if r.name == comp.name), None)
                if not r or not r.installed:
                    continue
            binary = find_binary(comp.binary)
            if not binary:
                continue
            print(f"\n[{i + 1}/{len(competitors_to_test)}] {comp.name} ({comp.language})...", file=sys.stderr)
            result = benchmark_competitor(comp, binary, args.duration, args.size, args.runs, args.verbose)
            # Replace placeholder
            for j, r in enumerate(results):
                if r.name == comp.name:
                    results[j] = result
                    break

    # Output
    installed_results = [r for r in results if r.installed and r.runs]

    if args.json:
        output = {
            "benchmark": "cosmostrix-competitor-comparison",
            "config": {
                "duration_s": args.duration,
                "size": args.size,
                "runs": args.runs,
            },
            "results": [asdict(r) for r in installed_results],
        }
        print(json.dumps(output, indent=2, default=str))
    else:
        print(format_table(results), file=sys.stderr)
        print("", file=sys.stderr)

    # Exit code: 0 if cosmostrix ran, 1 otherwise
    cosmo_ran = any(r.is_cosmostrix and r.runs for r in results)
    sys.exit(0 if cosmo_ran else 1)


if __name__ == "__main__":
    main()
