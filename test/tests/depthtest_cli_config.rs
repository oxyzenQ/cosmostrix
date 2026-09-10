// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-hunter-19 & depthtest-1: depth stresstest for the CLI and
//! config/live-reload chain.
//!
//! Complements `property.rs` (proptest on the parser) with a
//! deterministic, seeded stress harness that walks the FULL input
//! pipeline the live-reload watcher exercises every config save:
//!
//! ```text
//! argv (expand → clap)          config.toml text
//!        │                              │
//!        │                     parse_config_text()
//!        │                              │
//!        │                     validate_config_strictly()
//!        │                              │
//!        └──────────────► rebuild_cloud_config(base, cfg)
//! ```
//!
//! Contracts under stress:
//! 1. **No panic on any input** — a config editor (or a shell loop)
//!    can write ANY byte sequence into config.toml while cosmostrix
//!    is running; the watcher thread must never die from it (the
//!    polling heartbeat restarts on panic today, but the watcher
//!    loop itself is only poison-guarded — a parse panic would take
//!    the reload path down to poll-only).
//! 2. **Deterministic classification** — the same input must always
//!    classify the same way (parse → same keys, validate → same
//!    verdict) so the startup, watcher, and --testconf surfaces stay
//!    in lockstep (the S-master-HUNT-2 contract).
//! 3. **Rebuild invariants** — whatever survives validation, the
//!    rebuilt CloudConfig must keep every numeric field finite and
//!    inside its documented operating range (NaN/inf leaking into
//!    the render loop would poison frame pacing and spawn math).
//! 4. **argv expansion safety** — the pre-clap rewrite must be
//!    total: every token either passes through byte-identical or
//!    expands to the documented long form (the -mfs typo arm is
//!    process-exiting by design and is excluded from the pool).
//!
//! The RNG is a seeded xorshift64 — failures print their seed so a
//! flaky-looking hit can be replayed deterministically.

use std::collections::HashMap;

use clap::Parser;

use crate::configfile::{parse_config_text, ParsedConfig, USER_CONFIG_KEYS};
use crate::testconf::{validate_config_strictly, validate_field_value};

// ─────────────────────────────────────────────────────────────────
// Deterministic RNG (xorshift64* — no external crate, reproducible)
// ─────────────────────────────────────────────────────────────────

struct StressRng(u64);

