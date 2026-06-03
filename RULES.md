# Cosmostrix Project Rules

## Source file size

All Rust source files under `src/` must stay **under 1,000 gross lines**.
This rule is enforced by `scripts/check-rs-loc.sh`, which runs as part of
`./build.sh check-all`.

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
./build.sh check-all
```

## License headers

All core, config, and script files must carry an SPDX license identifier.
See `scripts/check-headers.sh` for the enforced format.

## Code quality

- Clippy must pass with `-D warnings` (warnings are errors).
- `cargo fmt` must report no differences.
- All tests must pass on every commit.
- MSRV: Rust 1.81.0 (stable).
