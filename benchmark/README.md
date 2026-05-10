# Benchmark

This folder contains performance and profiling artifacts for Cosmostrix.

## Summary (current results)

- Peak heap (Valgrind Massif): `~162 KB` (`165,240 bytes` total)
  - `mem_heap_B=162471`
  - `mem_heap_extra_B=2769`
- Heap behavior: stable (no obvious growth during runtime)
- **Performance comparison (hyperfine, 215,318 frames @ 60 fps):**
  - `release`: 30.6s (~6,948 fps)
  - `pro-native`: 26.5s (~7,969 fps) — **1.16x faster**

Practical takeaway:

- Memory usage is low and stable for a terminal visualizer (~162 KB peak).
- The `pro-native` profile provides measurable performance gains over standard `release`.

Massif detail (from `massif.out` peak snapshot):

- `mem_heap_B=162471`
- `mem_heap_extra_B=2769`
- Total: `162471 + 2769 = 165240 bytes (~162 KB)`

Artifacts currently tracked here:

- `massif.out`: Massif output (heap snapshots)
- `flamegraph.svg`: CPU flamegraph (SVG)

Large/temporary artifacts:

- `perf.data`: raw perf recording (large)

Recommendation: don’t commit `perf.data` long-term (it is big and highly machine-specific). This folder has a local `.gitignore` to ignore `perf.data*` for future recordings.

If `perf.data` is already tracked in git, ignoring it is not enough. Untrack it with:

```bash
git rm --cached benchmark/perf.data
git commit -m "chore(bench): stop tracking perf.data"
```

## Reproducible 30s benchmark

Recommended approach for consistent benchmarking is to run Cosmostrix with a fixed duration so it can exit cleanly:

```bash
cargo build
cargo build --release

./target/release/cosmostrix --duration 30
```

If you want to compare debug vs release manually:

```bash
./target/debug/cosmostrix --duration 30
./target/release/cosmostrix --duration 30
```

Then run the benchmark script:

```bash
bash benchmark/benchmark.sh
```

The script will try to generate:

- `benchmark/hyperfine.md` — comparison table (release vs pro-native)
- `benchmark/time-release.txt`, `benchmark/time-pro-native.txt` — /usr/bin/time -v output
- `benchmark/perf-release.txt`, `benchmark/perf-pro-native.txt` — perf stat output
- `benchmark/massif-release-{frames}f.out`, `benchmark/massif-pro-native-{frames}f.out` — Valgrind heap profiles

(If a tool is missing, the related step is skipped.)
