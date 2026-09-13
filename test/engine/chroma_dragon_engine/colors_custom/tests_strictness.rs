// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-hunt-37 strictness-contract tests: rain-stop ceiling (9),
//! rain/stops field overload, block-count ceiling, required bg, and
//! the shared stop splitter. The owner's original repro (a 67-stop
//! `[colors-custom.test]` block that passed every gate) is pinned as
//! a named regression.

use super::super::*;
use super::*; // strictness items (split_rain_stop_entries, the validators) // colors_custom items (block validator, collector, load contract)

fn valid_block_cfg() -> HashMap<String, String> {
    let mut cfg = HashMap::new();
    cfg.insert(
        "colors-custom.test.rain".to_string(),
        "#1a0033, #4d0080, #9933ff".to_string(),
    );
    cfg.insert("colors-custom.test.bg".to_string(), "#0a0a0a".to_string());
    cfg
}

fn stops_value(n: usize) -> String {
    let stops: Vec<String> = (0..n).map(|i| format!("\"#{i:06x}\"")).collect();
    format!("[{}]", stops.join(", "))
}

#[test]
fn splitter_handles_array_csv_and_empty() {
    // TOML array form.
    let arr = split_rain_stop_entries("[\"#1a0033\", \"#4d0080\"]");
    assert_eq!(arr, vec!["#1a0033", "#4d0080"]);
    // CSV form.
    let csv = split_rain_stop_entries("#1a0033, #4d0080");
    assert_eq!(csv, vec!["#1a0033", "#4d0080"]);
    // Quoted CSV (quotes stripped by the splitter, then by hex parse).
    let quoted = split_rain_stop_entries("\"#1a0033\", \"#4d0080\"");
    assert_eq!(quoted, vec!["#1a0033", "#4d0080"]);
    // Empty array / empty entries are filtered, not counted.
    assert!(split_rain_stop_entries("[]").is_empty());
    assert!(split_rain_stop_entries("").is_empty());
    assert_eq!(split_rain_stop_entries("[#111, ]").len(), 1);
    // Bare single color (CSV degenerate).
    assert_eq!(split_rain_stop_entries("#ff0000"), vec!["#ff0000"]);
}

#[test]
fn nine_stops_pass_and_ten_fail() {
    let mut cfg = valid_block_cfg();
    cfg.insert(
        "colors-custom.test.rain".to_string(),
        stops_value(COLORS_CUSTOM_MAX_RAIN_STOPS),
    );
    assert_eq!(
        validate_colors_custom_blocks(&cfg),
        None,
        "exactly {COLORS_CUSTOM_MAX_RAIN_STOPS} stops is the documented maximum and must pass"
    );
    cfg.insert(
        "colors-custom.test.rain".to_string(),
        stops_value(COLORS_CUSTOM_MAX_RAIN_STOPS + 1),
    );
    let e = validate_colors_custom_blocks(&cfg).expect("10 stops must be a hard error");
    assert!(
        e.contains("10 rain stops") && e.contains("maximum is 9"),
        "error must name the count and the ceiling, got: {e}"
    );
}

#[test]
fn hunt37_owner_repro_67_stops_is_a_hard_error() {
    // The owner's exact config shape (2026-09-13): bg + a 67-stop rain
    // array. Pre-hunt-37 this passed every surface (the collector
    // silently capped at 64). It must now be rejected loudly.
    let mut cfg = HashMap::new();
    cfg.insert("colors-custom.test.bg".to_string(), "#0a0a0a".to_string());
    cfg.insert("colors-custom.test.rain".to_string(), stops_value(67));
    let e = validate_colors_custom_blocks(&cfg).expect("67-stop repro must fail");
    assert!(
        e.contains("colors-custom.test.rain") && e.contains("67 rain stops"),
        "error must name the block key and the real stop count, got: {e}"
    );
    // CSV form is the same defect (the pre-hunt-37 twin splitter path).
    let stops: Vec<String> = (0..67).map(|i| format!("#{i:06x}")).collect();
    cfg.insert("colors-custom.test.rain".to_string(), stops.join(", "));
    let e = validate_colors_custom_blocks(&cfg).expect("67-stop CSV repro must fail");
    assert!(e.contains("67 rain stops"), "got: {e}");
}

