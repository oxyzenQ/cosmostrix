# Benchmark

This folder contains the benchmark script and reference results for Cosmostrix.

## Reference results

- RSS: stable ~1.8 MB (release) / ~2.0 MB (pro-native), no growth during runtime
- `pro-native` profile: ~1.08x faster than standard `release` (hyperfine, 165k frames @ 60 fps, 120x40)
- Throughput: ~12,380 fps (release) / ~13,322 fps (pro-native)

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
