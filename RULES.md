# Cosmostrix Project Rules

## Source file size

All Rust source files under `src/` must stay **under 1,000 gross lines**.
This rule is enforced by `scripts/check-rs-loc.sh`, which runs as part of
`./scripts/build.sh check-all`.

### Scope

- Core source and code files: `src/**/*.rs`, `build.rs`
- Config and build files: `*.toml`, `.cargo/config.toml`, `rust-toolchain.toml`
- Scripts: `*.sh`, `scripts/*.sh`, `benchmark/*.sh`
- CI workflows: `.github/workflows/*.yml`, `.github/FUNDING.yml`

### Excluded

- Documentation: `*.md`, `docs/**/*.md`
- Text and media: `*.txt`, assets, images, videos
- Generated files: `Cargo.lock`, `target/`
- Git metadata: `.git/`

## Module organization

Prefer splitting modules by responsibility over allowing large files.

- `main.rs` should remain bootstrap and wiring only; target 100–300 LOC long-term.
- `cli.rs` may be larger if it contains mostly Clap command definitions, but must
  still stay under 1,000 LOC.
- Module directories (e.g. `src/cloud/`, `src/interactive/`) use `mod.rs` as the
  public entry point and split implementation into focused submodules.
- Tests are colocated with their module in dedicated `tests/` subdirectories.

## Validation

Behavior-preserving refactors must pass the full validation suite:

```bash
scripts/check-rs-loc.sh
scripts/check-headers.sh
cargo fmt --all
cargo test --all --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
./scripts/build.sh check-all
```

## License headers

All core, config, and script files must carry an SPDX license identifier.
See `scripts/check-headers.sh` for the enforced format.

## Code quality

- Clippy must pass with `-D warnings` (warnings are errors).
- `cargo fmt` must report no differences.
- All tests must pass on every commit.
- MSRV: Rust 1.81.0 (stable).

## Test discipline

Tests must verify **behavior**, never **identity**. A test assertion that
a constant value matches itself (tautology) provides zero information and
breaks the suite on every unrelated change.

### Forbidden: tautological version assertions

```rust
// FORBIDDEN — Cargo.toml always contains its own version field.
// Always true, zero information.
assert!(include_str!("../Cargo.toml").contains("version = \"5.0.1\""));

// FORBIDDEN — PKGBUILD/.SRCINFO/README contain their own version.
assert!(include_str!("../aur/cosmostrix-bin/PKGBUILD").contains("pkgver=5.0.1"));
assert!(include_str!("../aur/cosmostrix-bin/.SRCINFO").contains("pkgver = 5.0.1"));
assert!(include_str!("../README.md").contains(r#"TAG="v5.0.1""#));
```

### Allowed: dynamic version assertions

```rust
// ALLOWED — env!() injects the compile-time package version from
// Cargo.toml [package] version. Single source of truth.
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

assert!(include_str!("../Cargo.toml")
    .contains(&format!("version = \"{}\"", CURRENT_VERSION)));
assert!(include_str!("../aur/cosmostrix-bin/PKGBUILD")
    .contains(&format!("pkgver={}", CURRENT_VERSION)));
```

### Forbidden: test-on-test meta-pattern

Tests must not assert that **other test files** contain a particular
literal string. Every version bump would force manual edits across
multiple test files just to satisfy one meta-test.

```rust
// FORBIDDEN — tests that another test file contains a literal version.
let p14 = include_str!("ledger_p14_tests.rs");
assert!(p14.contains("3.1.0"), "p14 tests must assert 3.1.0");
```

### Allowed: historical CHANGELOG assertions

Asserting that a past release has an entry in CHANGELOG is legitimate —
those entries are immutable historical record and remain valid forever.

```rust
// ALLOWED — verifies CHANGELOG has an entry for a historical release.
let changelog = include_str!("../CHANGELOG.md");
assert!(changelog.contains("## v4.0.0"));
assert!(changelog.contains("## v5.0.0"));
```

### Enforcement

`scripts/check-version-anti-patterns.sh` (run by `build.sh check-all`)
scans `src/**/*.rs` for forbidden patterns and fails the build if any
are detected. The guard catches:

- `contains("version = \"X.Y.Z\"")` and `contains(r#"version = "X.Y.Z""#)`
- `contains("pkgver=X.Y.Z")` and `contains("pkgver = X.Y.Z")`
- `contains(r#"TAG="vX.Y.Z""#)` (README install tag)

If a future test genuinely needs the current package version, use
`env!("CARGO_PKG_VERSION")` — never hardcode the literal string.

