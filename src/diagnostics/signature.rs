// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! Provenance depth matrix — the embedded build-signature layers.
//!
//! night-signature-1: a single plaintext signature line is a one-layer
//! defense — trivial to strip or overwrite with a rebrand while keeping
//! the rest of the build. This module layers the same ownership claim at
//! three depths so the claim survives independent tampering of any one
//! layer, and so the `-V/--version` surface can *prove* internally that
//! the layers are intact:
//!
//!   D0  `COSMIC_DRAGON_SIGNATURE` — plaintext manifest, deliberately
//!       grep-able. Discoverable with `strings(1)`:
//!
//!       ```text
//!       strings ./cosmostrix | grep "rezky_nightky"
//!       ```
//!
//!       Its job is attribution: any redistributed binary (legitimate or
//!       stolen) carries the owner's mark in the artifact itself.
//!
//!   D1  `DEPTH_MATRIX_SHARDS` — the owner identity tuple
//!       "rezky_nightky (oxyzenQ) 2026" stored as fragments that only
//!       assemble at runtime. No single shard is the identity, so a
//!       rebrand attempt that only rewrites the D0 line still leaves D1
//!       reconstructing the original owner in memory.
//!
//!   D2  `DEPTH_MATRIX_FINGERPRINT` — an FNV-1a 64-bit digest over D0 +
//!       all D1 shards, evaluated as a `const` at compile time and
//!       recomputed at runtime. The two must agree byte-for-byte; a
//!       patched binary (stripped D0, edited shard, or edited digest)
//!       makes the runtime recompute diverge from the embedded constant
//!       and `-V` reports the divergence honestly instead of crashing.
//!
//! The `-V/--version` report (see `info::version_report`) prints the D0
//! manifest plus a `Depth-matrix:` line carrying the fingerprint and the
//! integrity status. Both lines are rendered by `provenance_report()`,
//! which performs ONE opaque read of the constants and feeds that single
//! read to both the display and the runtime fold. That binding is the
//! load-bearing detail: the compiler may clone a referenced constant into
//! several `.rodata` copies across consumer functions, so a display site
//! and a fold site in different functions can end up reading DIFFERENT
//! copies — an earlier draft had exactly that hole, found by an
//! end-to-end tamper probe (patching the display copy alone showed a
//! forged identity next to `status=verified`). With the single-read
//! binding, whatever bytes the report displays are exactly the bytes the
//! fingerprint folds: any patch flips `status=tampered` on the same
//! screen. The runtime fold also anchors the constants against linker
//! dead-code elimination.
//!
//! # Threat model (honest limits)
//!
//! This is provenance marking and tamper *evidence*, not DRM. A
//! determined attacker with a compiler can rebuild from modified source
//! without these constants, or locate and rewrite both the shards and
//! the embedded digest. The layered design raises the cost of a lazy
//! rebrand (string-swap theft) from trivial to multi-site consistent
//! patching, and gives the owner a mechanical authenticity check:
//! a genuine build always prints `status=verified`. Legal ownership
//! claims live in NOTICE and TRADEMARK.md; this module is the in-binary
//! technical witness, and the two are complementary by design.
//!
//! # Why FNV-1a
//!
//! Const-evaluable in stable Rust (no dependencies, no `for` loops in
//! `const fn`), trivially reproducible by external audit scripts, and
//! collision behavior is irrelevant here — the digest binds two short
//! constant strings compiled together, not adversarial input. Not a
//! cryptographic primitive and not claimed as one.

// --- Layer D0: plaintext manifest ------------------------------------

/// Embedded build signature (layer D0).
///
/// Marked `pub(crate)` so external tooling (e.g. FFI probes, binary diff
/// scripts, supply-chain scanners) can grep for it both in source and
/// in the produced artifact. Referenced by `version_report()` so the
/// linker keeps it in the final binary even under aggressive dead-code
/// elimination.
///
/// Year-stamped depth form (night-signature-1): the manifest now carries
/// the full owner identity tuple including the build year, matching the
/// copyright line of the `-V` report.
pub(crate) const COSMIC_DRAGON_SIGNATURE: &str =
    "Cosmic Dragon — Official Build by rezky_nightky (oxyzenQ) 2026";

