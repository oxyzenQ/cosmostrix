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
# Override priority at STARTUP: CLI flags > config.toml > scene defaults.
# At RUNTIME (config save / live-reload): user shortkeys > ambient scene
# (while a phase is active) > config.toml keys (incl. scene-custom block
# fields) > the locked CLI startup value > scene defaults. The CLI wins
# only at startup — a present config key overrides it at runtime; when
# the key is removed, cosmostrix falls back to the locked startup value.
# Validate after editing: cosmostrix --testconf
# File location: ~/.config/cosmostrix/config.toml (see --help for platform paths)
#
# See --list-scenes, --list-colors, --list-charsets

# Standard Settings
# All values shown are defaults. Uncomment to override.

# scene = "cinematic"                   # Built-in name OR a [scene-custom.<name>] block name (v80.0.0-beta.2: custom names accepted — see --list-scenes)
# color = "energy-zen"                  # Built-in theme OR a [colors-custom.<name>] block name (v80.0.0-beta.2: custom names accepted, same as charset — see --list-colors)
# charset = "zen"                       # See: cosmostrix --list-charsets (cinematic default)
# color-bg = "black"                   # or "default-background" (default: black)
# intro = "logo"                        # logo | cosmic | none (default: logo)
# intro-color = "energy-zen"            # intro color override for BOTH cosmic + logo styles (default: brand EnergyZen — NOT the rain color). v80.0.0-beta.1: cosmic burst now chroma-integrated like logo — samples the full intro palette gradient, not just 1 accent color.

# Overlay Message
# Two keys mirror the CLI flags (-m and -mb). If both are present,
# `message-border` wins (border=true). When neither CLI nor config
# provides a message, interactive mode defaults to a bordered overlay
# showing "Experience a masterpiece with cosmostrix v<CARGO_PKG_VERSION>"
# (dynamic via env!, never hardcoded — see default_message_text in
# src/types/constants.rs). Benchmark mode never shows a message overlay.
# Max text length: 200 characters (MESSAGE_MAX_LEN in src/types/constants.rs).
# msg-mode master switch (default: true): when false, disables BOTH the
# default message AND any config message/message-border. CLI -m / -mb
# always wins over msg-mode=false. Set false to turn off the overlay
# entirely via config without removing the message/message-border lines.

# message         = "A masterpiece"     # message WITHOUT border (matches -m)
# message-border  = "A masterpiece"     # message WITH border    (matches -mb)
# msg-mode        = true                # true = overlay active (default), false = suppressed
# msg-fill-style  = "engrave"            # typewriter | fade | words | slide | instant | engrave | hologram | glitch | scorch | cascade (default: engrave — v80.0.0-beta.2 owner champion)
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
# shading-mode = 1                       # 0=random, 1=cinematic

# Color Tuning
# [color.tune]
# brightness = 1.0                      # global (0.0-3.0, default 1.0)
# saturation = 1.0                      # 0.0-3.0 (0.0 = grayscale, >1.0 = oversaturate)
# head = 1.0                            # 0.0-3.0
# body = 1.0                            # 0.0-3.0
# tail = 1.0                            # 0.0-3.0

# Custom Scenes (v80.0.0-beta.2 schema)
# Define named scenes, load with: cosmostrix --scene-custom <name>
# A block is a COMPLETE self-contained profile: ALL six dimensions are
# required (one of each pair). An incomplete block is a hard error at
# startup, on live-reload, and in --testconf.
#   color OR colors-custom   = built-in theme name OR custom palette block
#   charset OR charset-custom = built-in preset OR custom charset block
#   fps = 1-240, speed = 1-100, density = 0.01-5.0, glitch-level = none|subtle|default|intense
# Don't mix a pair (both color and colors-custom) — --testconf will hint.
# REMOVED in v80.0.0-beta.2: base-scene (custom scenes always render glyph
# rain — no built-in inheritance), bold, shading-mode, async-mode (style
# keys are top-level, not per-scene).

# [scene-custom.hacker-mode]
# color = "green"                       # built-in color name (OR colors-custom = "<palette>")
# charset = "hacker"                    # built-in charset (OR charset-custom = "<set>")
# fps = 60                              # 1-240
# speed = 28
# density = 1.2
# glitch-level = "intense"

# [scene-custom.cyberpunk_2077]
# colors-custom = "cyberpunk_2077"      # see [colors-custom.cyberpunk_2077] below
# charset-custom = "cyberpunk_2077"     # see [charset-custom.cyberpunk_2077] below
# fps = 90
# speed = 12
# density = 0.90
# glitch-level = "none"

# [scene-custom.tron_legacy]
# colors-custom = "tron_legacy"
# charset-custom = "tron_legacy"
# fps = 75
# speed = 8
# density = 0.70
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

# IMPORTANT — ambient overlay precedence (v80.0.0-beta.2 honesty note):
# When ANY ambient.HH-MM entry is active, the ambient scene is the ground
# truth for the scene-family dimensions — it outranks config.toml keys
# (and locked CLI values) for those fields while the schedule is
# non-empty. Editing these keys in config mid-run is a no-op until the
# overlay lifts (a custom palette set via config color does NOT survive
# either — the ambient scene re-asserts on every rebuild and phase
# boundary).
#
# Ambient-owned while a phase is active (config edits are no-ops):
#   scene, color, charset, fps, speed, density, glitch-level
#   (and any [scene-custom.<name>] block edits to those same fields)
#
# Still works via config while ambient is active (NOT scene-owned):
#   monolith-size, color-bg, bold, shading-mode,
#   color.tune.* (color tune is a separate layer),
#   power-dragon, crystal-dragon, async-mode,
#   message, message-border, msg-mode, msg-fill-style,
#   ambient-snapback-secs, ambient.HH-MM (editing the schedule itself)
#
# All RUNTIME SHORTKEYS (q/r/c/C/s/S/x/X/p/[/]/Up/Down) work normally
# during ambient — they set user_override_since_ambient=true so the
# ambient scheduler yields control until the next phase boundary
# (or after ambient-snapback-secs of input idle). The '+', '-', '_',
# '=' density aliases were removed (v30 simplify — never documented
# in --help); use '[' and ']' for density down/up. The 'a' shortcut
# was removed (v35) — auto-snapback replaced it. There is no 'i'
# shortkey (verified v80.0.0-beta.2 — stale references removed).
#
# To make ambient-owned config edits take effect: comment out ALL
# ambient.HH-MM entries and save. The schedule empties, the ambient
# overlay lifts (an ambient-owned scene reverts to the locked startup
# scene family — see docs/LIVE_RELOAD_BEHAVIOR.md section 14), and the
# scene-owned config keys become live-editable again.
#
# See docs/LIVE_RELOAD_BEHAVIOR.md section 8 "Known Limitations" and
# section 14 "ambient.* is a config-family overlay on the scene family"
# for the full contract.
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
