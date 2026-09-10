// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-lts-6 (2026-09-10): early-return precedence ladder tests.
//!
//! Pins the canonical command-precedence contract documented in
//! `src/cli/early_returns.rs` so a future refactor cannot silently
//! reorder it. Owner report: combined invocations like
//! `cosmostrix -v -s --dump-config <path> --version --doctor` must
//! select exactly ONE command, and the winner must not depend on the
//! order the flags were typed.
//!
//! The tests use the pure `classify_pre_config` / `classify_post_config`
//! functions (no I/O, no process exits). Dispatch-side behavior
//! (stdout writes, file writes, ux dies) is already covered by the
//! runtime smoke matrix recorded in the module docs.

use super::{classify_post_config, classify_pre_config, PostConfigCmd, PreConfigCmd};
use crate::config::Args;
use clap::Parser;

/// Parse an argv slice into `Args` (same path main() uses, minus the
/// prevalidation/expand stages that are irrelevant to classification).
fn parse(argv: &[&str]) -> Args {
    let mut v = vec!["cosmostrix"];
    v.extend_from_slice(argv);
    Args::try_parse_from(v).expect("argv must parse")
}

// ── single-command sanity (each flag alone fires its own branch) ────

#[test]
fn single_pre_config_commands_fire() {
    assert_eq!(
        classify_pre_config(&parse(&["--help"])),
        Some(PreConfigCmd::Help)
    );
    assert_eq!(
        classify_pre_config(&parse(&["--reset-terminal"])),
        Some(PreConfigCmd::ResetTerminal)
    );
    assert_eq!(
        classify_pre_config(&parse(&["--dump-config"])),
        Some(PreConfigCmd::DumpConfig)
    );
    assert_eq!(
        classify_pre_config(&parse(&["--dump-config", "x.toml"])),
        Some(PreConfigCmd::DumpConfig)
    );
    assert_eq!(
        classify_pre_config(&parse(&["--config-path"])),
        Some(PreConfigCmd::ConfigPath)
    );
    assert_eq!(
        classify_pre_config(&parse(&["--testconf"])),
        Some(PreConfigCmd::Testconf)
    );
    assert_eq!(
        classify_pre_config(&parse(&["--list-scenes"])),
        Some(PreConfigCmd::ListScenes)
    );
    assert_eq!(
        classify_pre_config(&parse(&["--list-charsets"])),
        Some(PreConfigCmd::ListCharsets)
    );
    assert_eq!(
        classify_pre_config(&parse(&["--list-colors"])),
        Some(PreConfigCmd::ListColors)
    );
    assert_eq!(
        classify_pre_config(&parse(&["--show-scene", "cinematic"])),
        Some(PreConfigCmd::ShowScene)
    );
}

#[test]
fn single_post_config_commands_fire() {
    assert_eq!(
        classify_post_config(&parse(&["--doctor"])),
        Some(PostConfigCmd::Doctor)
    );
    assert_eq!(
        classify_post_config(&parse(&["--version"])),
        Some(PostConfigCmd::Version)
    );
    assert_eq!(
        classify_post_config(&parse(&["-V"])),
        Some(PostConfigCmd::Version)
    );
    assert_eq!(
        classify_post_config(&parse(&["--docs"])),
        Some(PostConfigCmd::Docs)
    );
    assert_eq!(
        classify_post_config(&parse(&["--check-update"])),
        Some(PostConfigCmd::CheckUpdate)
    );
}

#[test]
fn runtime_only_flags_do_not_early_return() {
    // The owner's confusion case: `-v -s` alone must NOT trigger any
    // early return — they are inert runtime flags.
    assert_eq!(classify_pre_config(&parse(&["-v", "-s"])), None);
    assert_eq!(classify_post_config(&parse(&["-v", "-s"])), None);
    // Same for the common rain-tuning flags.
    assert_eq!(
        classify_pre_config(&parse(&["-c", "green", "-f", "60", "-d", "1.0"])),
        None
    );
    assert_eq!(
        classify_post_config(&parse(&["-c", "green", "-f", "60", "-d", "1.0"])),
        None
    );
}

// ── cross-phase: pre-config beats post-config, regardless of order ──

#[test]
fn pre_config_beats_post_config_both_argv_orders() {
    // Owner's exact example family: dump-config + version + doctor.
    for order in [
        vec!["-v", "-s", "--dump-config", "--version", "--doctor"],
        vec!["--doctor", "--version", "--dump-config", "-s", "-v"],
        vec!["--version", "--doctor", "--dump-config"],
    ] {
        assert_eq!(
            classify_pre_config(&parse(&order)),
            Some(PreConfigCmd::DumpConfig),
            "argv order {order:?} must select dump-config"
        );
    }
}

#[test]
fn list_scenes_beats_version_and_doctor() {
    // The post-config classifier DOES see --doctor/--version in the
    // parsed Args (they are true), but main() never reaches the
    // post-config phase: handle_pre_config_returns fires first. The
    // sequencing contract is therefore pinned by the pre-config
    // classification below — main() returns before post-config dispatch.
    for order in [
        vec!["--list-scenes", "--version", "--doctor"],
        vec!["--doctor", "--list-scenes", "--version"],
    ] {
        assert_eq!(
            classify_pre_config(&parse(&order)),
            Some(PreConfigCmd::ListScenes),
            "argv order {order:?} must select list-scenes"
        );
    }
}