// --- Layer D1: identity shards ---------------------------------------

/// Owner identity fragments (layer D1).
///
/// Concatenated in order they reconstruct exactly
/// "rezky_nightky (oxyzenQ) 2026". Stored as fragments so the identity
/// does not exist as a single greppable string beyond the D0 manifest;
/// the D2 fingerprint and the test oracle assemble them at runtime.
/// Fragment order and content are part of the D2 digest — editing any
/// shard breaks the fingerprint check.
const DEPTH_MATRIX_SHARDS: [&str; 6] = ["rezky_", "nightky", " (oxy", "zenQ)", " 20", "26"];

// --- Layer D2: fingerprint -------------------------------------------

/// FNV-1a 64-bit offset basis.
const FNV1A64_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
/// FNV-1a 64-bit prime.
const FNV1A64_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Fold one byte slice into a running FNV-1a 64-bit digest.
///
/// Written as a `const fn` with a `while` loop because `for` loops are
/// not permitted in const evaluation on stable Rust. Used by both the
/// compile-time fingerprint (`DEPTH_MATRIX_FINGERPRINT`) and the runtime
/// recompute, so the two paths share one implementation and cannot
/// drift.
const fn fnv1a64_fold(mut hash: u64, bytes: &[u8]) -> u64 {
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(FNV1A64_PRIME);
        i += 1;
    }
    hash
}

/// Digest the D0 manifest plus every D1 shard, in order.
///
/// Shared shape for the const fingerprint and the runtime recompute;
/// keeping the input order fixed (D0 first, shards in declaration order)
/// is part of the contract — reordering inputs changes the digest.
const fn fnv1a64_provenance_of(signature: &str, shards: &[&str]) -> u64 {
    let mut hash = fnv1a64_fold(FNV1A64_OFFSET_BASIS, signature.as_bytes());
    let mut s = 0;
    while s < shards.len() {
        hash = fnv1a64_fold(hash, shards[s].as_bytes());
        s += 1;
    }
    hash
}

/// Embedded depth-matrix fingerprint (layer D2).
///
/// Compile-time digest over `COSMIC_DRAGON_SIGNATURE` followed by every
/// `DEPTH_MATRIX_SHARDS` fragment. Evaluated by the compiler into the
/// binary as a 64-bit constant; `depth_matrix_runtime_fingerprint()`
/// recomputes the identical digest from the same constants at runtime.
const DEPTH_MATRIX_FINGERPRINT: u64 =
    fnv1a64_provenance_of(COSMIC_DRAGON_SIGNATURE, &DEPTH_MATRIX_SHARDS);

// --- Version-report surface ------------------------------------------

/// Render the provenance block for the `-V/--version` report.
///
/// Emits two lines (the block is inserted verbatim into the report body):
///
/// ```text
/// Signature: Cosmic Dragon — Official Build by rezky_nightky (oxyzenQ) 2026
/// Depth-matrix: fp=<16 hex> status=verified layers=d0:plain,d1:shards,d2:fingerprint
/// ```
///
/// # Single-read binding invariant
///
/// The manifest and the shard slice are read exactly once, through
/// `black_box`, and the SAME locals feed both the `Signature:` display
/// and the runtime fold. Display and fingerprint therefore can never
/// disagree about which bytes they saw: a patched copy shows a forged
/// manifest AND `status=tampered` together, never a forged identity
/// alongside a green status. See the module docs for why the read must
/// not be split across functions.
///
/// `status` flips to `tampered` when the runtime fold diverges from the
/// embedded constant. The lines are plain text (no ANSI) so they survive
/// pipes and log capture like the rest of the report body. The format is
/// a stable surface — audit scripts may grep `fp=` and `status=`.
#[must_use]
pub(crate) fn provenance_report() -> String {
    use std::hint::black_box;

    // Single opaque read (see the invariant above): LLVM cannot prove
    // these are the compile-time constants, so the fold below runs for
    // real and reads whatever the artifact actually contains.
    let manifest: &str = black_box(COSMIC_DRAGON_SIGNATURE);
    let shards: &[&str] = black_box(DEPTH_MATRIX_SHARDS.as_slice());

    let runtime_fp = fnv1a64_provenance_of(manifest, shards);
    let status = if runtime_fp == DEPTH_MATRIX_FINGERPRINT {
        "verified"
    } else {
        "tampered"
    };
    format!(
        "Signature: {manifest}\n\
         Depth-matrix: fp={DEPTH_MATRIX_FINGERPRINT:016x} status={status} layers=d0:plain,d1:shards,d2:fingerprint"
    )
}

