// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! The `sig` command surface — the six-depth signature report
//! (night-signature-advanced-3, extended by night-netrunner-Z1).
//!
//! The owner runs sibling tools across the fleet (zejtron prints the
//! same depth shape), so `cosmostrix sig` renders the same verdict
//! ladder over cosmostrix's own provenance layers:
//!
//!   D0  runtime-identity — the masked manifest materializes, carries
//!       the marker and the identity shape.
//!   D1  embedded-marker — the masked authors layer materializes and
//!       binds to the manifest identity (coherent pair).
//!   D2  anchor-integrity — the embedded blake3 anchor recomputes from
//!       the materialized payload (the cryptographic seal).
//!   D3  binary-self-hash — blake3 over the running executable image.
//!   D4  engine-oracle — the binary renders a pinned synthetic sequence
//!       in-memory and matches the compile-time vector (night-frontier-Z1,
//!       Proof Moat 1c: the capability proof).
//!   D5  commit-signature — the commit the binary claims is GPG-
//!       signed by the pinned maintainer key, verified at build time
//!       against the repo's own pinned public key (night-netrunner-Z1,
//!       the zelynic night-solid-defense-2 lane ported; see
//!       [`sig_commit_sig`]). This is the row that binds the provenance
//!       chain to the release channel: with the maintainer key verified
//!       out-of-band and releases cut from signed tags, D5 closes the
//!       MITM hole where a swapped release-page binary replays its own
//!       claims — the chain runs key to tag to commit to binary with
//!       no silent channel left.
//!
//! Numbering note: these D0-D5 are the six VERIFICATION depths (the
//! zejtron-parity ladder extended by the capability depth in
//! night-frontier-Z1 and the commit-signature depth in
//! night-netrunner-Z1). The `-V` Depth-matrix line's d0:-d3: tokens are
//! the four STORAGE layers
//! (masked manifest, masked authors, FNV fingerprints, blake3 anchor)
//! — a different axis, established in night-signature-advanced-2 and
//! unchanged. The two numbering systems are documented here so a
//! reader never has to guess which axis a `D2` refers to.
//!
//! The first three depths are the `-V` Depth-matrix line rendered per
//! depth; they reuse `provenance_view()`'s single pass (the invariant
//! documented in `signature.rs` — the report shows exactly the bytes
//! the integrity folds verify). D3, D4 and D5 are exclusive to this
//! command: D3 reads the whole image, D4 runs the engine oracle, and
//! D5 reports the build-time commit-signature stamp — all three stay
//! off the `-V` path.
//!
//! # Honest limits of D3
//!
//! A self-hash cannot embed its own digest (the chicken-and-egg law of
//! hashing), so D3 is a FINGERPRINT, not a self-verifying seal: it is
//! the value to compare against a published release hash (or a
//! previously recorded local hash) to prove the exact image running.
//! `pass` means the image was read in full and hashed; the comparison
//! is the caller's. The docs and the report text below say this
//! plainly — no fake "verified against itself" claim anywhere.
//!
//! # Honest limits of D5
//!
//! D5 reports the BUILD's verdict, stamped by build/gpg.rs when the
//! binary was compiled: it proves the commit the binary was built
//! from carried the maintainer's signature at build time, nothing
//! more. It does not re-verify at runtime (the private key never
//! touched this machine, and a runtime keyring read would widen the
//! attack surface); it does not prove the tag the release was cut
//! from (that is the consumer's out-of-band `git tag -v` step); and
//! on a tree whose HEAD is unsigned it honestly FAILS — a signed
//! release channel is the owner's discipline, the row's job is to
//! refuse to lie about it.

use std::io::Read;

use crate::output::println_safe;

use super::signature::{CONTACT, identity_core, provenance_view};

/// The D5 commit-signature policy (night-netrunner-Z1): pinned
/// fingerprint and the honest verdict sentence for every stamp state.
use super::sig_commit_sig::commit_signature_verdict;

