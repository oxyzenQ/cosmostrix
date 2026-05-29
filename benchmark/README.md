# Benchmark

This folder contains the benchmark script and reference results for Cosmostrix.

## Reference results

Historical local measurements from the current benchmark script:

- Peak heap (Valgrind Massif): ~194 KB (198,663 bytes), stable — no growth during runtime
- RSS: stable ~1.8 MB (release) / ~2.0 MB (pro-native)
- `pro-native` profile: ~1.08x faster than standard `release` on the measured host (hyperfine, 165k frames @ 60 fps, 120x40)
- Throughput on that host: ~12,380 fps (release) / ~13,322 fps (pro-native)

Treat these as a baseline example, not a portable promise. Re-run the benchmark
on the target machine before claiming a performance change.

## Reproducible benchmark

Run the headless benchmark path for deterministic frame throughput:

```bash
COSMOSTRIX_BENCH_COLS=120 COSMOSTRIX_BENCH_LINES=40 \
  target/release/cosmostrix --fps 60 --bench-frames 10000
```

Then use the benchmark script to generate a full comparison:

```bash
bash benchmark/benchmark.sh
```

The script builds `release` and local-only `pro-native`, calibrates a repeatable
frame count, and records FPS, frame pacing, and memory/profiling data when the
optional tools are installed. CI intentionally does not gate on these numbers;
they are measurement aids, not stable pass/fail thresholds.

The script generates (in this folder):

- `hyperfine.md` — release vs pro-native comparison table
- `time-*.txt` — `/usr/bin/time -v` output
- `perf-*.txt` — `perf stat` output
- `massif-*-*.out` — Valgrind heap profiles

All generated outputs are gitignored (see `.gitignore`).
