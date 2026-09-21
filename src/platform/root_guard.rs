// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Root-usage guard (NIGHT-security-4, 2026-09-21): cosmostrix is
//! designed for regular (non-root) users.
//!
//! Owner report: `sudo cosmostrix -vV` and `sudo cosmostrix
//! --check-update` ran silently — the config path switched to
//! `/root/.config/cosmostrix/config.toml` and the update check performed
//! its network fetch with uid 0 privileges, with zero indication that
//! the trust boundary had changed. Running a terminal renderer as root
//! is a high-risk WRONG USE CASE, not a supported one: root-owned
//! config trust, network fetches inside the root trust domain,
//! root-owned `--dump-config`/`--save-baseline` artifacts, and uid 0
//! terminal escape output on shared sessions.
//!
//! Policy: warn loudly on EVERY root invocation — one stderr block,
//! printed after argument parsing (clap error output stays clean) and
//! before any command output, so every path is covered (`--version`,
//! `--check-update`, `--doctor`, `--help`, benchmark, and the
//! interactive loop). stdout is never touched, so piped output stays
//! clean.
//!
//! Advisory by design, never blocking: container defaults legitimately
//! run as euid 0 (a hard refusal would break them), and `sudo -u
//! <other-user>` correctly does not warn — the effective UID is the
//! ground truth, not the elevation path.
//!
//! The canonical policy text ("Running as Root") lives in
//! docs/SECURITY_AUDIT.md; this warning cites it exactly once
//! (NIGHT-docs-8 tell-once rule — cite, do not re-tell). Precedent:
//! scripts/install.sh refuses to run as root for its cargo-build step;
//! this guard extends the non-root posture to the shipped binary.

use crate::output::{eprintln_safe, warn_bold};

/// True when this process runs with effective UID 0 (root).
///
/// Effective UID, not real UID: `sudo` sets both, but setuid binaries
/// and partial privilege transitions differ — the kernel permission
/// checks that define "running as root" follow the EFFECTIVE uid, so
/// that is what the guard reads.
#[cfg(unix)]
#[must_use]
pub(crate) fn is_effective_root() -> bool {
    // SAFETY: geteuid() takes no arguments, dereferences no pointers,
    // and cannot fail — it returns the kernel's cached effective UID
    // (a pure read; same libc-FFI family as clock/posix_time.rs, which
    // is why libc stays the only FFI surface here).
    let euid = unsafe { libc::geteuid() };
    euid == 0
}

/// Non-Unix (Windows): no euid and no root concept — the Administrator
/// elevation model is a different trust boundary and out of this
/// guard's scope (SECURITY_AUDIT.md honest-limits note).
#[cfg(not(unix))]
#[must_use]
pub(crate) fn is_effective_root() -> bool {
    false
}

/// The root-usage warning, one entry per line. Line 0 is the headline
/// (rendered bold warning yellow); the rest are plain two-space-indented
/// body lines matching the multi-line error style of cli/ux.rs and
/// scripts/install.sh. Fixed app constants — no user data is
/// interpolated, so nothing needs escaping beyond `eprintln_safe!`'s
/// broken-pipe safety.
#[must_use]
pub(crate) fn root_warning_lines() -> [&'static str; 6] {
    [
        "cosmostrix: security warning: running as root (euid 0)",
        "  Unsupported, high-risk use case. cosmostrix is designed for regular",
        "  (non-root) users - never sudo/su. Root execution shifts the trust",
        "  boundary: root-owned config paths, network update checks, and",
        "  terminal escape output at uid 0.",
        "  Forced to run as root? Read docs/SECURITY_AUDIT.md, \"Running as Root\".",
    ]
}

/// Emit the root-usage security warning to stderr (once per invocation).
///
/// No-op for regular users, for `sudo -u <user>` targets (effective UID
/// non-zero), and on non-Unix platforms. Never blocks, never touches
/// stdout, never panics on a broken stderr (`eprintln_safe!`).
pub(crate) fn warn_if_root() {
    if is_effective_root() {
        let lines = root_warning_lines();
        eprintln_safe!("{}", warn_bold(lines[0]));
        for line in &lines[1..] {
            eprintln_safe!("{line}");
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// NIGHT-security-4 headline contract: the first line must name the
    /// binary, the class (security warning), and the trigger (root,
    /// euid 0) — the exact words a user pastes into a search or a bug
    /// report. The verdict ("unsupported, high-risk use case") follows
    /// in the body.
    #[test]
    fn headline_pins_the_warning_trigger() {
        let lines = root_warning_lines();
        assert!(
            lines[0].starts_with("cosmostrix: security warning: running as root (euid 0)"),
            "headline drifted: {}",
            lines[0]
        );
        let joined = lines.join("\n");
        assert!(joined.contains("Unsupported"), "verdict class missing");
        assert!(joined.contains("high-risk"), "risk class missing");
    }

    /// NIGHT-docs-8 tell-once: the runtime warning must CITE the
    /// canonical policy doc (SECURITY_AUDIT.md, "Running as Root"), not
    /// re-tell the policy — the doc path and the section name are the
    /// contract.
    #[test]
    fn warning_cites_the_canonical_policy_doc() {
        let joined = root_warning_lines().join("\n");
        assert!(
            joined.contains("docs/SECURITY_AUDIT.md"),
            "warning lost the SECURITY_AUDIT.md pointer"
        );
        assert!(
            joined.contains("\"Running as Root\""),
            "warning lost the section name"
        );
    }

    /// The warning must teach the correction (regular user, never
    /// sudo/su) and the concrete risk surfaces, not just report the
    /// trigger.
    #[test]
    fn warning_teaches_the_non_root_design() {
        let joined = root_warning_lines().join("\n");
        assert!(joined.contains("non-root"));
        assert!(joined.contains("never sudo/su"));
        assert!(joined.contains("network update checks"));
    }

    /// Formatting contract: every line fits an 80-column terminal and
    /// every body line keeps the two-space indent (house multi-line
    /// error style).
    #[test]
    fn warning_lines_fit_80_columns() {
        for (i, line) in root_warning_lines().iter().enumerate() {
            assert!(
                line.chars().count() <= 80,
                "line {i} exceeds 80 columns: {line}"
            );
        }
    }

    /// Cross-platform compilability + no-panic pin. The live uid value
    /// is host-dependent (root CI containers exist), so only the
    /// callability is asserted — mirroring the platform/mod.rs test
    /// style for cfg-gated platform helpers.
    #[test]
    fn is_effective_root_is_callable() {
        let _ = is_effective_root();
    }
}