/// The build-time D5 stamp accessor (see `info::vcs_commit_sig`).
use super::info::vcs_commit_sig;

/// Local alias for the engine-oracle runner (Proof Moat 1c): the
/// bench-owned oracle is the one renderer of the pinned sequence, and
/// the depth matrix consumes its verdict.
use crate::bench::engine_oracle::run_engine_oracle;

/// Verdict of the depth ladder.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SigVerdict {
    /// Every depth passed.
    Authentic,
    /// At least one depth failed; the report names which.
    Tampered,
}

/// One rendered depth row: `D{n} <name>  {pass|FAIL} — {detail}`.
struct DepthRow {
    name: &'static str,
    passed: bool,
    detail: String,
}

/// blake3 over the running executable image, read in chunks (the
/// release artifact is ~10 MB; streaming keeps the allocation flat).
/// `Err` names the failure honestly (unreadable image, mid-read error)
/// and fails the depth instead of guessing.
fn binary_self_hash() -> Result<String, String> {
    let path =
        std::env::current_exe().map_err(|e| format!("cannot locate the running image: {e}"))?;
    let mut file =
        std::fs::File::open(&path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 64 * 1024];
    let mut total = 0usize;
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("read error on {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        total += n;
    }
    if total == 0 {
        return Err("image is empty".to_string());
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest.as_bytes() {
        hex.push_str(&format!("{byte:02x}"));
    }
    Ok(hex)
}

/// Run the six-depth ladder and render the full report block.
///
/// The text is stable by contract (tests pin its shape, like the `-V`
/// report): boxed header, six depth rows, an identity field block,
/// then the verdict. Exit code is the dispatcher's job (`sig` exits 2
/// on [`SigVerdict::Tampered`], the fatal-CLI code this tree already
/// uses), so the report itself stays side-effect free.
#[must_use]
pub(crate) fn sig_report() -> (String, SigVerdict) {
    let view = provenance_view();

    // D0: the manifest materialized and carries the identity shape.
    // The display carries the identity CORE (night-ux-orion-2): the
    // marker's tail minus the embedded year — the year belongs to the
    // frozen embedded claim, not to the handle the report shows.
    let identity = identity_core(&view.manifest)
        .map(str::to_string)
        .unwrap_or_default();
    let d0_shape_ok = !identity.is_empty();

    // D1: the authors layer materialized and binds to the identity.
    // The same shared core extraction the binding law uses: the
    // marker/year shape must survive, or the claim is incoherent.
    let binding_ok =
        identity_core(&view.manifest).is_some_and(|core| view.authors.starts_with(core));

    // D2: the cryptographic anchor (already recomputed in the same
    // single pass that produced the display bytes).
    let anchor_ok = view.verified;

    // D3: the running image's own blake3 fingerprint.
    let self_hash = binary_self_hash();

    // D4 (night-frontier-Z1, Proof Moat 1c): the embedded engine
    // oracle. Milliseconds, single-threaded, no terminal I/O: the
    // pinned 16-frame synthetic sequence renders in-memory and folds
    // against the compile-time vector. `Err` carries the recomputed
    // vs pinned divergence for the depth row.
    let engine = run_engine_oracle();

    // D5 (night-netrunner-Z1): the commit-signature binding. The
    // stamp is the build's own verified verdict over the claimed
    // commit (build/gpg.rs, verified against the pinned public key
    // in an isolated keyring); the policy — pinned fingerprint,
    // honest sentence for every state, fail-closed on the unknown —
    // lives in sig_commit_sig.rs.
    let commit_sha = super::info::build_commit_short().unwrap_or("unknown");
    let (commit_sig_ok, commit_sig_detail) = commit_signature_verdict(vcs_commit_sig(), commit_sha);

    let rows = [
        DepthRow {
            name: "runtime-identity",
            passed: d0_shape_ok,
            detail: identity.clone(),
        },
        DepthRow {
            name: "embedded-marker",
            passed: binding_ok,
            detail: "the masked authors layer binds to the manifest identity".to_string(),
        },
        DepthRow {
            name: "anchor-integrity",
            passed: anchor_ok,
            detail: format!("b3:{} recomputes from the masked payload", view.anchor_hex),
        },
        DepthRow {
            name: "binary-self-hash",
            passed: self_hash.is_ok(),
            detail: match &self_hash {
                Ok(hex) => format!("b3:{hex}"),
                Err(e) => format!("unavailable: {e}"),
            },
        },
        DepthRow {
            name: "engine-oracle",
            passed: engine.is_ok(),
            detail: match &engine {
                Ok(digest) => format!(
                    "b3:{} recomputes in-memory (16-frame pinned replay)",
                    crate::bench::frame_digest::hex32(digest)
                ),
                Err(divergence) => format!("pinned lane vector mismatch: {divergence}"),
            },
        },
        DepthRow {
            name: "commit-signature",
            passed: commit_sig_ok,
            detail: commit_sig_detail,
        },
    ];

    let depths_passed = rows.iter().filter(|r| r.passed).count();
    let verdict = if depths_passed == rows.len() {
        SigVerdict::Authentic
    } else {
        SigVerdict::Tampered
    };

    let mut text = String::new();
    text.push_str("\u{2501}\u{2501}\u{2501} cosmostrix signature \u{2014} depth matrix \u{2501}\u{2501}\u{2501}\n");
    for (i, row) in rows.iter().enumerate() {
        let status = if row.passed { "pass" } else { "FAIL" };
        text.push_str(&format!(
            "D{} {:<18} {} \u{2014} {}\n",
            i, row.name, status, row.detail
        ));
    }
    text.push('\n');
    text.push_str(&format!("identity    {identity}\n"));
    text.push_str(&format!("contact     {CONTACT}\n"));
    text.push_str(&format!(
        "fingerprint fp={:016x} (fnv1a64, dual-layer)\n",
        super::signature::manifest_fingerprint()
    ));
    text.push_str(&format!("anchor      b3:{}\n", view.anchor_hex));
    match crate::diagnostics::info::build_commit_short() {
        Some(commit) => text.push_str(&format!("commit      {commit}\n")),
        None => text.push_str("commit      unknown\n"),
    }
    text.push_str(&format!(
        "build-time  {}\n",
        option_env!("COSMOSTRIX_BUILD_TIME").unwrap_or("unknown")
    ));
    text.push('\n');
    text.push_str(&format!(
        "verdict: SIGNATURE: {} ({depths_passed}/{} depths verified)\n",
        if verdict == SigVerdict::Authentic {
            "AUTHENTIC"
        } else {
            "TAMPERED"
        },
        rows.len()
    ));
    text.push_str("note: the self-hash is a fingerprint to compare against a\n");
    text.push_str("published release hash; it cannot embed its own digest.\n");

    (text, verdict)
}

/// Print the report (the `sig` dispatch calls this, then exits 2 on
/// tamper so scripts can gate on the exit code).
///
/// night-signature-advanced-4 (owner directive): the banner line rides
/// the bold brand purple tier — the zelynic report-title canon
/// (`report.rs` paints every `━━━ TITLE ━━━` banner this way). The
/// gate is a real stdout TTY, exactly like `version_report`'s purple
/// header; the capability-aware openers additionally fold in the
/// NO_COLOR/CLICOLOR contract, so piped output keeps every byte plain.
pub(crate) fn print_sig_report() {
    let (text, verdict) = sig_report();
    // The report builder terminates its last line with a newline, and
    // println adds the final newline itself; printing the body
    // untrimmed would emit a blank line at the very end of the report.
    let text = text.trim_end_matches('\n');
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stdout());
    match text.split_once('\n') {
        Some((banner, rest)) => {
            if is_tty {
                println_safe!(
                    "{}{banner}{}",
                    crate::output::brand_bold_open(),
                    crate::output::reset()
                );
            } else {
                println_safe!("{banner}");
            }
            println_safe!("{rest}");
        }
        None => println_safe!("{text}"),
    }
    if verdict == SigVerdict::Tampered {
        // Flush the report before the hard exit: the verdict IS the
        // error surface, and a swallowed buffer would hide it.
        use std::io::Write;
        let _ = std::io::stdout().flush();
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pristine_build_reports_the_stamped_signature_truth() {
        let (text, verdict) = sig_report();
        // The ladder's honesty law (night-netrunner-Z1, the zelynic
        // night-solid-defense-2 precedent): the D5 row reports the
        // BUILD's own verified verdict, so a tree on an unsigned HEAD
        // reads FAIL "no GPG signature" (5/6) and a tree whose HEAD
        // is signed by the pinned maintainer key reads AUTHENTIC
        // 6/6 — both are the pristine truth for their build, and the
        // report must never claim otherwise.
        let stamp = super::info::vcs_commit_sig();
        let sig_verified = stamp
            .strip_prefix("verified:")
            .is_some_and(|fpr| fpr == super::sig_commit_sig::PINNED_MAINTAINER_KEY_FPR);
        if sig_verified {
            assert_eq!(verdict, SigVerdict::Authentic, "{text}");
            assert!(
                text.contains("SIGNATURE: AUTHENTIC (6/6 depths verified)"),
                "{text}"
            );
        } else {
            assert_eq!(verdict, SigVerdict::Tampered, "{text}");
            let d5 = text
                .lines()
                .find(|line| line.starts_with("D5 commit-signature"))
                .unwrap_or_else(|| panic!("the D5 row must exist:\n{text}"));
            assert!(
                d5.contains(" FAIL "),
                "an unstamped-truth build must fail D5 by name:\n{text}"
            );
        }
        for row in [
            "D0 runtime-identity",
            "D1 embedded-marker",
            "D2 anchor-integrity",
            "D3 binary-self-hash",
            "D4 engine-oracle",
            "D5 commit-signature",
        ] {
            assert!(text.contains(row), "missing depth row {row}:\n{text}");
        }
    }

    #[test]
    fn report_names_the_owner_identity() {
        let (text, _) = sig_report();
        // The Masterclass identity block: the yearless identity core
        // (the year stays in the frozen embedded claim, never in the
        // handle the report shows) plus the owner contact line.
        assert!(
            text.contains("identity    rezky_nightky (oxyzenQ)\n"),
            "identity field must render the yearless core:\n{text}"
        );
        assert!(
            !text.contains("identity    rezky_nightky (oxyzenQ) 2026"),
            "identity field must not leak the embedded year:\n{text}"
        );
        assert!(
            text.contains("contact     oxyzenQ@proton.me\n"),
            "contact field must carry the owner address:\n{text}"
        );
        assert!(
            text.contains("anchor      b3:"),
            "anchor field present:\n{text}"
        );
    }

    #[test]
    fn self_hash_hashes_the_test_image() {
        // In the test build the "running image" is the test harness
        // binary; the only honest invariant available here is that the
        // hash succeeds and is 64 lowercase hex chars (or fails with a
        // named error on exotic hosts — both are honest outcomes).
        match binary_self_hash() {
            Ok(hex) => {
                assert_eq!(hex.len(), 64);
                assert!(
                    hex.chars()
                        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
                );
            }
            Err(e) => assert!(!e.is_empty(), "failure must name a reason"),
        }
    }

    #[test]
    fn report_shape_is_stable() {
        let (text, verdict) = sig_report();
        assert!(text.starts_with("\u{2501}\u{2501}\u{2501} cosmostrix signature"));
        assert!(text.contains("verdict: SIGNATURE: "));
        assert!(text.contains("note: the self-hash is a fingerprint"));
        assert!(matches!(
            verdict,
            SigVerdict::Authentic | SigVerdict::Tampered
        ));
    }
}