#[test]
fn rain_and_stops_in_one_block_is_an_overload() {
    let mut cfg = valid_block_cfg();
    cfg.insert(
        "colors-custom.test.stops".to_string(),
        "#111111, #222222".to_string(),
    );
    let e =
        validate_colors_custom_blocks(&cfg).expect("rain + stops in one block must be an error");
    assert!(
        e.contains("both 'rain' and the deprecated 'stops'"),
        "error must name the overload, got: {e}"
    );
    // The deprecated alias alone still satisfies the rain slot.
    let mut cfg = HashMap::new();
    cfg.insert(
        "colors-custom.old.stops".to_string(),
        "#111111, #222222".to_string(),
    );
    cfg.insert("colors-custom.old.bg".to_string(), "#0a0a0a".to_string());
    assert_eq!(
        validate_colors_custom_blocks(&cfg),
        None,
        "a stops-only block is legacy-valid (deprecation warning, not an error)"
    );
}

#[test]
fn block_count_over_100_is_a_hard_error() {
    let mut cfg = HashMap::new();
    for i in 0..=COLORS_CUSTOM_MAX_BLOCKS {
        cfg.insert(
            format!("colors-custom.p{i}.rain"),
            "#111111, #222222".to_string(),
        );
        cfg.insert(format!("colors-custom.p{i}.bg"), "#0a0a0a".to_string());
    }
    let e = validate_colors_custom_blocks(&cfg).expect("101 blocks must be an error");
    assert!(
        e.contains("101 blocks") && e.contains("maximum is 100"),
        "error must name the count and the cap, got: {e}"
    );
    // Exactly at the cap passes.
    cfg.remove("colors-custom.p0.rain");
    cfg.remove("colors-custom.p0.bg");
    assert_eq!(validate_colors_custom_blocks(&cfg), None);
}

#[test]
fn bg_is_required_for_a_loadable_block() {
    // rain without bg: the owner's "must be complete filed" rule.
    let mut cfg = HashMap::new();
    cfg.insert(
        "colors-custom.nobg.rain".to_string(),
        "#111111, #222222".to_string(),
    );
    let e = validate_colors_custom_blocks(&cfg).expect("missing bg must be an error");
    assert!(
        e.contains("colors-custom.nobg") && e.contains("'bg' field"),
        "error must name the block and the missing field, got: {e}"
    );
    // And the runtime load contract agrees (the F-24-1 no-drift rule:
    // validation asks the constructor itself).
    assert!(colors_custom_load_error(&cfg, "nobg").is_some());
    // Complete block: no error from either surface.
    cfg.insert("colors-custom.nobg.bg".to_string(), "#0a0a0a".to_string());
    assert_eq!(validate_colors_custom_blocks(&cfg), None);
    assert_eq!(colors_custom_load_error(&cfg, "nobg"), None);
}

#[test]
fn collector_cap_is_defense_in_depth_at_nine() {
    // The collector itself still bounds its vec (bypass paths such as
    // COSMOSTRIX_SKIP_STARTUP_VALIDATION never see the validator) —
    // now at 9, matching the new contract.
    let mut cfg = HashMap::new();
    cfg.insert("colors-custom.big.rain".to_string(), stops_value(100));
    let map = collect_colors_custom(&cfg);
    assert!(
        map["big"].rain.len() <= COLORS_CUSTOM_MAX_RAIN_STOPS,
        "collector must cap at {}, got {}",
        COLORS_CUSTOM_MAX_RAIN_STOPS,
        map["big"].rain.len()
    );
}