// --- Test-only helpers -------------------------------------------------

/// Assemble the owner identity from the D1 shards (test-side oracle).
///
/// Reconstructs "rezky_nightky (oxyzenQ) 2026" at runtime so tests can
/// prove the shard sequence is correct and that the D0 manifest carries
/// the same identity. Production never calls it — the identity is
/// surfaced through the D0 manifest line, while the shards stay
/// reachable in release binaries through the D2 runtime fingerprint,
/// which folds them directly. `#[cfg(test)]` keeps the accessor out of
/// release artifacts and satisfies the dead-code gate honestly: the
/// function has no production caller by design, not by omission.
#[cfg(test)]
#[must_use]
pub(crate) fn depth_matrix() -> String {
    let mut identity = String::new();
    for shard in &DEPTH_MATRIX_SHARDS {
        identity.push_str(shard);
    }
    identity
}

/// Runtime recomputation of the D2 fingerprint over D0 + D1 shards
/// (test-side oracle).
///
/// On a pristine build this equals `DEPTH_MATRIX_FINGERPRINT` exactly
/// (same fold function, same input order, same constants). Test-only:
/// production reads the fingerprint through `provenance_report()`, whose
/// single-read binding folds and displays the same bytes. `#[cfg(test)]`
/// keeps the accessor out of release artifacts and satisfies the
/// dead-code gate honestly.
///
/// # The black_box barrier (night-signature-1 hardening)
///
/// `fnv1a64_provenance_of` is a `const fn` called with all-constant
/// arguments, so an optimizing compiler would constant-fold this entire
/// call back into the embedded digest — the fold would never read the
/// binary's string bytes, and a patched artifact would still report
/// `verified`. That flaw was found by an end-to-end tamper probe (a
/// single byte flipped in the D0 manifest of the built binary kept the
/// status green) and is fixed with `std::hint::black_box`: the inputs
/// are passed through an optimizer-opaque barrier so the fold genuinely
/// reads the artifact's `.rodata` bytes at runtime. Without this barrier
/// layer D2 would be theater, not evidence.
#[cfg(test)]
#[must_use]
pub(crate) fn depth_matrix_runtime_fingerprint() -> u64 {
    use std::hint::black_box;

    // black_box makes the pointer optimizer-opaque: LLVM cannot prove
    // the bytes are the compile-time constants, so the fold runs for
    // real and reads whatever the artifact actually contains.
    fnv1a64_provenance_of(
        black_box(COSMIC_DRAGON_SIGNATURE),
        black_box(DEPTH_MATRIX_SHARDS.as_slice()),
    )
}