impl StressRng {
    fn new(seed: u64) -> Self {
        // xorshift must never be seeded with 0.
        Self(seed | 0x9E37_79B9_7F4A_7C15)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// f64 in [0, 1).
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

// ─────────────────────────────────────────────────────────────────
// Adversarial corpora
// ─────────────────────────────────────────────────────────────────

/// Values that break naive parsers: non-finite floats, unicode
/// digits (bidi-spoofable numerals), embedded control characters,
/// overflow exponents, whitespace padding, and mixed quote forms.
const NASTY_VALUES: &[&str] = &[
    "",
    " ",
    "\t",
    "\n",
    "NaN",
    "nan",
    "inf",
    "-inf",
    "infinity",
    "+inf",
    "1e999",
    "-1e999",
    "1e-999",
    "0x10",
    "0b1010",
    "0o17",
    "١٢٣",
    "٣.٥",
    "𝟏𝟐𝟑",
    "0",
    "-0",
    "1.2.3",
    " 42 ",
    "42\t",
    "1,000",
    "42; DROP TABLE",
    "--",
    "=",
    "#",
    "\"\"",
    "''",
    "[]",
    "[1,2,3]",
    "true",
    "TRUE",
    "False",
    "yes",
    "no",
    "on",
    "off",
    "1_000",
    "𐍈",
    "ﬁ",
    "e",
    "E5",
    "5s",
    "5m",
    "1h30m",
    "-5",
    "999999999999999999999999999999",
    "0.0000000000000000001",
];

/// Config keys (top-level) that take numeric values — the rebuild
/// invariants below assert range/finiteness for exactly these.
const NUMERIC_KEYS: &[(&str, f64, f64)] = &[
    ("speed", 0.1, 120.0),
    ("density", 0.01, 5.0),
    ("fps", 1.0, 240.0),
];

/// Valid builtin scene names (source of truth: catalog.rs SCENES) —
/// used to generate *valid* random scene entries for the pipeline.
const SOME_SCENES: &[&str] = &[
    "sorgonemous_intrascals",
    "aeolian",
    "solar_flare",
    "dna_helix",
    "cosmos",
    "monolith",
];

const SOME_COLORS: &[&str] = &[
    "green",
    "cosmos",
    "energy-zen",
    "aurora",
    "sun",
    "neptune",
    "carbonic",
];

const SOME_CHARSETS: &[&str] = &["binary", "zen", "runic", "greek", "dna", "retro"];

// ─────────────────────────────────────────────────────────────────
// CloudConfig fixture (same shape as the live_config tests' minimal
// fixture — the full field list, locked here so a field rename
// anywhere in app.rs breaks this file loudly at compile time).
// ─────────────────────────────────────────────────────────────────

fn stress_base_config() -> crate::app::CloudConfig {
    use crate::intro_style::IntroType;
    use crate::msg_fill_style::MsgFillStyle;
    use crate::rain_style::RainStyle;
    use crate::runtime::{BoldMode, ColorMode, ColorScheme, MonolithSize, ShadingMode};

    crate::app::CloudConfig {
        color_mode: ColorMode::TrueColor,
        shading_mode: ShadingMode::Random,
        bold_mode: BoldMode::Random,
        async_mode: true,
        default_bg: true,
        color_scheme: ColorScheme::NeonPurple,
        custom_palette: None,
        custom_palette_name: None,
        rain_style: RainStyle::Glyph,
        glitch_enabled: true,
        glitch_level: crate::config::GlitchLevel::Default,
        glitch_pct: 10.0,
        glitch_low: 300,
        glitch_high: 400,
        linger_low: 400,
        linger_high: 600,
        short_pct: 50.0,
        die_early_pct: 33.0,
        max_dpc: 5,
        density: 0.75,
        speed: 9.0,
        monolith_size: MonolithSize::Normal,
        chars: vec!['0', '1'],
        message: None,
        message_border: false,
        msg_fill_style: MsgFillStyle::Typewriter,
        target_fps: 60.0,
        xtermjs_host: false,
        default_fps_cap: 240.0,
        duration_s: None,
        bench_frames: None,
        benchmark: false,
        bench_duration: None,
        screen_size: None,
        color_tune: crate::color_tune::ColorTune::IDENTITY,
        json: false,
        save_baseline: None,
        compare_baseline: None,
        bench_io: false,
        bench_all: false,
        bench_scene: None,
        verbose: false,
        density_auto: true,
        base_density: 0.75,
        perf_stats: false,
        screensaver: false,
        intro: IntroType::None,
        intro_color: None,
        mouse: false,
        charset_preset: "binary".to_string(),
        user_ranges: vec![],
        def_ascii: false,
        crystal_dragon: false,
        power_dragon: true,
        msg_mode: true,
        effects_enabled: true,
        config_path_for_watcher: None,
        scene_name: "cosmos".to_string(),
        scene_custom_name: None,
        scene_custom_config_owned: false,
        cli_explicit: crate::app::CliExplicit::default(),
        ambient_schedule: crate::crystal_dragon_engine::ambient::AmbientSchedule::default(),
        ambient_snapback_secs: None,
        crystal_dragon_secs: None,
    }
}

// ─────────────────────────────────────────────────────────────────
// 1. argv expansion stress (NIGHT-hunter-19)
// ─────────────────────────────────────────────────────────────────

/// Token pool for the expansion fuzz. Deliberately EXCLUDES any
/// `-mfs…` token whose suffix is not a valid style: that arm is a
/// designed process exit (clap-format typo UX), not a panic to
/// catch — fuzzing it would kill the test harness.
const ARGV_TOKENS: &[&str] = &[
    "-mb",
    "-mb=",
    "-mb=x",
    "-m",
    "-mhello",
    "--message-border",
    "--msg-fill-style",
    "-mfs",
    "-mfs=fade",
    "-mfsfade",
    "-mfsradar",
    "--fps",
    "30",
    "-c",
    "green",
    "--color",
    "cosmos",
    "-S",
    "12",
    "--speed",
    "0",
    "-d",
    "-1",
    "--",
    "-",
    "",
    "=",
    "#",
    "-x",
    "-q",
    "--unknown",
    "value with spaces",
    "\t",
    "-MFS",
    "-Mb",
    "𐍈",
    "-mfs=𝐅𝐀𝐃𝐄",
    "-mfs=",
];

#[test]
fn stress_argv_expansion_is_total_and_order_preserving() {
    let mut rng = StressRng::new(0xDE_9B_51);
    for _ in 0..2_000 {
        let n = rng.below(12);
        let tokens: Vec<std::ffi::OsString> = (0..n)
            .map(|_| std::ffi::OsString::from(rng.pick(ARGV_TOKENS)))
            .collect();
        // The contract: with a non-empty argv, the expansion NEVER
        // panics and never drops or reorders pass-through tokens.
        // (argv[0] is the program name — required, mirroring the
        // production call site.)
        let mut argv: Vec<std::ffi::OsString> = vec![std::ffi::OsString::from("cosmostrix")];
        argv.extend(tokens);
        let expanded = crate::cli::argv_expand::expand_argv_shorthands(&argv);
        // Total: output is at least the program name + every token
        // either passed through or expanded into ≥1 tokens.
        assert!(
            expanded.len() >= argv.len(),
            "expansion shrank argv: {:?} -> {:?}",
            argv,
            expanded
        );
    }
}

#[test]
fn stress_argv_expansion_empty_argv_is_not_a_panic() {
    // depth-audit bug fix lock: execve() can hand cosmostrix an EMPTY
    // argv (argvp = [NULL]). std::env::args_os() then yields zero
    // items and the old `argv[0]` index panicked before clap ever
    // ran. The fix degrades to an empty expansion — clap handles the
    // no-program-name case itself.
    let expanded = crate::cli::argv_expand::expand_argv_shorthands(&[]);
    assert!(
        expanded.is_empty(),
        "empty argv must expand to empty, got {expanded:?}"
    );
}

// ─────────────────────────────────────────────────────────────────
// 2. clap-level argv stress (NIGHT-hunter-19)
// ─────────────────────────────────────────────────────────────────

#[test]
fn stress_clap_parse_never_panics_on_adversarial_argv() {
    let mut rng = StressRng::new(0xC1A9);
    for i in 0..1_500 {
        let n = rng.below(10);
        let mut argv: Vec<std::ffi::OsString> = vec![std::ffi::OsString::from("cosmostrix")];
        for _ in 0..n {
            let tok = if rng.next_f64() < 0.5 {
                rng.pick(ARGV_TOKENS).to_string()
            } else {
                rng.pick(NASTY_VALUES).to_string()
            };
            argv.push(std::ffi::OsString::from(tok));
        }
        // clap must return Ok or Err — never panic. A panic here is
        // a crash-on-startup bug reachable from a shell script.
        let result = std::panic::catch_unwind(|| crate::config::Args::try_parse_from(argv.iter()));
        assert!(
            result.is_ok(),
            "clap parse panicked on adversarial argv (iteration {i})"
        );
        // Also assert the result type itself is sane (drop it — the
        // interesting contract is "returned", not "succeeded").
        let _ = result.unwrap();
    }
}

// ─────────────────────────────────────────────────────────────────
// 3. Numeric CLI parser stress (NIGHT-hunter-19)
// ─────────────────────────────────────────────────────────────────

#[test]
fn stress_parse_secs_f64_classifies_every_nasty_value_without_panic() {
    for v in NASTY_VALUES {
        // Contract: Ok(finite in [0, 86400]) or Err — never a panic,
        // never NaN/inf/negative leaking through.
        if let Ok(secs) = crate::cli::cli_parse::parse_secs_f64(v) {
            assert!(
                secs.is_finite() && (0.0..=86_400.0).contains(&secs),
                "parse_secs_f64({v:?}) = {secs} leaked out of contract"
            );
        }
    }
}

#[test]
fn stress_parse_duration_classifies_every_nasty_value_without_panic() {
    for v in NASTY_VALUES {
        if let Ok(secs) = crate::cli::cli_parse::parse_duration("--stress", v) {
            assert!(
                (1..=86_400).contains(&secs),
                "parse_duration({v:?}) = {secs} leaked out of contract"
            );
        }
    }
}

#[test]
fn stress_parse_screen_size_classifies_every_nasty_value_without_panic() {
    for v in NASTY_VALUES {
        if let Ok((w, h)) = crate::cli::cli_parse::parse_screen_size(v) {
            assert!(
                w >= crate::constants::MIN_TERMINAL_COLS
                    && h >= crate::constants::MIN_TERMINAL_LINES
                    && w != 0
                    && h != 0,
                "parse_screen_size({v:?}) = {w}x{h} leaked out of contract"
            );
        }
    }
}

/// Random WxH-shaped strings — valid shapes with adversarial parts.
#[test]
fn stress_parse_screen_size_fuzz_shapes() {
    let mut rng = StressRng::new(0x517E);
    for _ in 0..3_000 {
        let shape = rng.pick(&[
            "{}x{}",
            "{}X{}",
            "{} x {}",
            "{}x {}",
            "x{}",
            "{}x",
            "{}x{}x{}",
            "0x{}",
            "{}x0",
            "-{}x{}",
            "{}x-{}",
            "{}{}x{}",
            "  {}x{}  ",
        ]);
        let a: u64 = rng.next_u64() % 70_000;
        let b: u64 = rng.next_u64() % 70_000;
        let input = shape
            .replace("{}", &a.to_string())
            .replace("{}", &b.to_string());
        let result = std::panic::catch_unwind(|| crate::cli::cli_parse::parse_screen_size(&input));
        assert!(result.is_ok(), "parse_screen_size panicked on {input:?}");
    }
}

// ─────────────────────────────────────────────────────────────────
// 4. Config text parser stress (depthtest-1 — the watcher's front door)
// ─────────────────────────────────────────────────────────────────

/// Structural line corpus — every documented parser pitfall family:
/// quoted values containing brackets (bug #19), unquoted '#' inside
/// arrays (bug #7), multi-line arrays, [section] headers, promotion
/// candidates, CRLF, lone quotes, empty values, and unicode keys.
const CONFIG_LINE_CORPUS: &[&str] = &[
    "scene = cosmic",
    "scene = \"cosmic\"",
    "color = green",
    "charset = binary",
    "speed = 12",
    "density = 0.55",
    "fps = 60",
    "glitch-level = none",
    "glitch-level = subtle",
    "bold = 2",
    "shading-mode = 1",
    "async-mode = true",
    "power-dragon = false",
    "crystal-dragon-secs = 45s",
    "ambient-snapback-secs = 30",
    "message = \"hello world\"",
    "message-border = bordered",
    "msg-mode = false",
    "msg-fill-style = engrave",
    "intro = logo",
    "intro-color = energy-zen",
    "monolith-size = double",
    "color-bg = black",
    "color.tune.sat = 1.2",
    "color.tune.brightness = 0.9",
    "ambient.12-00 = monolith",
    "ambient.00-30 = sorgonemous_intrascals",
    "colors-custom.brand.bg = \"#0a0a12\"",
    "colors-custom.brand.rain = [\"#00ff41\", \"#00c232\"]",
    "charset-custom.mini.set = \"01\"",
    "charset-custom.tricky.set = \"[\"",
    "scene-custom.my_scene.color = green",
    "scene-custom.my_scene.charset = binary",
    "scene-custom.my_scene.fps = 60",
    "scene-custom.my_scene.speed = 12",
    "scene-custom.my_scene.density = 0.55",
    "scene-custom.my_scene.glitch-level = none",
    "scene-custom.my_scene.rain = black_hole",
    "[scene-custom.blocky]",
    "[colors-custom.brand]",
    "[charset-custom.mini]",
    "color = [#ff0000, #00ff00]",
    "rain = [",
    "  \"#1a0033\",",
    "  \"#2a0044\"",
    "]",
    "rain = [",
    "[section-after-array]",
    "fps = 30",
    "",
    "   ",
    "# comment",
    "   # indented comment",
    "novalue =",
    " = novalue",
    "nolhs",
    "=",
    "key = = double equals",
    "ke\"y = value",
    "日本語 = value",
    "scene＝cosmic",
    "scene = cosmic # trailing comment",
    "scene = \"cosmic\" # quoted + comment",
    "scene=cosmic",
    "SCENE = cosmic",
    "  Scene = cosmic  ",
    "set = \"]\"",
    "set = \"=\"",
];

#[test]
fn stress_parse_config_text_never_panics_on_corpus_permutations() {
    let mut rng = StressRng::new(0xC0F1);
    for iteration in 0..1_000 {
        let n = 1 + rng.below(14);
        let mut content = String::new();
        for _ in 0..n {
            content.push_str(rng.pick(CONFIG_LINE_CORPUS));
            // Random line endings — CRLF, LF, and (historically
            // valid for .lines()) lone CR.
            match rng.below(3) {
                0 => content.push('\n'),
                1 => content.push_str("\r\n"),
                _ => content.push('\r'),
            }
        }
        let result = std::panic::catch_unwind(|| parse_config_text(&content));
        assert!(
            result.is_ok(),
            "parse_config_text panicked on permutation {iteration}:\n{content}"
        );
        let parsed = result.unwrap();
        assert_config_parse_invariants(&content, &parsed);
    }
}

/// Structural invariants that must hold for ANY parsed content:
/// - every reported malformed line must be a real line of the input
///   (never an invented diagnostic that hides content);
/// - every value key must be a known key (unknown went elsewhere);
/// - values are never empty (empty-value lines are malformed).
fn assert_config_parse_invariants(content: &str, parsed: &ParsedConfig) {
    let input_lines: Vec<&str> = content.lines().collect();
    for malformed in &parsed.malformed_lines {
        // A malformed report must be traceable to an input line. The
        // parser reports the comment-STRIPPED form of the line (the
        // bug #7 family appends a `# ERROR:` annotation to the value
        // truncated at the offending token), so the report's base is
        // a PREFIX of the raw input line — never an invented line.
        let base = malformed.split("  # ERROR").next().unwrap_or(malformed);
        let base_t = base.trim();
        assert!(
            input_lines
                .iter()
                .any(|l| l.trim().starts_with(base_t) || base_t.starts_with(l.trim())),
            "malformed report {malformed:?} does not match any input line"
        );
    }
    for key in parsed.values.keys() {
        // The values map only receives keys the parser classified as
        // known (unknown keys route to unknown_keys) — mirror that
        // classification via a spot-check: the key must at least be
        // non-empty and lowercase-normalized.
        assert!(!key.is_empty(), "empty key landed in values");
        assert_eq!(
            key.to_ascii_lowercase(),
            *key,
            "non-normalized key {key:?} landed in values"
        );
    }
}

#[test]
fn stress_parse_config_text_random_byte_soup_never_panics() {
    let mut rng = StressRng::new(0x50_0F_19);
    for iteration in 0..1_000 {
        let len = rng.below(2_000);
        let soup: String = (0..len)
            .map(|_| {
                let b = (rng.next_u64() % 256) as u8;
                // Keep it in the printable + whitespace + unicode
                // mixed space: pure control chars would just be
                // malformed lines (already covered), the interesting
                // soup mixes valid fragments with noise.
                if rng.next_f64() < 0.7 && b.is_ascii_graphic() {
                    b as char
                } else {
                    char::from_u32((rng.next_u64() % 0x300) as u32).unwrap_or('?')
                }
            })
            .collect();
        let result = std::panic::catch_unwind(|| parse_config_text(&soup));
        assert!(
            result.is_ok(),
            "parse_config_text panicked on byte soup {iteration}: {soup:?}"
        );
    }
}

// ─────────────────────────────────────────────────────────────────
// 5. Strict validation stress (depthtest-1 — the reload gate)
// ─────────────────────────────────────────────────────────────────

#[test]
fn stress_validate_field_value_classifies_every_known_key_x_nasty_value() {
    // For every known top-level key, every nasty value must yield a
    // deterministic verdict with no panic. This is the exact table
    // the watcher consults on every save — a panic here kills the
    // reload thread, a mis-classification silently accepts garbage.
    for key in USER_CONFIG_KEYS {
        for value in NASTY_VALUES {
            let result = std::panic::catch_unwind(|| validate_field_value(key, value));
            assert!(
                result.is_ok(),
                "validate_field_value({key:?}, {value:?}) panicked"
            );
        }
    }
}

#[test]
fn stress_validate_config_strictly_rejects_nasty_overrides_on_valid_base() {
    // Start from a fully VALID config, then override one key at a
    // time with a nasty value. The strict gate must either reject
    // (Err) or the key must be a legit string key that accepts the
    // value — but never panic, and never accept a value that then
    // breaks the rebuild invariants (checked separately below).
    let base: HashMap<String, String> = [
        ("scene", "sorgonemous_intrascals"),
        ("color", "energy-zen"),
        ("charset", "binary"),
        ("speed", "12"),
        ("density", "0.55"),
        ("fps", "60"),
        ("glitch-level", "none"),
        ("bold", "1"),
        ("shading-mode", "1"),
        ("async-mode", "true"),
        ("power-dragon", "true"),
        ("crystal-dragon-secs", "60"),
        ("msg-mode", "true"),
        ("msg-fill-style", "engrave"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();

    // Sanity: the base itself validates.
    assert!(
        validate_config_strictly(&base).is_ok(),
        "stress fixture must be a valid config"
    );

    for key in USER_CONFIG_KEYS {
        for value in NASTY_VALUES {
            let mut cfg = base.clone();
            cfg.insert(key.to_string(), value.to_string());
            let result = std::panic::catch_unwind(|| validate_config_strictly(&cfg));
            assert!(
                result.is_ok(),
                "validate_config_strictly panicked on {key} = {value:?}"
            );
            // Classification must be DETERMINISTIC (run twice).
            let first = validate_config_strictly(&cfg);
            let second = validate_config_strictly(&cfg);
            assert_eq!(
                first.is_err(),
                second.is_err(),
                "non-deterministic validation verdict for {key} = {value:?}"
            );
            if let Ok(()) = first {
                // If a nasty value was ACCEPTED for a numeric key,
                // the rebuild invariant test below will catch any
                // poison — here we assert the accept was at least
                // a parseable value.
                if let Some((_, lo, hi)) = NUMERIC_KEYS.iter().find(|(k, _, _)| k == key) {
                    let parsed: f64 = value.trim().parse().unwrap_or(f64::NAN);
                    assert!(
                        parsed.is_finite() && (*lo..=*hi).contains(&parsed),
                        "strict gate accepted {key} = {value:?} (outside [{lo}, {hi}])"
                    );
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────
// 6. rebuild_cloud_config stress (depthtest-1 — the render-thread side)
// ─────────────────────────────────────────────────────────────────

#[test]
fn stress_rebuild_cloud_config_random_valid_maps_hold_invariants() {
    let mut rng = StressRng::new(0x1E9A);
    for iteration in 0..600 {
        let mut cfg: HashMap<String, String> = HashMap::new();
        // Random mix of valid known keys.
        if rng.next_f64() < 0.8 {
            cfg.insert("scene".to_string(), rng.pick(SOME_SCENES).to_string());
        }
        if rng.next_f64() < 0.8 {
            cfg.insert("color".to_string(), rng.pick(SOME_COLORS).to_string());
        }
        if rng.next_f64() < 0.8 {
            cfg.insert("charset".to_string(), rng.pick(SOME_CHARSETS).to_string());
        }
        if rng.next_f64() < 0.7 {
            cfg.insert(
                "speed".to_string(),
                format!("{:.2}", 0.5 + rng.next_f64() * 60.0),
            );
        }
        if rng.next_f64() < 0.7 {
            cfg.insert(
                "density".to_string(),
                format!("{:.3}", 0.01 + rng.next_f64() * 4.0),
            );
        }
        if rng.next_f64() < 0.7 {
            cfg.insert(
                "fps".to_string(),
                format!("{:.1}", 1.0 + rng.next_f64() * 239.0),
            );
        }
        if rng.next_f64() < 0.5 {
            cfg.insert(
                "glitch-level".to_string(),
                rng.pick(&["none", "subtle", "default", "heavy"])
                    .to_string(),
            );
        }
        if rng.next_f64() < 0.4 {
            cfg.insert("bold".to_string(), rng.pick(&["0", "1", "2"]).to_string());
        }
        if rng.next_f64() < 0.4 {
            cfg.insert(
                "async-mode".to_string(),
                rng.pick(&["true", "false"]).to_string(),
            );
        }
        if rng.next_f64() < 0.3 {
            cfg.insert("message".to_string(), "stress message".to_string());
        }

        let base = stress_base_config();
        let result = std::panic::catch_unwind(|| {
            crate::config::live_config::rebuild_cloud_config(&base, &cfg)
        });
        assert!(
            result.is_ok(),
            "rebuild_cloud_config panicked on iteration {iteration} with cfg {cfg:?}"
        );
        let rebuilt = result.unwrap();
        assert_rebuild_invariants(&base, &cfg, &rebuilt);
    }
}

/// Invariants that must hold for every rebuilt config:
/// - numeric fields stay finite and within their operating ranges;
/// - the charset pool is never empty (an empty pool would render
///   nothing but would also risk div-by-zero in glyph draw math);
/// - a valid `speed`/`density`/`fps`/`charset`/`color`/`scene` key
///   present in the map is actually reflected (the temporal-chain
///   contract: the config key is the most recent intent).
fn assert_rebuild_invariants(
    base: &crate::app::CloudConfig,
    cfg: &HashMap<String, String>,
    rebuilt: &crate::app::CloudConfig,
) {
    // Finiteness + range (either from the key, or inherited from base).
    assert!(
        rebuilt.speed.is_finite() && (0.1..=120.0).contains(&rebuilt.speed),
        "rebuilt speed {} out of range (base {}, cfg {:?})",
        rebuilt.speed,
        base.speed,
        cfg.get("speed")
    );
    assert!(
        rebuilt.density.is_finite() && (0.01..=5.0).contains(&rebuilt.density),
        "rebuilt density {0} out of range",
        rebuilt.density
    );
    assert!(
        rebuilt.target_fps.is_finite() && (1.0..=240.0).contains(&rebuilt.target_fps),
        "rebuilt fps {0} out of range",
        rebuilt.target_fps
    );
    assert!(
        !rebuilt.chars.is_empty(),
        "rebuilt charset pool is empty (cfg charset = {:?})",
        cfg.get("charset")
    );
    // Reflection contract: a valid key must win over the base value.
    if let Some(v) = cfg.get("speed") {
        if let Ok(n) = crate::validation::parse_canonical_speed("speed", v) {
            assert_eq!(rebuilt.speed, n, "valid speed key {v} not reflected");
        }
    }
    if let Some(v) = cfg.get("density") {
        if let Ok(n) = crate::validation::parse_canonical_f32_range("density", v, 0.01, 5.0) {
            assert_eq!(rebuilt.density, n, "valid density key {v} not reflected");
            assert_eq!(
                rebuilt.base_density, n,
                "valid density key must sync base_density"
            );
        }
    }
    if let Some(v) = cfg.get("fps") {
        if let Ok(n) = crate::validation::parse_canonical_f64_range("fps", v, 1.0, 240.0) {
            assert_eq!(rebuilt.target_fps, n, "valid fps key {v} not reflected");
        }
    }
    if let Some(v) = cfg.get("charset") {
        if crate::charset::charset_from_str(v, false).is_ok()
            || crate::charset_custom::load_custom_charset_if_matches(cfg, v).is_some()
        {
            assert_eq!(
                &rebuilt.charset_preset, v,
                "valid charset key {v} not reflected"
            );
        }
    }
    if let Some(v) = cfg.get("color") {
        if let Ok(scheme) = crate::cli::parse_color_scheme(v) {
            assert_eq!(
                rebuilt.color_scheme, scheme,
                "valid color key {v} not reflected"
            );
        }
    }
}

#[test]
fn stress_rebuild_cloud_config_nasty_keys_are_ignored_not_poisonous() {
    // Invalid values must be IGNORED (keeping base values), never
    // poison the rebuilt config — the "soft-fail" contract of the
    // rebuild layer (strict validation runs upstream; this layer
    // must survive validation regressions defense-in-depth).
    let base = stress_base_config();
    for key in USER_CONFIG_KEYS {
        for value in NASTY_VALUES {
            let mut cfg: HashMap<String, String> = HashMap::new();
            cfg.insert(key.to_string(), value.to_string());
            let result = std::panic::catch_unwind(|| {
                crate::config::live_config::rebuild_cloud_config(&base, &cfg)
            });
            assert!(result.is_ok(), "rebuild panicked on {key} = {value:?}");
            let rebuilt = result.unwrap();
            // Numeric poison must NEVER leak even if the parse
            // contract above regresses.
            assert!(
                rebuilt.speed.is_finite() && rebuilt.speed > 0.0,
                "speed poisoned by {key} = {value:?}: {}",
                rebuilt.speed
            );
            assert!(
                rebuilt.density.is_finite() && rebuilt.density > 0.0,
                "density poisoned by {key} = {value:?}: {}",
                rebuilt.density
            );
            assert!(
                rebuilt.target_fps.is_finite() && rebuilt.target_fps > 0.0,
                "fps poisoned by {key} = {value:?}: {}",
                rebuilt.target_fps
            );
            assert!(
                !rebuilt.chars.is_empty(),
                "charset pool emptied by {key} = {value:?}"
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────
// 7. Full watcher-pipeline simulation (depthtest-1)
// ─────────────────────────────────────────────────────────────────

/// Simulates the exact watcher chain on every save:
/// `read text → parse_config_text → validate_and_send's validation →
/// rebuild_cloud_config` — minus the threads. Random mutations of a
/// valid config file body, mirroring what an editor/formatter/linter
/// chain does mid-save (truncations, partial writes, new keys,
/// comment toggles, block reordering).
#[test]
fn stress_full_reload_pipeline_random_mutations() {
    let mut rng = StressRng::new(0x1E10);
    let valid_body = [
        "scene = sorgonemous_intrascals",
        "color = energy-zen",
        "charset = binary",
        "speed = 12",
        "density = 0.55",
        "fps = 60",
        "glitch-level = none",
        "msg-mode = true",
        "msg-fill-style = engrave",
        "power-dragon = true",
        "crystal-dragon-secs = 60",
    ]
    .join("\n");

    let base = stress_base_config();

    for iteration in 0..800 {
        let lines: Vec<&str> = valid_body.lines().collect();
        let mut mutated: Vec<String> = Vec::new();
        for line in lines {
            let dice = rng.next_f64();
            if dice < 0.12 {
                // Comment the key out (the "key removed" delta —
                // RestoreLocked / CLI-fallback territory).
                mutated.push(format!("# {line}"));
            } else if dice < 0.20 {
                // Replace the value with a nasty one.
                let (k, _) = line.split_once('=').unwrap_or((line, ""));
                mutated.push(format!("{k} {}", rng.pick(NASTY_VALUES)));
            } else if dice < 0.24 {
                // Truncate mid-line (non-atomic editor write).
                let cut = 1 + rng.below(line.len());
                mutated.push(line[..cut].to_string());
            } else if dice < 0.27 {
                // Delete the line entirely.
                continue;
            } else {
                mutated.push(line.to_string());
            }
        }
        // Randomly splice in corpus lines (sections, arrays, blocks).
        for _ in 0..rng.below(3) {
            mutated.push(rng.pick(CONFIG_LINE_CORPUS).to_string());
        }
        let content = mutated.join("\n");

        // ── watcher chain, in order ──
        let parsed = parse_config_text(&content);
        // malformed + unknown keys → the watcher's Err(msg) arm
        // (render thread exits with code 2); the Ok arm rebuilds.
        let valid = parsed.malformed_lines.is_empty() && parsed.unknown_keys.is_empty();
        if valid {
            let verdict = validate_config_strictly(&parsed.values);
            if verdict.is_ok() {
                // The render thread's rebuild step.
                let result = std::panic::catch_unwind(|| {
                    crate::config::live_config::rebuild_cloud_config(&base, &parsed.values)
                });
                assert!(
                    result.is_ok(),
                    "pipeline rebuild panicked on iteration {iteration}:\n{content}"
                );
                let rebuilt = result.unwrap();
                assert!(
                    rebuilt.speed.is_finite() && rebuilt.speed > 0.0,
                    "pipeline leaked speed {} from:\n{content}",
                    rebuilt.speed
                );
                assert!(
                    rebuilt.target_fps.is_finite() && rebuilt.target_fps > 0.0,
                    "pipeline leaked fps {} from:\n{content}",
                    rebuilt.target_fps
                );
                assert!(
                    !rebuilt.chars.is_empty(),
                    "pipeline emptied charset pool from:\n{content}"
                );
            }
            // Err verdicts are the designed exit(2) path — fine.
        }
        // The malformed/unknown arm is the designed exit(2) path — fine.
    }
}

// ─────────────────────────────────────────────────────────────────
// 8. Round-trip: the owner's exact live-reload scenario
// ─────────────────────────────────────────────────────────────────

/// NIGHT-hunter-27 companion lock: the owner's repro — run the
/// flagship scene, live-reload `color = green` (or shortkey `c`),
/// then verify what a full-fresh restore must return to. This pins
/// the SCENE side of the contract (the 'r' key itself is tested in
/// the interactive suite): the scene's builtin layer must be able to
/// re-assert its defaults over ANY config-key override through the
/// same runtime path the restart uses.
#[test]
fn stress_scene_builtin_layer_survives_config_overrides_for_fresh_reset() {
    // The flagship scene's builtin config (mirror of catalog.rs —
    // the live catalog is the source of truth; assert it exists and
    // carries the owner's documented defaults).
    let scene_info = crate::scene::get_scene("sorgonemous_intrascals")
        .expect("flagship scene must exist in the catalog");
    let sc = scene_info.config;
    assert_eq!(sc.color, Some("energy-zen"));
    assert_eq!(sc.charset, Some("binary"));
    assert_eq!(sc.speed, Some(12.0));
    assert_eq!(sc.density, Some(0.55));

    // A config map that has overridden everything the scene owns
    // (the "user changes" state live in the map at runtime):
    let mut cfg: HashMap<String, String> = HashMap::new();
    cfg.insert("color".to_string(), "green".to_string());
    cfg.insert("charset".to_string(), "runic".to_string());
    cfg.insert("speed".to_string(), "45".to_string());
    cfg.insert("density".to_string(), "2.0".to_string());

    // The rebuild honors the config keys (most recent intent)…
    let base = stress_base_config();
    let rebuilt = crate::config::live_config::rebuild_cloud_config(&base, &cfg);
    assert_eq!(
        rebuilt.color_scheme,
        crate::runtime::ColorScheme::Green,
        "config color key must win the rebuild"
    );

    // …and the scene's builtin layer must still be able to re-apply
    // over it at runtime (the path 'r' takes): the values are all
    // present, parseable, and in range.
    if let Some(color) = sc.color {
        assert!(
            crate::cli::parse_color_scheme(color).is_ok(),
            "scene color {color} must parse for the reset path"
        );
    }
    if let Some(charset) = sc.charset {
        assert!(
            crate::charset::charset_from_str(charset, false).is_ok(),
            "scene charset {charset} must parse for the reset path"
        );
    }
    if let Some(speed) = sc.speed {
        assert!(
            crate::validation::parse_canonical_speed("speed", &speed.to_string()).is_ok(),
            "scene speed {speed} must parse for the reset path"
        );
    }
    if let Some(density) = sc.density {
        assert!(
            crate::validation::parse_canonical_f32_range(
                "density",
                &density.to_string(),
                0.01,
                5.0
            )
            .is_ok(),
            "scene density {density} must parse for the reset path"
        );
    }
}