// ── within-phase pairwise matrix (documented ladder order) ──────────

#[test]
fn pre_config_pairwise_ladder() {
    // help wins over every other pre-config command.
    for other in [
        "--reset-terminal",
        "--dump-config",
        "--config-path",
        "--testconf",
        "--list-scenes",
        "--list-charsets",
        "--list-colors",
    ] {
        for argv in [vec!["--help", other], vec![other, "--help"]] {
            assert_eq!(
                classify_pre_config(&parse(&argv)),
                Some(PreConfigCmd::Help),
                "argv {argv:?}: help must win over {other}"
            );
        }
    }
    // reset-terminal beats dump-config.
    for argv in [
        vec!["--reset-terminal", "--dump-config"],
        vec!["--dump-config", "--reset-terminal"],
    ] {
        assert_eq!(
            classify_pre_config(&parse(&argv)),
            Some(PreConfigCmd::ResetTerminal)
        );
    }
    // dump-config beats config-path / testconf / list-* / show-scene.
    for other in [
        "--config-path",
        "--testconf",
        "--list-scenes",
        "--list-charsets",
        "--list-colors",
    ] {
        for argv in [vec!["--dump-config", other], vec![other, "--dump-config"]] {
            assert_eq!(
                classify_pre_config(&parse(&argv)),
                Some(PreConfigCmd::DumpConfig),
                "argv {argv:?}: dump-config must win over {other}"
            );
        }
    }
    // config-path beats testconf.
    for argv in [
        vec!["--config-path", "--testconf"],
        vec!["--testconf", "--config-path"],
    ] {
        assert_eq!(
            classify_pre_config(&parse(&argv)),
            Some(PreConfigCmd::ConfigPath)
        );
    }
    // testconf beats the list-* family.
    for other in ["--list-scenes", "--list-charsets", "--list-colors"] {
        for argv in [vec!["--testconf", other], vec![other, "--testconf"]] {
            assert_eq!(
                classify_pre_config(&parse(&argv)),
                Some(PreConfigCmd::Testconf),
                "argv {argv:?}: testconf must win over {other}"
            );
        }
    }
    // list order: scenes > charsets > colors (documented ladder).
    for argv in [
        vec!["--list-charsets", "--list-scenes"],
        vec!["--list-scenes", "--list-charsets"],
    ] {
        assert_eq!(
            classify_pre_config(&parse(&argv)),
            Some(PreConfigCmd::ListScenes)
        );
    }
    for argv in [
        vec!["--list-colors", "--list-charsets"],
        vec!["--list-charsets", "--list-colors"],
    ] {
        assert_eq!(
            classify_pre_config(&parse(&argv)),
            Some(PreConfigCmd::ListCharsets)
        );
    }
}

#[test]
fn post_config_pairwise_ladder() {
    // doctor > version > docs > check-update, regardless of argv order.
    for argv in [vec!["--version", "--doctor"], vec!["--doctor", "--version"]] {
        assert_eq!(
            classify_post_config(&parse(&argv)),
            Some(PostConfigCmd::Doctor),
            "argv {argv:?}: doctor must win"
        );
    }
    for argv in [vec!["--docs", "--version"], vec!["--version", "--docs"]] {
        assert_eq!(
            classify_post_config(&parse(&argv)),
            Some(PostConfigCmd::Version),
            "argv {argv:?}: version must win"
        );
    }
    for argv in [
        vec!["--check-update", "--docs"],
        vec!["--docs", "--check-update"],
    ] {
        assert_eq!(
            classify_post_config(&parse(&argv)),
            Some(PostConfigCmd::Docs),
            "argv {argv:?}: docs must win"
        );
    }
    // Combined with inert runtime flags the winner is unchanged.
    assert_eq!(
        classify_post_config(&parse(&["-v", "-s", "--version", "--doctor"])),
        Some(PostConfigCmd::Doctor)
    );
}

// ── boundary contract: post-config commands stay silent pre-apply ───

#[test]
fn post_config_flags_are_invisible_to_pre_config_classifier() {
    // --doctor/--version/--docs/--check-update never fire in the
    // pre-config phase (they need the merged config view), so the
    // pre-config classifier must return None for them alone.
    for flag in ["--doctor", "--version", "-V", "--docs", "--check-update"] {
        assert_eq!(
            classify_pre_config(&parse(&[flag])),
            None,
            "{flag} must not be a pre-config command"
        );
    }
}

#[test]
fn pre_config_flags_are_invisible_to_post_config_classifier() {
    // Conversely the pre-config commands never reach the post-config
    // dispatcher in practice (main returns earlier), and the classifier
    // confirms they are not post-config commands.
    for flag in [
        "--help",
        "--reset-terminal",
        "--config-path",
        "--testconf",
        "--list-scenes",
    ] {
        assert_eq!(
            classify_post_config(&parse(&[flag])),
            None,
            "{flag} must not be a post-config command"
        );
    }
}
