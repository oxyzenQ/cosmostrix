// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Config dump + fingerprint helpers — extracted from `configfile.rs`
//! to keep that file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns 4 pure functions:
//! - `dump_config_text()`: the commented config template body (raw string).
//! - `dump_config_with_header()`: template + timestamp + sha512 fingerprint.
//! - `sha512_hex(data)`: SHA-512 hash as 128-char lowercase hex.
//! - `extract_template_fingerprint(content)`: parses the
//!   `# template-fingerprint: <hash>` header from a config file.
//!
//! Re-exported from `configfile.rs` via `pub(crate) use` so all
//! existing `crate::configfile::{dump_config_text, sha512_hex, ...}`
//! call sites resolve unchanged.

use sha2::{Digest, Sha512};

pub(crate) fn dump_config_text() -> &'static str {
    r##"# cosmostrix configuration
#
# Override priority: CLI flags > config.toml > scene defaults.
# Validate after editing: cosmostrix --testconf
# File location: ~/.config/cosmostrix/config.toml (see --help for platform paths)
#
# See --list-scenes, --list-colors, --list-charsets

# Standard Settings
# All values shown are defaults. Uncomment to override.

# scene = "cinematic"                   # See: cosmostrix --list-scenes
# color = "energy-zen"                  # See: cosmostrix --list-colors (cinematic default)
# charset = "zen"                       # See: cosmostrix --list-charsets (cinematic default)
# color-bg = "default-background"       # or "black"
# intro = "logo"                        # logo | cosmic | none (default: logo)
# intro-color = "energy-zen"            # intro color override (default: same as rain color)

# Overlay Message
# Two keys mirror the CLI flags (-m and -mb). If both are present,
# `message-border` wins (border=true). When neither CLI nor config
# provides a message, interactive mode defaults to a bordered overlay
# showing "cosmostrix v<CARGO_PKG_VERSION>" (dynamic, never hardcoded).
# Benchmark mode never shows a message overlay.
# Max text length: 200 characters (MESSAGE_MAX_LEN in src/types/constants.rs).
# msg-mode master switch (default: true): when false, disables BOTH the
# default message AND any config message/message-border. CLI -m / -mb
# always wins over msg-mode=false. Set false to turn off the overlay
# entirely via config without removing the message/message-border lines.

# message         = "A masterpiece"     # message WITHOUT border (matches -m)
# message-border  = "A masterpiece"     # message WITH border    (matches -mb)
# msg-mode        = true                # true = overlay active (default), false = suppressed
# msg-fill-style  = "typewriter"        # typewriter | fade | words | slide | instant | engrave | hologram | glitch | scorch | cascade
#                                       # reveal animation for the overlay (CLI: -mfs/--msg-fill-style)

# Motion

# fps = 60                              # 1-240 (default: dynamic — 60 or 144 on high-refresh)
# speed = 9                             # 1-100 (cinematic default)
# density = 0.75                        # 0.01-5.0 (cinematic default)
# async-mode = true                     # variable column speeds (CLI: --async-mode true|false)
# monolith-size = "normal"              # small | normal | large (monolith scene only)

# Behavior

# glitch-level = "subtle"               # none | subtle | default | intense (cinematic default)
# power-dragon = true                   # Power Dragon adaptive protection (true=throttle on pressure, false=keep user settings)
# crystal-dragon = false                # Crystal Dragon ambient color drift (point-based temperature groups)
# ambient-snapback-secs = 30            # 0.0..=86400.0 (30s default; 86400=disable snapback; 0=instant)
# bold = 1                              # 0=off, 1=random, 2=all
# shadingmode = 1                       # 0=random, 1=cinematic

# Color Tuning
# [color.tune]
# brightness = 1.0                      # global (0.0-3.0, default 1.0)
# saturation = 1.0                      # 0.0-3.0 (0.0 = grayscale, >1.0 = oversaturate)
# head = 1.0                            # 0.0-3.0
# body = 1.0                            # 0.0-3.0
# tail = 1.0                            # 0.0-3.0

# Custom Scenes
# Define named scenes, load with: cosmostrix --scene-custom <name>
# Paired fields: `color`/`charset` = built-in name; `colors-custom`/`charset-custom`
# = block reference. Don't mix — --testconf will hint if you do.

