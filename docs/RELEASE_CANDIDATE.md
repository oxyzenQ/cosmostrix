# Release Candidate Checklist
<!-- SPDX-License-Identifier: GPL-3.0-only -->

Cosmostrix follows [SemVer](https://semver.org/) for package versions. Git tags and
GitHub Releases use a leading `v` (e.g. `v4.0.0`). Stable releases do not use
`-stable.N` suffixes. Do not bump the version or create a tag until the release
phase is explicitly authorized. Phase 12.1 bumps version metadata to 4.0.0 but
does not create a tag.

## Required Commands

```bash
cargo fmt --all
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --all --locked
./scripts/build.sh check-all
cargo pro-linux-v3
./scripts/version-to.sh --check <version>
```

All must pass with zero errors before a release candidate is considered.

## Runtime Smoke

```bash
BIN="target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix"

"$BIN" -V
"$BIN" -i
"$BIN" --doctor
"$BIN" --benchmark
"$BIN" --benchmark --bench-duration 3
"$BIN" --color red --color-tune saturation=1.5 --benchmark
```

Expected defaults:

- `application_mode`: disabled
- `effective_runtime`: identity
- `shadow_metrics`: identity
- `shadow_risk`: identity
- `config_gate`: disabled
- `visual_runtime`: protected
- `runtime_application`: identity
- `actual_execution`: single-threaded-renderer

v11.1.0+ benchmark output must include the following section headers
(grep to verify):

```bash
"$BIN" --benchmark 2>&1 | grep -E "^(BENCHMARK ENVIRONMENT|MEMORY|CPU|COMPONENT TIMING|DRIFT|RESOURCE):"
```

All six headers must appear. On Linux/macOS, `MEMORY`, `CPU`, and
`RESOURCE` must report real numbers (not `unsupported`). The `RENDERER`
section must contain `gpu_usage: not_applicable`.

### JSON output smoke

```bash
"$BIN" --benchmark --json | python3 -c "import json,sys; json.load(sys.stdin); print('valid JSON')"
```

Must print `valid JSON`. The JSON object must contain 13 top-level keys:
status, system, renderer, config, environment, performance, memory, cpu,
resource, component_timing, drift, throughput, timing.

## v11.x Benchmark & HUD RC Checklist

Additional smoke checks for the v11.1.0 benchmark depth + theme tuning
release. All must pass before tagging v11.1.0.

### `--bench-duration` validation

```bash
# In-range must succeed:
"$BIN" --benchmark --bench-duration 3   # exits 0, DRIFT section present
"$BIN" --benchmark --bench-duration 600 # exits 0 (max boundary)

# Out-of-range must fail with a clear error:
"$BIN" --benchmark --bench-duration 0     # "below the 1-second minimum"
"$BIN" --benchmark --bench-duration 601   # "exceeds the 600-second maximum"
```

### `--color-tune` validation

```bash
# Valid syntax must launch (use --benchmark for headless verify):
"$BIN" --color green --color-tune "saturation=1.5,brightness=0.9" --benchmark
"$BIN" --color aurora --color-tune "sat=0.0" --benchmark        # grayscale
"$BIN" --color red --color-tune "bright=1.3" --benchmark        # brightness only

# Invalid must fail with a clear error:
"$BIN" --color-tune "hue=30"          # "unknown key 'hue'"
"$BIN" --color-tune "saturation=4.0"  # "out of range [0, 3]"
"$BIN" --color-tune ""                # "value is empty"
```

### Benchmark section presence

```bash
"$BIN" --benchmark 2>&1 | grep -c "^MEMORY:"
"$BIN" --benchmark 2>&1 | grep -c "^CPU:"
"$BIN" --benchmark 2>&1 | grep -c "^COMPONENT TIMING:"
"$BIN" --benchmark 2>&1 | grep -c "^DRIFT:"
"$BIN" --benchmark 2>&1 | grep -c "^RESOURCE:"
"$BIN" --benchmark 2>&1 | grep -c "^BENCHMARK ENVIRONMENT:"
"$BIN" --benchmark 2>&1 | grep -c "gpu_usage: not_applicable"
```

Each must print `1` (exactly one section header). On Linux/macOS,
`MEMORY`, `CPU`, and `RESOURCE` must report real numbers; on other
platforms they emit `unsupported` with a reason field.

### JSON output validation

```bash
# Must produce valid parseable JSON:
"$BIN" --benchmark --json | python3 -c "import json,sys; d=json.load(sys.stdin); assert 'performance' in d; assert d['renderer']['gpu_usage']=='not_applicable'; print('OK')"

# Must print OK. Verifies JSON parses, has performance section, and
# the GPU-not-used declaration is present.
```

### Live HUD overlay (manual interactive smoke)

```bash
"$BIN"
```

Then press `?` and verify:

- A top-right overlay appears showing `fps`, `avg`, `p99`, `max`, `rss`.
- The overlay updates ~4 times per second without flickering.
- Press `?` again — the overlay disappears cleanly.
- Press `q` — clean exit, terminal restored.

## v4.6 Atmosphere RC Checklist

Additional smoke checks for the controlled atmosphere expansion (v4.6.0):

```bash
"$BIN" --list-profiles
```

Expected:

- Output contains `CONTROLLED ATMOSPHERE PRESETS (opt-in only)`.
- All six presets listed: `atmosphere-calm`, `atmosphere-pulse`,
  `atmosphere-signal`, `atmosphere-compression`, `atmosphere-void`,
  `atmosphere-monolith-pressure`.
- `atmosphere-storm` does NOT appear.
- Default remains `disabled / protected / identity`.
- Storm remains unavailable at every layer.
- `--color sun` stickiness is documented and tested.
- Terminal writer remains `single-owner`.
- `compute_parallelism` remains `disabled`.

Note: The benchmark and README guard checks in `rc-smoke.sh` must still pass
before any version tag is created.

## v4.7 Profile RC Checklist

Additional smoke checks for the profile ecosystem (v4.7.0):

- `docs/PROFILE_ECOSYSTEM.md` exists with full profile contract,
  behavior matrix, and validation documentation.
- `docs/PROFILE_EXAMPLES.md` exists with 9 profile examples and
  validation notes.
- `--list-profiles` points to both `docs/PROFILE_ECOSYSTEM.md` and
  `docs/PROFILE_EXAMPLES.md`.
- `--dump-config` points to `docs/PROFILE_EXAMPLES.md` and atmosphere
  preset examples.
- Unknown profile produces a clean error listing available profiles.
- Storm remains unavailable at every layer with a clear message.
- `CLI > profile > config > defaults` precedence is documented and tested.
- Terminal writer remains `single-owner`.
- `compute_parallelism` remains `disabled`.
- `zactrix-20k-lab` branch is parked for v4.8.
- Benchmark and README rules in `rc-smoke.sh` must still pass before
  any version tag is created.

## Controlled-Live Temp Config Smoke

```bash
TMP_CONFIG="$(mktemp)"
cat > "$TMP_CONFIG" <<'EOF'
scene = monolith
color = sun
atmosphere-mode = controlled-live
atmosphere-regime = pulse
EOF

"$BIN" --config "$TMP_CONFIG" -i
"$BIN" --config "$TMP_CONFIG" --color sun -i
rm -f "$TMP_CONFIG"
```

Expected:

- `config_gate`: armed
- `visual_runtime`: protected
- `runtime_application`: identity
- `shadow_risk`: whisper
- CLI `--color sun` forces color sun even when config sets a different color.

## Disabled + Non-Calm Temp Config Smoke

```bash
TMP_CONFIG_DISABLED="$(mktemp)"
cat > "$TMP_CONFIG_DISABLED" <<'EOF'
atmosphere-mode = disabled
atmosphere-regime = pulse
EOF

"$BIN" --config "$TMP_CONFIG_DISABLED" -i
rm -f "$TMP_CONFIG_DISABLED"
```

Expected:

- `application_mode`: disabled
- `effective_runtime`: identity
- `shadow_metrics`: identity
- `shadow_risk`: identity
- `config_gate`: disabled

## AUR Metadata Check

Verify `aur/cosmostrix-bin/PKGBUILD` and `aur/cosmostrix-bin/.SRCINFO` have matching
`pkgver`, `pkgdesc`, and repository URL. Run `./scripts/version-to.sh --check <version>` to
automate this.

## README / CHANGELOG Guard

- README must link to CHANGELOG.md.
- README must not contain release notes sections.
- README must not contain old version-history headings (v2.x.x).
- README must stay under 350 lines.
- CHANGELOG is the dedicated release history document.
- Canonical tagline must be aligned across Cargo.toml, README.md, clap about,
  runtime identity, and AUR pkgdesc.

## Benchmark Interpretation

Benchmark FPS is synthetic uncapped throughput measured in a headless simulation.
The actual runtime target is the configured FPS (normally 60). Do not chase raw
FPS; frame-time stability and p99 latency matter more. See
[benchmark/README.md](../benchmark/README.md) for detailed metric definitions.

## Manual Visual Smoke

Run these interactively and verify clean exit with `q`:

```bash
"$BIN"
"$BIN" --color sun
"$BIN" -mb "one world first seriously matrix rain"
"$BIN" --color green --color-tune "saturation=1.5,brightness=1.2"
```

For the last command, verify the rain renders with visibly boosted
saturation + brightness compared to `--color green` alone.

Also test the live HUD overlay (v11.1.0+): launch `"$BIN"`, press `?`,
verify a top-right overlay showing fps/avg/p99/max/rss appears; press
`?` again to dismiss; verify clean exit with `q`.

Verify:

- Terminal restored cleanly on exit (no raw mode, no alternate screen residue).
- No visual regressions compared to the previous release.
- Color, charset, and scene transitions are smooth.

## Rollback Notes

- Use `git revert` to undo a release commit if issues are found post-push.
- GitHub Releases can be deleted if no users have downloaded the asset.
- AUR package can be reset by bumping `pkgrel` and publishing a fix.
- Do not force-push to `main`; use revert or fix-forward.

## Release Workflow Authentication

The release workflow (`.github/workflows/release.yml`) requires `contents: write`
permission for the `publish_release` job to create and upload GitHub Release assets.
The `GITHUB_TOKEN` is passed explicitly to `softprops/action-gh-release` via `env`.
If the workflow fails with HTTP 401 on the release publish step, verify that the
repository or organization settings have not restricted the default `GITHUB_TOKEN`
permissions.
