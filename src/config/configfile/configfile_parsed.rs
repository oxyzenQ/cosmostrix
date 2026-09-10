// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `ParsedConfig` — the diagnostic record `parse_config_text` returns.
//!
//! Extracted from `configfile.rs` to keep that file under the 800-LOC
//! hard cap (see `src/RULES_LOC.md`), following the same pattern as
//! `configfile_dump.rs` / `configfile_promote.rs` / `configfile_load.rs`.
//! Re-exported from `configfile.rs` via `pub(crate) use` so every
//! historical `configfile::ParsedConfig` path keeps resolving.

use std::collections::HashMap;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct ParsedConfig {
    pub values: HashMap<String, String>,
    pub unknown_keys: Vec<String>,
    /// Non-empty, non-comment lines that do not match `key = value` syntax.
    ///
    /// Tracked so `--testconf` can report them as errors and `load_config_file`
    /// can warn on stderr. A line lands here when it has no `=` at all, or when
    /// either side of `=` is empty after trimming.
    pub malformed_lines: Vec<String>,
    /// keys auto-promoted from a nested section to root scope:
    /// `(original_nested_key, promoted_root_key)`. A top-level key
    /// written after a `[section]` header parses as nested; when the
    /// un-prefixed form is a known top-level key it is re-homed so
    /// top-level keys and blocks coexist without TOML scope lessons.
    pub promoted_keys: Vec<(String, String)>,
    /// NIGHT-depthtest-2: keys defined more than once in the same
    /// scope (`rain = "glyph"` twice under one `[scene-custom.x]`,
    /// or `fps` twice at root). Real TOML rejects duplicate keys
    /// with a hard parse error; cosmostrix's forgiving parser used
    /// to let the LAST writer silently win with zero signal. The
    /// parser now records every repeat here (the map still keeps
    /// last-wins so the value layer stays unchanged when validation
    /// is bypassed via COSMOSTRIX_SKIP_STARTUP_VALIDATION) and all
    /// three validation surfaces (startup, --testconf, the
    /// live-reload watcher) reject the file. This is the
    /// duplicate-name contract for the charset-custom,
    /// colors-custom and scene-custom namespaces: a duplicate
    /// `[colors-custom.zen]` header AND a duplicate
    /// `colors-custom.zen.rain` key both land in these vectors.
    pub duplicate_keys: Vec<String>,
    /// NIGHT-depthtest-2: `[section]` headers opened more than once
    /// (case-insensitive — the parser lowercases section names, so
    /// `[SCENE-CUSTOM.x]` after `[scene-custom.x]` is a repeat).
    /// Real TOML rejects table redefinition; the parser used to
    /// silently merge the blocks. The merge behavior is kept for
    /// the values map, but the repeat is recorded so the
    /// validation layers reject the file.
    pub duplicate_sections: Vec<String>,
    /// NIGHT-depthtest-2: disk-level read failure for an EXPLICIT
    /// `--config <path>` (file missing, unreadable, or over the size
    /// cap). `parse_config_text` never sets this — only
    /// `configfile_load::parse_config_at` does, and only when no
    /// fallback applies (the default-path load falls back to
    /// /etc and to empty-defaults by design, so a missing DEFAULT
    /// config stays silent — that is a normal first run). Surfaced
    /// by startup as a hard error: the user named a specific file,
    /// and silently running defaults on a typo'd path is the exact
    /// silent-failure class NIGHT-depthtest-2 hunts.
    pub read_error: Option<String>,
}
