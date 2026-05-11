# Benchmark

This folder contains the benchmark script and reference results for Cosmostrix.

## Reference results

- Peak heap (Valgrind Massif): ~162 KB (165,240 bytes total)
- Heap behavior: stable, no growth during runtime
- `pro-native` profile: ~1.16x faster than standard `release` (hyperfine, 215k frames @ 60 fps)

## Reproducible benchmark

Run Cosmostrix with a fixed duration for clean, consistent measurement:

```bash
cargo build --release
./target/release/cosmostrix --duration 30
```

Then use the benchmark script to generate a full comparison:

```bash
bash benchmark/benchmark.sh
```

The script generates (in this folder):

- `hyperfine.md` — release vs pro-native comparison table
- `time-*.txt` — `/usr/bin/time -v` output
- `perf-*.txt` — `perf stat` output
- `massif-*-*.out` — Valgrind heap profiles

All generated outputs are gitignored (see `.gitignore`).