# [scene-custom.hacker-mode]
# base-scene = "matrix"                 # inherit defaults from a built-in scene
# color = "green"                       # built-in color name
# colors-custom = "neon"                # custom palette name (overrides color)
# charset = "hacker"                    # built-in charset preset
# charset-custom = "myglyphs"           # custom charset name (overrides charset)
# speed = 28
# density = 1.2
# fps = 60                              # 1-240
# bold = 1                              # 0=off, 1=random, 2=all
# shadingmode = 1                       # 0=random, 1=cinematic
# glitch-level = "intense"
# density-map = "0.5,1.0,1.5,1.0,0.5"   # per-zone density weights (each 0.0-1.0, out-of-range clamped)
# async-mode = true                     # variable column speeds

# [scene-custom.cyberpunk_2077]
# base-scene = "storm"                  # inherit storm defaults (purple, cyberpunk)
# colors-custom = "cyberpunk_2077"
# charset-custom = "cyberpunk_2077"
# speed = 12
# density = 0.90
# bold = 1
# shadingmode = 1
# glitch-level = "intense"

# [scene-custom.tron_legacy]
# base-scene = "signal"                 # inherit signal defaults (aurora, retro)
# colors-custom = "tron_legacy"
# charset-custom = "tron_legacy"
# speed = 8
# density = 0.70
# bold = 1
# shadingmode = 1
# glitch-level = "subtle"

# Custom Color Palettes
# Define named palettes, reference via: colors-custom = <name>
# Hex values MUST be quoted: "#ff0000" (unquoted # = TOML comment).
# rain stops: min 2, no hard max — but the OKLab gradient engine expands
# all stops to exactly 9 perceptual samples. 7 stops is the sweet spot
# (enough anchors for smooth interpolation; more than ~8 gives no
# visible improvement since output is always 9 samples).

# [colors-custom.zen]
# bg = "#0a0a0a"
# rain = ["#1a0033", "#4d0080", "#9933ff", "#cc66ff", "#e6b3ff", "#f2ccff", "#ffffff"]

# [colors-custom.cyberpunk_2077]
# bg = "#0A0008"
# rain = ["#FFE100", "#FF6B00", "#FF0066", "#FF00CC", "#CC00FF", "#00FFFF", "#E0E0E0"]

# [colors-custom.tron_legacy]
# bg = "#02080C"
# rain = ["#002B4D", "#0066AA", "#00BBEE", "#22DDFF", "#88EEFF", "#CCF4FF", "#FFFFFF"]

# Custom Character Sets
# Define named charsets, reference via: charset-custom = <name>
# Rules: printable chars only. Controls → error. Wide/zero-width (CJK, emoji) → silently skipped with warning.
#        max 256 characters per set (exceeding = error at startup/--testconf). TOML is UTF-8 — type the actual glyphs.
#        Any single-width glyph is legal incl. [ ] # = — always quote the value (set = "[" works; a lone " is not expressible).
# Activate: cosmostrix --charset <name>  or  charset = "<name>"

# [charset-custom.zen]
# set = "|"

# [charset-custom.quantum]
# set = "∀∃∄∅∈∉∋∌∏∑∫∂∆∇√∞≈≠≤≥±∓×÷⊕⊗⊖⊘⊙⊚⊛⊜⊝⊞⊟⊠⊡⊢⊣⊤⊥⊦⊧⊨⊩⊪⊫⊬⊭⊮⊯"

# [charset-custom.cyberpunk_2077]
# set = "0123456789ABCDEF<>{}[]|=+*ｱｲｳｴｵﾊﾋﾌﾍﾎﾏ"

# [charset-custom.tron_legacy]
# set = "0123456789ABCDEF←→↑↓█▌▐░▒▓│─┤├┬┴┼"

# Ambient Phase Scheduler
# Time-of-day scene switches. Config-only (no CLI flag).
# Format: ambient.<HH-MM> = <scene-name>  (24-hour, zero-padded)
# Live reload: edits take effect on save.
# Max 256 entries.

# ambient.06-00 = "signal"
# ambient.12-00 = "monolith"
# ambient.20-00 = "cinematic"
"##
}

