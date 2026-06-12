<!-- SPDX-License-Identifier: MIT -->

# Release Guard

This document defines the mandatory pre-tag gates that must pass before
creating a release tag.  It exists because the v4.8.0 release almost
shipped without a benchmark report in `benchmark/README.md`.  The guard
prevents that class of mistake from recurring.

## Pre-Tag Gates

All gates must pass before signing and pushing a release tag.  Gates are
ordered; earlier gates should be completed first because later gates may
depend on their output.

### Gate 1 — Version metadata check

Verify the version is correct across all metadata files:

```bash
./version-to.sh --check <VERSION>
```

This validates `Cargo.toml`, `Cargo.lock`, `PKGBUILD`, and `.SRCINFO`
all agree on the target version.

### Gate 2 — Full validation

Run the complete check suite:

```bash
cargo fmt --all -- --check
cargo test --all --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
./scripts/rc-smoke.sh
```

All tests must pass.  Clippy must produce zero warnings.  RC smoke must
pass all checks.

### Gate 3 — Final release binary build

Build the release binary with the optimized profile:

```bash
cargo pro-linux-v3
```

Verify the binary reports the correct version and commit:

```bash
target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix -V
```

### Gate 4 — 5-run benchmark

Run 5 benchmark iterations and record the results.  The helper script
automates collection and Markdown generation:

```bash
# Generate a report section for review (does NOT edit files):
./scripts/release-benchmark-report.sh X.Y.Z

# Custom run count, skip build:
./scripts/release-benchmark-report.sh X.Y.Z --runs 5 --no-build

# Output goes to stdout — review, then paste into benchmark/README.md.
# The script validates invariants and fails if they are violated.
```

Manual process (if script is not used):

```bash
BIN="target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix"
for i in 1 2 3 4 5; do
  echo "RUN $i"
  "$BIN" --benchmark
  sleep 3
done
```

Record: avg_fps, median_fps, p95_frame_time, p99_frame_time,
frame_time_stability, avg_dirty_cell_ratio, active_streams_avg,
actual_execution, terminal_writer, compute_parallelism.

### Gate 5 — Update benchmark/README.md

Add a release benchmark section for the new version to
`benchmark/README.md`.  The section must include:

* Final commit hash
* Benchmark run count (5)
* Build profile
* Binary version output
* Per-run table
* Mean avg_fps
* Invariants table
* Notes about workload scope and honest FPS boundaries

This gate is **mandatory**.  Never tag before the benchmark report is
committed.

### Gate 6 — Docs guard tests

Ensure docs guard tests pass.  For the benchmark report specifically,
the following guards must be satisfied (see `src/docs_tests/release.rs`):

* Benchmark README mentions the current release version
* Benchmark README mentions "release benchmark"
* Benchmark README mentions the run count
* Benchmark README states `terminal_writer: single-owner`
* Benchmark README states `compute_parallelism: disabled`
* Benchmark README states 50k was not reached / not promised
* Benchmark README reports `actual_execution: single-threaded-renderer`

### Gate 7 — Terminal lifecycle verification

If terminal code has changed since the last release, verify the terminal
lifecycle matrix paths:

1. Run `--doctor` and confirm no errors.
2. Test normal `q` / `Esc` exit — no visible residue, prompt clean.
3. Test Ctrl-C (SIGINT) — no visible residue on main screen.
4. Test `pkill -TERM -f cosmostrix` — no visible residue (v4.8 Phase 4B
   cleanup).
5. Run `cosmostrix --reset-terminal` — confirms destructive recovery works.
6. Review `docs/TERMINAL_LIFECYCLE_MATRIX.md` for accuracy.
7. **Do not claim SIGKILL cleanup.** SIGKILL cannot be caught. The fork
   guard is best-effort and Linux-only. Document honestly.

If no terminal code changed since the last release, this gate is a
lightweight review (confirm matrix doc exists, no changes needed).

See `docs/TERMINAL_LIFECYCLE_MATRIX.md` for the full matrix of all
14 lifecycle paths and their expected behavior.

### Gate 8 — CI green

The CI workflow must pass on `main` at the commit that will be tagged.
Do not tag until CI is green.

### Gate 9 — Signed tag

Create a signed tag only after all above gates pass:

```bash
git tag -s vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
```

### Gate 10 — GitHub release

Create the GitHub release only after the tag workflow passes and CI
artifacts are available.  The release workflow (`.github/workflows/release.yml`)
handles this automatically when triggered by the tag.

### Gate 11 — AUR publish

Publish to AUR only after release assets and checksums exist on GitHub.
Verify the binary checksum matches before updating the AUR package.

## Honesty Rules

* **Never tag before benchmark report.**  The benchmark report in
  `benchmark/README.md` is a required pre-tag artifact.

* **Never claim benchmark results from a different workload as the
  default benchmark.**  Heavy message mode, matrix mode, and other
  non-default workloads will yield different FPS.  Only the default
  renderer workload (cosmic rain at 120x40) is the standard benchmark.

* **Heavy message/matrix mode is not comparable to the default
  benchmark.**  These modes have different computational costs and
  must not be used to inflate or deflate release benchmark numbers.

* **SIGKILL cleanup cannot be guaranteed.**  No process can intercept
  SIGKILL.  Terminal residue after SIGKILL is expected and documented
  in `docs/TERMINAL_KILL_CLEANUP.md`.

* **50k FPS is not a release promise.**  The 50k FPS lab target was
  not reached and is not promised for any release.  Do not reference
  50k FPS in release notes, CHANGELOG, or marketing material as a
  achieved or guaranteed target.

* **Renderer invariants are non-negotiable.**  Every release must
  honestly report:
  - `actual_execution: single-threaded-renderer`
  - `terminal_writer: single-owner`
  - `compute_parallelism: disabled`

* **Terminal lifecycle matrix is authoritative.**  The matrix in
  `docs/TERMINAL_LIFECYCLE_MATRIX.md` defines expected cleanup behavior
  for all 14 lifecycle paths.  Owner visual smoke testing is required
  before release if terminal code changes.

* **Owner visual smoke remains required before release** if terminal
  code changes.  Automated tests cannot verify visible terminal residue.
  The owner must manually test normal exit, Ctrl-C, and SIGTERM paths.

## Pattern for Future Releases

When preparing release N:

1. Complete all feature work and validation.
2. Run the benchmark report helper (Gate 4):
   ```bash
   ./scripts/release-benchmark-report.sh X.Y.Z > /tmp/bench-report.md
   ```
3. Review the generated Markdown, then add it to
   `benchmark/README.md` (Gate 5).
4. Add a docs guard test for the new version in
   `src/docs_tests/release.rs` following the existing pattern.
5. Ensure all gates pass.
6. Tag only after CI green.