/// Integrity check: embedded D2 digest vs runtime recomputation
/// (test-side oracle).
///
/// `true` on every genuine build. `false` means the binary's signature
/// constants no longer agree with the digest embedded at compile time —
/// the artifact was modified after compilation. Production surfaces the
/// same verdict as `status=` inside `provenance_report()`.
#[cfg(test)]
#[must_use]
pub(crate) fn depth_matrix_verified() -> bool {
    depth_matrix_runtime_fingerprint() == DEPTH_MATRIX_FINGERPRINT
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test oracle: the exact owner identity the shards must rebuild.
    /// Hardcoded here (not imported) so any accidental shard edit fails
    /// the test instead of silently passing against a moved target.
    const OWNER_IDENTITY_ORACLE: &str = "rezky_nightky (oxyzenQ) 2026";

    #[test]
    fn shards_assemble_to_owner_identity() {
        assert_eq!(depth_matrix(), OWNER_IDENTITY_ORACLE);
    }

    #[test]
    fn plaintext_manifest_carries_full_identity_and_year() {
        assert!(!COSMIC_DRAGON_SIGNATURE.is_empty());
        assert!(COSMIC_DRAGON_SIGNATURE.contains("Cosmic Dragon"));
        assert!(COSMIC_DRAGON_SIGNATURE.contains(OWNER_IDENTITY_ORACLE));
        assert!(COSMIC_DRAGON_SIGNATURE.contains("2026"));
    }

    #[test]
    fn no_single_shard_equals_the_identity() {
        // D1's reason to exist: the identity must not survive as one
        // fragment. Every shard must be a strict fragment of the whole.
        for shard in &DEPTH_MATRIX_SHARDS {
            assert_ne!(*shard, OWNER_IDENTITY_ORACLE);
        }
    }

    #[test]
    fn const_fingerprint_matches_runtime_recompute() {
        assert_eq!(
            depth_matrix_runtime_fingerprint(),
            DEPTH_MATRIX_FINGERPRINT,
            "pristine build must have matching const and runtime fingerprints"
        );
        assert!(depth_matrix_verified());
    }

    #[test]
    fn tampered_shard_or_manifest_breaks_the_fingerprint() {
        // Simulate post-compile patching: any single-byte divergence in
        // D0 or any shard must change the digest. Proves the fingerprint
        // actually binds the data it claims to protect.
        let tampered_sig = "Cosmic Dragon — Official Build by someone_else (oxyzenQ) 2026";
        let tampered_shards = ["rezky_", "nightky", " (oxy", "zenQ)", " 20", "27"];
        assert_ne!(
            fnv1a64_provenance_of(tampered_sig, &DEPTH_MATRIX_SHARDS),
            DEPTH_MATRIX_FINGERPRINT,
            "patching D0 must break the fingerprint"
        );
        assert_ne!(
            fnv1a64_provenance_of(COSMIC_DRAGON_SIGNATURE, &tampered_shards),
            DEPTH_MATRIX_FINGERPRINT,
            "patching any D1 shard must break the fingerprint"
        );
    }

    #[test]
    fn report_block_shape_is_stable() {
        let block = provenance_report();
        let mut lines = block.lines();
        let sig_line = lines.next().expect("Signature line present");
        let depth_line = lines.next().expect("Depth-matrix line present");
        assert!(lines.next().is_none(), "block must be exactly two lines");
        assert!(
            sig_line.starts_with("Signature: Cosmic Dragon "),
            "first line is the D0 manifest display: {sig_line}"
        );
        assert!(
            sig_line.ends_with(OWNER_IDENTITY_ORACLE),
            "manifest display must end with the owner identity: {sig_line}"
        );
        assert!(
            depth_line.starts_with("Depth-matrix: fp="),
            "depth line shape"
        );
        assert!(
            depth_line.contains(" status=verified "),
            "pristine build reports verified"
        );
        assert!(depth_line.contains("layers=d0:plain,d1:shards,d2:fingerprint"));
        // The fingerprint hex must be exactly 16 lowercase hex digits.
        let fp = depth_line
            .split(" status=")
            .next()
            .and_then(|head| head.strip_prefix("Depth-matrix: fp="))
            .expect("fp= token present");
        assert_eq!(fp.len(), 16);
        assert!(
            fp.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }

    #[test]
    fn provenance_block_binds_display_to_fold() {
        // The single-read invariant, checked from the outside: the
        // displayed manifest must be byte-identical to the manifest the
        // runtime fold reads. If these diverge, a patched display copy
        // could show a forged identity next to a green status.
        let block = provenance_report();
        let displayed = block
            .lines()
            .next()
            .and_then(|l| l.strip_prefix("Signature: "))
            .expect("manifest display present");
        assert_eq!(displayed, COSMIC_DRAGON_SIGNATURE);
        // And the folded bytes must equal the manifest on a pristine
        // build (status=verified already asserts this; re-derive it
        // explicitly so the binding is pinned by two assertions).
        assert!(block.contains("status=verified"));
    }
}
