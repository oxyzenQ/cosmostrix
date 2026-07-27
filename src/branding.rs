// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Branding constants — official build signature.
//!
//! `DRAGON_SIGNATURE` is embedded as a literal `&'static str` so it
//! survives codegen and is discoverable via `strings(1)` on the produced
//! binary. This lets users and supply-chain auditors confirm that a given
//! `cosmostrix` binary was produced from the official source tree by
//! rezky_nightky (oxyzenQ), distinguishing it from third-party rebuilds.
//!
//! Verification:
//!
//! ```text
//! strings ./cosmostrix | grep "Cosmic Dragon"
//! ```
//!
//! The constant is referenced by [`crate::info::version_report`] (the
//! `--version` / `-V` path), which guarantees the linker keeps it in the
//! final binary even under aggressive dead-code elimination.

/// Embedded build signature.
///
/// Marked `pub` so external tooling (e.g. FFI probes, binary diff
/// scripts, supply-chain scanners) can grep for it both in source and
/// in the produced artifact.
pub const DRAGON_SIGNATURE: &str = "Cosmic Dragon — Official Build by rezky_nightky (oxyzenQ)";

/// Return the dragon signature.
///
/// Thin accessor that exists so call sites have a stable function symbol
/// to reference. Calling this function from a reachable code path pins
/// `DRAGON_SIGNATURE` into the binary.
#[must_use]
pub fn dragon_signature() -> &'static str {
    DRAGON_SIGNATURE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_is_non_empty_and_stable() {
        assert!(!DRAGON_SIGNATURE.is_empty());
        assert!(DRAGON_SIGNATURE.contains("Cosmic Dragon"));
        assert!(DRAGON_SIGNATURE.contains("rezky_nightky"));
        assert!(DRAGON_SIGNATURE.contains("oxyzenQ"));
        assert_eq!(dragon_signature(), DRAGON_SIGNATURE);
    }
}
