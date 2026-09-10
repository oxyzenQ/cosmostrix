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
# At RUNTIME (config save / live-reload): shortkeys > the active ambient
# phase > config.toml keys > the locked CLI startup value (the full
# contract lives in docs/LIVE_RELOAD_BEHAVIOR.md).
# Validate after editing: cosmostrix --testconf
# Location: ~/.config/cosmostrix/config.toml (platform paths: --help)
# Catalogs: --list-scenes, --list-colors, --list-charsets

# -- Standard settings (defaults shown; uncomment to override) ------

# scene = "cinematic"        # built-in name OR a [scene-custom.<name>] block
# color = "energy-zen"       # built-in theme OR a [colors-custom.<name>] block
# charset = "zen"            # built-in preset (--list-charsets)
# color-bg = "black"         # or "default-background"
# intro = "logo"             # logo | cosmic | none
# intro-color = "energy-zen" # intro override (default: brand EnergyZen, never the rain color)

# -- Overlay message ------------------------------------------------

# message         = "A masterpiece" # without border (matches -m)
# message-border  = "A masterpiece" # with border (matches -mb; wins if both present)
# msg-mode        = true            # false suppresses the overlay entirely (CLI -m/-mb still wins)
# msg-fill-style  = "engrave"       # reveal animation: typewriter | fade | words | slide | instant
#                                   # engrave | hologram | glitch | scorch | cascade | radar

# Message notes: max 200 characters; no message anywhere -> interactive
# mode shows the bordered default "Experience a masterpiece with
# cosmostrix v<version>"; benchmark mode never shows an overlay.

# -- Motion ----------------------------------------------------------

# fps = 60                    # 1-240 (default: dynamic, 60 or 144 on high-refresh)
# speed = 9                   # 1-100
# density = 0.75              # 0.01-5.0
# async-mode = true           # variable column speeds
# monolith-size = "normal"    # small | normal | large (monolith scene only)

# -- Behavior ---------------------------------------------------------

# glitch-level = "subtle"       # none | subtle | default | intense
# bold = 1                      # 0=off, 1=random, 2=all
# shading-mode = 1              # 0=random, 1=cinematic
#
# power-dragon = true           # adaptive throttle. With it ON, the HUD dsty: line shows
#                               # the EFFECTIVE (banded) density, not the configured one —
#                               # that is correct, not a bug. For the exact fixed value:
#                               # power-dragon = false (or --power-dragon false).
#
# crystal-dragon = false        # ambient palette drift (see docs/AMBIENT_SCHEDULER.md)
#
# crystal-dragon-secs = 60      # drift poll cadence: 0.0-86400.0, human forms
#                               # (60s, 1m, 1h30m); live-reloadable
#
# ambient-snapback-secs = 30    # how long a drift (or shortkey override) holds
#                               # before the ambient phase re-asserts; 86400
#                               # disables. Harmony: keep this UNDER
#                               # crystal-dragon-secs (timing guide:
#                               # docs/AMBIENT_SCHEDULER.md)

# -- Color tuning -----------------------------------------------------

# [color.tune]
# brightness = 1.0              # global 0.0-3.0
# saturation = 1.0              # 0.0-3.0 (0.0 = grayscale)
# head = 1.0                    # 0.0-3.0
# body = 1.0                    # 0.0-3.0
# tail = 1.0                    # 0.0-3.0

# -- Custom scenes ----------------------------------------------------
# Load with: cosmostrix --scene-custom <name>. A block is a COMPLETE
# profile: ALL seven dimensions are required —
#   color OR colors-custom — the palette pair (never both)
#   charset OR charset-custom — the glyph pair (never both)
#   rain, fps, speed, density, glitch-level
#   an incomplete block is a hard error at startup, live-reload and
#   --testconf.
#   rain = glyph|monolith|vortex|flux|lorenz|dragon|physarum|black_hole|aeolian|solar_flare|dna_helix|murmuration|quasar|neural
# REMOVED in v80.0.0-beta.2: base-scene inheritance (the rain field owns
# the style). bold/shading-mode/async-mode are top-level keys, not
# per-scene.

# [scene-custom.hacker-mode]
# rain = "glyph"
# color = "green"
# charset = "hacker"
# fps = 60
# speed = 28
# density = 1.2
# glitch-level = "intense"

# [scene-custom.cyberpunk_2077]
# rain = "monolith"                     # monolith streams for the megacity feel
# colors-custom = "cyberpunk_2077"      # see [colors-custom.cyberpunk_2077] below
# charset-custom = "cyberpunk_2077"     # see [charset-custom.cyberpunk_2077] below
# fps = 90
# speed = 12
# density = 0.90
# glitch-level = "none"

