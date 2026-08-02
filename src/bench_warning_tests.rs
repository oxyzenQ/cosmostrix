// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Benchmark mode noop-flag warning tests.
//!
//! Extracted from main.rs to keep that file under the 1500 LOC cap.
//! Tests the `collect_bench_noop_warnings` pure function and the
//! `clap::ValueSource` mechanism that `cli_explicit.fps` relies on
//! to distinguish CLI-explicit `--fps` from the default value.

use crate::config::Args;
use clap::{CommandFactory, FromArgMatches, Parser};

use super::collect_bench_noop_warnings;

/// Regression: user reported that `cosmostrix --benchmark --fps 60`
/// produced no warning, even though `--fps` in benchmark mode does NOT
/// cap render throughput (it only sets the simulation rate). The prior
/// implementation explicitly skipped `--fps` from `warn_bench_noop_flags`
/// on the rationale that "it has an effect, just a different one". User
/// feedback overruled that — silence caused real confusion. Now the
/// warning MUST fire whenever `--fps` is explicit (CLI) OR set to a
/// non-default value in config.toml (detected via args.fps != 60.0).
#[test]
fn bench_noop_warnings_include_fps_when_user_set() {
    // Default Args (no explicit flags, fps=60.0 default). fps_user_set=false → no --fps warning.
    let args = Args::try_parse_from(["cosmostrix"]).unwrap();
    let warns = collect_bench_noop_warnings(&args, false);
    assert!(
        !warns.iter().any(|w| w.starts_with("--fps")),
        "fps warning should NOT fire when fps not user-set, got: {warns:?}"
    );

    // fps_user_set=true → --fps warning MUST fire and mention "simulation rate".
    let warns = collect_bench_noop_warnings(&args, true);
    let fps_warn = warns.iter().find(|w| w.starts_with("--fps"));
    assert!(
        fps_warn.is_some(),
        "fps warning SHOULD fire when fps user-set, got: {warns:?}"
    );
    let fps_warn = fps_warn.unwrap();
    assert!(
        fps_warn.contains("simulation rate"),
        "fps warning should mention 'simulation rate', got: {fps_warn}"
    );
    assert!(
        fps_warn.contains("does NOT cap"),
        "fps warning should clarify it does NOT cap render throughput, got: {fps_warn}"
    );
    assert!(
        fps_warn.contains("config.toml"),
        "fps warning should hint at config.toml as possible source, got: {fps_warn}"
    );
}

/// Simulate the config.toml `fps = 10` path: clap parses with default
/// fps=60.0, then config_apply would set args.fps=10.0. The call site
/// computes `fps_user_set = cli_explicit.fps || args.fps != 60.0`.
/// When config.toml sets fps=10, cli_explicit.fps=false but
/// args.fps=10.0, so fps_user_set=true → warning fires. This test
/// verifies the detection logic by constructing args with non-default
/// fps (simulating post-config-apply state) and passing fps_user_set=true.
#[test]
fn bench_noop_warnings_catch_config_toml_fps_set() {
    // Simulate: config.toml has `fps = 10`. After config_apply, args.fps=10.0.
    // cli_explicit.fps=false (not on CLI), but args.fps != 60.0.
    // The call site computes: fps_user_set = false || (10.0 != 60.0) = true.
    let args = Args::try_parse_from(["cosmostrix", "--fps", "10"]).unwrap();
    assert_eq!(args.fps, 10.0);
    let fps_user_set = args.fps != 60.0; // simulates config.toml path
    assert!(fps_user_set, "fps=10 should trigger fps_user_set=true");
    let warns = collect_bench_noop_warnings(&args, fps_user_set);
    assert!(
        warns.iter().any(|w| w.starts_with("--fps")),
        "config.toml fps=10 should trigger --fps warning, got: {warns:?}"
    );
}

/// Verify clap's value_source("fps") correctly distinguishes the
/// default-value case from the explicit-CLI case. This is the actual
/// mechanism `cli_explicit.fps` relies on at runtime — if clap ever
/// changes ValueSource semantics, this test will catch it.
#[test]
fn clap_value_source_distinguishes_explicit_fps_from_default() {
    let cmd = Args::command();
    let matches_default = cmd.clone().try_get_matches_from(["cosmostrix"]).unwrap();
    assert!(
        !matches!(
            matches_default.value_source("fps"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        "default --fps should NOT be flagged as CommandLine-sourced"
    );

    // Explicit case: --fps 144 on command line → CommandLine-sourced
    let matches_explicit = cmd
        .try_get_matches_from(["cosmostrix", "--fps", "144"])
        .unwrap();
    assert!(
        matches!(
            matches_explicit.value_source("fps"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        "explicit --fps 144 should be flagged as CommandLine-sourced"
    );

    // Sanity: in both cases the parsed fps value matches expectations.
    let args_default = Args::from_arg_matches(&matches_default).unwrap();
    let args_explicit = Args::from_arg_matches(&matches_explicit).unwrap();
    assert_eq!(args_default.fps, 60.0);
    assert_eq!(args_explicit.fps, 144.0);
}
