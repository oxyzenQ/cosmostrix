// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Config file LOADING — extracted from `configfile.rs` (800-LOC file
//! cap) together with the startup-parse memo (NIGHT-hunter-22, wart F1).
//!
//! `configfile.rs` owns TEXT parsing (line → key/value + diagnostics);
//! this module owns the disk side: path resolution, the size-capped
//! read, the system-wide `/etc` fallback, and the memo that collapses
//! the startup's 9-11 loads into ONE parse.
//!
//! ## The F1 wart, quantified
//!
//! A normal startup called the loader 9 times (11 under `--verbose`):
//! config_apply (the canonical parse + strict validation), canonicalize,
//! main.rs's color / color-tune / rain-style / charset resolution (4),
//! build_cloud_cfg's ambient schedule + snapback (2), the verbose
//! startup block (2), verbose's color-provenance check (1), and the
//! event loop's initial last-applied map (1). Each call re-read the
//! file and re-parsed the text — and, worse, two sites passed
//! INCONSISTENT paths (the intro custom-palette re-read used the
//! default path while validation used `args.config`; see
//! `event_loop_intro.rs` — fixed in the same hunt).
//!
//! ## Memo contract
//!
//! - Keyed on the RESOLVED path (the override verbatim, or the default
//!   path when `None`) — two different overrides never alias; a path
//!   change re-parses.
//! - Bounded by the distinct paths actually requested: startup touches
//!   ONE (at most two — the default-path load plus the watcher-resolved
//!   path when the default file does not exist and the /etc fallback
//!   applies). Tests exercising many temp paths accumulate entries per
//!   unique path, which is the same order as the test files themselves.
//! - Startup phases share ONE coherent snapshot per path (the file is
//!   stable for the whole startup window; mid-run edits flow through
//!   the live-reload watcher, which reads via `read_config_capped` +
//!   `parse_config_text` on its own thread and never consults this
//!   memo).
//! - The post-startup consumers that want "the config as it was at
//!   startup" (the event loop's initial last-applied diff baseline,
//!   the intro's palette re-read) get the SAME parse validation used.
//! - A poisoned lock degrades to a fresh parse (correctness first).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::configfile::{default_config_file_path, parse_config_text, ParsedConfig};

/// Load config file and return a HashMap of key → value pairs.
/// Returns empty HashMap if file doesn't exist or can't be read.
/// Warns on stderr for unrecognized keys (likely typos).
///
/// Search order when no explicit path is given:
/// 1. `$XDG_CONFIG_HOME/cosmostrix/config.toml` (or `~/.config/cosmostrix/config.toml`)
/// 2. `/etc/cosmostrix/config.toml` (system-wide default, installed by AUR/package manager)
///
/// This means AUR users get a working default config out of the box —
/// the package installs `/etc/cosmostrix/config.toml`, and cosmostrix
/// reads it automatically if no user-level config exists.
///
/// NIGHT-hunter-22: the call is memoized per resolved path for the
/// process lifetime (see the module docs) — repeated startup loads
/// return the first coherent parse instead of re-reading the disk.
#[must_use]
pub(crate) fn load_config_file(path_override: Option<&Path>) -> HashMap<String, String> {
    load_config_file_full(path_override).values
}

/// Phase 5 closure (P4-8): load config file and return the FULL parse result
/// (including `malformed_lines` and `unknown_keys` vectors).
///
/// `load_config_file` discards these vectors (it only returns `values`).
/// Callers that need malformed/unknown detection (e.g. startup validation in
/// `config_apply.rs`) previously had to re-read + re-parse the file from disk
/// to recover them. This function eliminates the redundant disk read by
/// returning the full `ParsedConfig` in one pass.
///
/// Most callers should use `load_config_file` (which returns just the values
/// HashMap). Use this function only when you need the malformed/unknown vectors.
///
/// NIGHT-hunter-22: memoized per resolved path like `load_config_file` —
/// the diagnostics vectors are part of the same coherent snapshot.
#[must_use]
pub(crate) fn load_config_file_full(path_override: Option<&Path>) -> ParsedConfig {
    let path = path_override
        .map(Path::to_path_buf)
        .unwrap_or_else(default_config_file_path);
    if let Some(hit) = memo_get(&path) {
        return hit;
    }
    let parsed = parse_config_at(&path, path_override.is_none());
    memo_store(path, parsed.clone());
    parsed
}

/// Single-path disk parse with the system-wide fallback.
///
/// `allow_system_fallback` mirrors the historical contract: only a
/// DEFAULT-path load (no explicit `--config`) falls back to
/// `/etc/cosmostrix/config.toml` when the user path is unreadable —
/// an explicit override that fails reads as empty (the user asked for
/// that exact file).
fn parse_config_at(path: &Path, allow_system_fallback: bool) -> ParsedConfig {
    // S-master-3-v2: size-capped read — an oversized (runaway/malicious)
    // config in a whitelisted dir is treated as unreadable (defaults or
    // /etc fallback apply) instead of an unbounded memory read.
    let content = match crate::config_io::read_config_capped(path) {
        Ok(c) => c,
        Err(_) => {
            // Fallback: try system-wide config at /etc/cosmostrix/config.toml.
            if allow_system_fallback {
                let system_path = PathBuf::from("/etc/cosmostrix/config.toml");
                match crate::config_io::read_config_capped(&system_path) {
                    Ok(sys_content) => sys_content,
                    Err(_) => return ParsedConfig::default(),
                }
            } else {
                return ParsedConfig::default();
            }
        }
    };
    parse_config_text(&content)
}

/// Startup-parse memo (NIGHT-hunter-22, wart F1) — see the module docs
/// for the full contract. Keyed by resolved path; a poisoned or
/// contended lock degrades to a fresh parse (correctness first).
static STARTUP_CONFIG_MEMO: Mutex<Option<HashMap<PathBuf, ParsedConfig>>> = Mutex::new(None);

/// Return the memoized parse for `path`, if present.
fn memo_get(path: &Path) -> Option<ParsedConfig> {
    let guard = STARTUP_CONFIG_MEMO.lock().ok()?;
    guard.as_ref()?.get(path).cloned()
}

/// Store the parse for `path` (best-effort; a poisoned or contended
/// lock is skipped — the parse result is returned to the caller
/// regardless).
fn memo_store(path: PathBuf, parsed: ParsedConfig) {
    if let Ok(mut guard) = STARTUP_CONFIG_MEMO.lock() {
        guard.get_or_insert_with(HashMap::new).insert(path, parsed);
    }
}