# [scene-custom.tron_legacy]
# rain = "flux"                         # flux field for the light-cycle grid
# colors-custom = "tron_legacy"
# charset-custom = "tron_legacy"
# fps = 75
# speed = 8
# density = 0.70
# glitch-level = "subtle"

# -- Custom palettes ----------------------------------------------------
# Reference from a scene-custom block via: colors-custom = <name>.
# Hex values MUST be quoted (unquoted # starts a TOML comment).
# rain stops: min 2, max 64 — 7 is the sweet spot (the OKLab engine
# expands all stops to 9 perceptual samples).

# [colors-custom.zen]
# bg = "#0a0a0a"
# rain = ["#1a0033", "#4d0080", "#9933ff", "#cc66ff", "#e6b3ff", "#f2ccff", "#ffffff"]

# [colors-custom.cyberpunk_2077]
# bg = "#0A0008"
# rain = ["#FFE100", "#FF6B00", "#FF0066", "#FF00CC", "#CC00FF", "#00FFFF", "#E0E0E0"]

# [colors-custom.tron_legacy]
# bg = "#02080C"
# rain = ["#002B4D", "#0066AA", "#00BBEE", "#22DDFF", "#88EEFF", "#CCF4FF", "#FFFFFF"]

# -- Custom charsets ----------------------------------------------------
# Reference from a scene-custom block via: charset-custom = <name>.
# Printable single-width glyphs only (max 256; wide/zero-width are
# skipped with a warning). Always quote the set (set = "[" works).

# [charset-custom.zen]
# set = "|"

# [charset-custom.quantum]
# set = "∀∃∄∅∈∉∋∌∏∑∫∂∆∇√∞≈≠≤≥±∓×÷⊕⊗⊘⊙⊚⊛⊜⊝⊞⊟⊠⊡⊢⊣⊤⊥⊦⊧⊨⊩⊪⊫⊬⊭⊮⊯"

# [charset-custom.cyberpunk_2077]
# set = "0123456789ABCDEF<>{}[]|=+*ｱｲｳｴｵﾊﾋﾌﾍﾎﾏ"

# [charset-custom.tron_legacy]
# set = "0123456789ABCDEF←→↑↓█▌▐░▒▓│─┤├┬┴┼"

# -- Ambient Phase Scheduler -------------------------------------------
# Time-of-day scene switches (config-only, live-reload on save,
# max 256 entries):
#   ambient.<HH-MM> = <scene-name>

# ambient.06-00 = "signal"
# ambient.12-00 = "monolith"
# ambient.20-00 = "cinematic"

# Combining crystal-dragon + ambient: when a drift fires while an
# ambient phase is active, the drift palette holds for
# ambient-snapback-secs, then the ambient scene re-asserts
# (snapback). Any configured value fires — a longer window just
# holds longer and delays the next drift. Harmony rule: keep
# ambient-snapback-secs < crystal-dragon-secs so each drift reverts
# before the next poll and the two systems take turns cleanly (both
# knobs are live-reload-able — tune the rhythm while watching the
# HUD). Do not want the interplay at all? Turn one of the two off.
#
# While ANY entry is active, the ambient scene owns the scene-family
# fields — edits to scene, color, charset, fps, speed, density or
# glitch-level in this file are no-ops until the schedule empties
# (comment out ALL ambient.HH-MM entries and save to lift the
# overlay). Still live while ambient runs: monolith-size, color-bg,
# bold, shading-mode, color.tune.*, power-dragon, crystal-dragon,
# crystal-dragon-secs, async-mode, the message keys,
# ambient-snapback-secs and the schedule itself.
# All RUNTIME SHORTKEYS (q/r/c/C/s/S/x/X/p/i/Up/Down) work during
# ambient and take control until the next phase boundary. The
# 'i' shortkey toggles the HUD metrics overlay (a real binding).
# '[' and ']' adjust density down/up. Full contract:
# docs/LIVE_RELOAD_BEHAVIOR.md sections 8 and 14.
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
///   - `dump_config_with_header()` → fingerprints the template body only
///     (labelled `template-fingerprint` so users don't expect it to match
///     `sha512sum` of the full file on disk, which includes header lines).
///   - `testconf::run()` → fingerprints the user's config file on disk
///     (matches `sha512sum` exactly).
///   - `testconf::run()` → also fingerprints the current built-in template
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