/// Build the full dump-config output with a generated header prepended.
///
/// The header is 5 comment lines:
///   ```text
///   # cosmostrix config file
///   # generated at <ISO 8601 UTC>
///   # using Howard Hinnant chrono design (libc::gmtime_r)
///   # template-fingerprint: <hex digest of template body>
///   # verify full file: sha512sum <path> or --testconf
///   ```
/// followed by a blank `#` line, then the existing curated `# cosmostrix
/// configuration` template from `dump_config_text()`.
///
/// v30 (Hinnant-style): the timestamp is produced by `clock::now_iso_utc()`
/// which uses direct `libc::gmtime_r` on Unix — no `chrono` dependency. The
/// "Howard Hinnant chrono design" attribution honors the algorithm
/// (civil-from-days + minimal abstraction) without claiming the chrono crate
/// is in use (it was dropped in v30 to eliminate 8 transitive deps).
///
/// v50: SHA-512 fingerprint of the template body (everything after the
/// header). Labelled `template-fingerprint` so users don't confuse it with
/// `sha512sum` of the full file on disk (which includes header lines).
/// Serves as a content-addressable identity — any change to the template
/// produces a different digest. `--testconf` extracts this fingerprint and
/// compares it against the current built-in template to detect drift.
/// Uses the same `sha2` crate already in-tree for live-reload change
/// detection (zero new dependencies). SHA-512 chosen over SHA-256 for
/// higher security margin (256-bit collision resistance vs 128-bit) at
/// negligible cost for small config files (<5 KB). The hash covers only the
/// template body (not the header itself), so the digest is deterministic
/// regardless of when `--dump-config` is run.
///
/// v50 (alpha.2): Added line 5 (`verify full file`) so users who only look at the
/// header immediately know which command produces the full-file hash that
/// matches `sha512sum`. This eliminates the most common confusion:
/// "why doesn't the template hash match sha512sum?".
///
/// Returns a `String` (allocates) instead of `&'static str` because the
/// timestamp is runtime-generated. Callers: `--dump-config` stdout path and
/// `--dump-config <path>` file-write path in `main.rs`.
#[must_use]
pub(crate) fn dump_config_with_header() -> String {
    let ts = crate::clock::now_iso_utc();
    let body = dump_config_text();
    let hash = sha512_hex(body.as_bytes());
    format!(
        "# cosmostrix config file\n# generated at {ts}\n# using Howard Hinnant chrono design (libc::gmtime_r)\n# template-fingerprint: {hash}\n# verify full file: sha512sum <path> or --testconf\n#\n{body}"
    )
}

/// Compute the SHA-512 hex digest of `data`.
///
/// Three distinct scopes:
///   - `dump_config_with_header()` → fingerprints the **template body** only
///     (labelled `template-fingerprint` so users don't expect it to match
///     `sha512sum` of the full file on disk, which includes header lines).
///   - `testconf::run()` → fingerprints the **user's config file on disk**
///     (matches `sha512sum` exactly).
///   - `testconf::run()` → also fingerprints the **current built-in template**
///     at runtime and compares it against the header fingerprint to detect
///     template drift (user edited the commented template body).
///
/// Returns a 128-character lowercase hex string.
#[must_use]
pub(crate) fn sha512_hex(data: &[u8]) -> String {
    let mut hasher = Sha512::new();
    hasher.update(data);
    format!("{:0128x}", hasher.finalize())
}

/// Extract the `template-fingerprint` hex digest from the header of a config
/// file (if present).
///
/// Looks for a line matching `# template-fingerprint: <128 hex chars>` in the
/// first 6 lines of the file. Returns `None` if the header is missing or
/// doesn't contain a fingerprint line (e.g., hand-written config, or pre-v50
/// format).
///
/// Used by `testconf::run()` to detect template drift: the extracted
/// fingerprint is compared against a fresh `sha512_hex(dump_config_text())`
/// computed at runtime.
#[must_use]
pub(crate) fn extract_template_fingerprint(content: &str) -> Option<String> {
    for line in content.lines().take(6) {
        let trimmed = line.trim_start();
        if let Some(hex) = trimmed.strip_prefix("# template-fingerprint: ") {
            let hex = hex.trim();
            // Validate: must be exactly 128 lowercase hex characters.
            if hex.len() == 128 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some(hex.to_owned());
            }
        }
        // Also accept the legacy v50 label for backward compat.
        if let Some(hex) = trimmed.strip_prefix("# sha512 (template): ") {
            let hex = hex.trim();
            if hex.len() == 128 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some(hex.to_owned());
            }
        }
    }
    None
}
