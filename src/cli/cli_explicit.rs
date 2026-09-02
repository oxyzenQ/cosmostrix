// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CliExplicit construction — extracted from `main.rs` to keep that file
//! under the 800-LOC cap. Pure code motion — no behavior change.
//!
//! Captures which CLI flags were explicitly set by the user (via clap's
//! ValueSource::CommandLine check). v80.0.0-beta.1 owner contract (2026-09-01):
//! the flags are the CLI LOCK, not a config blocker —
//!
//! ```text
//! Startup:  CLI > config.toml > scene defaults > built-in defaults
//! Runtime:  config key > CLI lock > scene defaults > built-in defaults
//! ```
//!
//! At startup the CLI flags win over config.toml. After startup an
//! explicit config key overrides the flag (the file edit is the most
//! recent user intent), but the CLI value stays locked underneath: when
//! the key is commented out, the engine falls back to the locked startup
//! value. `rebuild_cloud_config` reads the flags for the fallback arms
//! (color.tune / message / msg-mode) and the scene-default gates; the
//! event loop reads `any()` for the ambient startup deferral.

use clap::ArgMatches;

use crate::app::CliExplicit;

/// Build the CliExplicit struct from clap's ArgMatches.
///
/// Returns (cli_explicit_color, cli_explicit) — the color flag is tracked
/// separately because it's also used by the verbose startup block.
pub(crate) fn build_cli_explicit(matches: &ArgMatches) -> (bool, CliExplicit) {
    let cli_explicit_color = matches!(
        matches.value_source("color"),
        Some(clap::parser::ValueSource::CommandLine)
    );
    // Bug 3 fix: capture which CLI flags were explicitly set. v80.0.0-beta.1 owner
    // contract: the flags form the CLI LOCK — startup resolution is
    // CLI > config.toml > scene defaults, and at runtime a config key
    // overrides the flag only while present (commenting the key out falls
    // back to the locked startup value).
    let cli_explicit = crate::app::CliExplicit {
        color: cli_explicit_color,
        charset: matches!(
            matches.value_source("charset"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        speed: matches!(
            matches.value_source("speed"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        density: matches!(
            matches.value_source("density"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        fps: matches!(
            matches.value_source("fps"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        scene: matches!(
            matches.value_source("scene"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        glitch_level: matches!(
            matches.value_source("glitch_level"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        // Same intent tracking for --crystal-dragon.
        crystal_dragon: matches!(
            matches.value_source("crystal_dragon"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        // v80.0.0-alpha.1: --crystal-dragon-secs counts as user intent
        // for the ambient startup deferral (CliExplicit::any()).
        crystal_dragon_secs: matches!(
            matches.value_source("crystal_dragon_secs"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        // v50.0.0-alpha.7: track --power-dragon, --async-mode, --msg-mode,
        // --intro-color, and -m/-mb CLI explicit (was missing — live-reload
        // path overrode CLI intent on config edit).
        power_dragon: matches!(
            matches.value_source("power_dragon"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        async_mode: matches!(
            matches.value_source("async_mode"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        msg_mode: matches!(
            matches.value_source("msg_mode"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        // v80.0.0-beta.1 msg-fill-style: track -mfs/--msg-fill-style CLI explicit so
        // live-reload preserves CLI intent over config.toml edits.
        msg_fill_style: matches!(
            matches.value_source("msg_fill_style"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        intro_color: matches!(
            matches.value_source("intro_color"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        message: matches!(
            matches.value_source("message"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        // v50.0.0-alpha.7: track --monolith-size CLI explicit (Issue #4).
        monolith_size: matches!(
            matches.value_source("monolith_size"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        // v50.0.0-alpha.7: track --color-tune CLI explicit (color.tune
        // reset-on-comment fix — when CLI --color-tune is set, config
        // [color.tune] block absence must NOT reset to identity).
        color_tune: matches!(
            matches.value_source("color_tune"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        // Z-master-2-v2: track --bold / --shading-mode / --color-bg /
        // --colors-custom / --scene-custom CLI explicit (all were missing —
        // the live-reload path overrode CLI intent on config edit, same bug
        // class as the monolith-size Issue #4 fix).
        bold: matches!(
            matches.value_source("bold"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        shading_mode: matches!(
            matches.value_source("shading_mode"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        color_bg: matches!(
            matches.value_source("color_bg"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        colors_custom: matches!(
            matches.value_source("colors_custom"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        scene_custom: matches!(
            matches.value_source("scene_custom"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
    };
    (cli_explicit_color, cli_explicit)
}
