// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! The D5 commit-signature policy (night-netrunner-Z1): the runtime
//! half of the GPG commit binding, pure over the build-time stamp so
//! every verdict class is unit-testable without a keyring. Port of
//! the zelynic night-solid-defense-2 lane (the estate's mature
//! reference), adapted to cosmostrix's diagnostics tree and env
//! naming (`COSMOSTRIX_VCS_COMMIT_SIG`).
//!
//! The gap this row closes (the supply-chain hole the owner's
//! adversarial review named): D0-D2 prove the ownership claim, D3
//! fingerprints the whole image, D4 seals the engine's pinned replay
//! — but the `commit` line the report carries was a CLAIM, not a
//! bound fact. A binary swapped on the release channel replays its
//! own claims and the ladder had no row that could disagree. D5
//! closes exactly that: build.rs verified the commit the binary was
//! built from against the PINNED maintainer public key (the repo's
//! own tracked copy under `assets/gpg/`, imported into an isolated
//! throwaway keyring — never the building user's), and this row
//! turns that verdict into the ladder's sixth depth. The out-of-band
//! twin: a consumer who verified the maintainer key (keyservers,
//! GitHub, docs/VERIFY_RELEASE.md) verifies the SIGNED TAG the
//! release was cut from, the tag names the commit, and D5 proves the
//! binary really was built from a commit signed by that same key —
//! the chain from key to tag to commit to binary closes with no
//! channel left to MITM silently.
//!
//! The pinned fingerprint is the SAME file build/gpg.rs reads —
//! `include_str!` on both sides, one source of truth, zero drift
//! between the verifier and the reporter.

/// The maintainer's pinned primary key fingerprint (ed25519,
/// `F5324E0967F104D58CE025F347A50AEF4B65AAC2`, long id
/// `47A50AEF4B65AAC2` — the identity that signs the release
/// archives and tags per docs/VERIFY_RELEASE.md). Single-sourced
/// from `assets/gpg/pinned-maintainer-fpr.txt` (no trailing newline:
/// the include is the value), itself inside the signed asset set
/// (assets/signature.b3sum), so even the pin is tamper-evident.
pub(crate) const PINNED_MAINTAINER_KEY_FPR: &str =
    include_str!("../../assets/gpg/pinned-maintainer-fpr.txt");

/// The D5 verdict: `(passed, detail)` for the sig ladder's
/// commit-signature row. Every state of the build-time stamp maps to
/// an honest sentence; only a verified match against the pinned
/// fingerprint passes — unsigned, foreign-key, broken, missing and
/// unknown all FAIL, naming their shape (the estate's
/// fails-by-name law).
#[must_use]
pub(crate) fn commit_signature_verdict(stamp: &str, commit_sha: &str) -> (bool, String) {
    let (state, evidence) = match stamp.split_once(':') {
        Some((state, evidence)) => (state, Some(evidence)),
        None => (stamp, None),
    };
    match state {
        "verified" => {
            let fpr = evidence.unwrap_or_default();
            if fpr == PINNED_MAINTAINER_KEY_FPR {
                (
                    true,
                    format!(
                        "commit {commit_sha} carries a GPG signature by the pinned \
                         maintainer key {} (verified against the repo's pinned public \
                         key at build time)",
                        short_fpr(fpr)
                    ),
                )
            } else {
                // A VALIDSIG that is not the pin: the signature is
                // real but the signer is not the maintainer — the
                // MITM shape, named by fingerprint.
                (
                    false,
                    format!(
                        "commit {commit_sha} is signed by {} — NOT the pinned \
                         maintainer key {}",
                        short_fpr(fpr),
                        short_fpr(PINNED_MAINTAINER_KEY_FPR)
                    ),
                )
            }
        }
        "wrong-key" => (
            false,
            format!(
                "commit {commit_sha} carries a valid GPG signature by {} — NOT the \
                 pinned maintainer key {}",
                short_fpr(evidence.unwrap_or("an unknown key")),
                short_fpr(PINNED_MAINTAINER_KEY_FPR)
            ),
        ),
        "no-key" => (
            false,
            format!(
                "commit {commit_sha} carries a signature the pinned maintainer key \
                 does not cover{} — not the maintainer's signature",
                evidence
                    .map(|fpr| format!(" (signed by {fpr})"))
                    .unwrap_or_default()
            ),
        ),
        "bad-signature" => (
            false,
            format!(
                "commit {commit_sha} carries a GPG signature that FAILED the \
                 cryptographic check"
            ),
        ),
        "unsigned" => (
            false,
            format!(
                "commit {commit_sha} carries NO GPG signature — the provenance \
                 chain stops at an unbound commit; bind releases to signed \
                 commits/tags"
            ),
        ),
        "no-vcs" => (
            false,
            "built from a source snapshot without VCS evidence — the commit \
             signature cannot be verified here"
                .to_string(),
        ),
        "unavailable" => (
            false,
            "signature evidence exists but the build machine could not verify \
             (git/gpg unavailable at build time)"
                .to_string(),
        ),
        // "unstamped" and anything a future stamp grammar emits but
        // this binary does not know: fail closed, never a silent
        // pass (the D4 missing-stamp law).
        other => (
            false,
            format!("no commit-signature verdict ({other}) — fail-closed"),
        ),
    }
}

/// The display shape for fingerprints: 16-hex long id when the input
/// carries at least that much, else the input as-is (an honest
/// shorter token beats a truncated lie).
fn short_fpr(fpr: &str) -> String {
    if fpr.len() >= 16 {
        fpr[fpr.len() - 16..].to_string()
    } else {
        fpr.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PINNED: &str = PINNED_MAINTAINER_KEY_FPR;

    #[test]
    fn pinned_fpr_is_the_estate_key() {
        // The pin file's exact value, pinned: the estate master key
        // (docs/VERIFY_RELEASE.md). If this ever changes, the docs,
        // the keyserver import command, and this test must move
        // together — a drifted pin silently breaks the whole D5
        // chain.
        assert_eq!(PINNED, "F5324E0967F104D58CE025F347A50AEF4B65AAC2");
    }

    #[test]
    fn verified_stamp_with_pinned_key_passes() {
        let (ok, detail) = commit_signature_verdict(&format!("verified:{PINNED}"), "089badd");
        assert!(ok, "{detail}");
        assert!(detail.contains("089badd"), "names the commit: {detail}");
        assert!(
            detail.contains("47A50AEF4B65AAC2"),
            "names the key: {detail}"
        );
        assert!(
            detail.contains("verified against the repo's pinned public key"),
            "names the verification lane: {detail}"
        );
    }

    #[test]
    fn verified_stamp_with_foreign_key_fails_by_name() {
        // The MITM shape: a real VALIDSIG by some other key.
        let foreign = "0123456789ABCDEF0123456789ABCDEF01234567";
        let (ok, detail) = commit_signature_verdict(&format!("verified:{foreign}"), "deadbee");
        assert!(!ok, "a foreign signer must fail: {detail}");
        assert!(detail.contains("NOT the pinned maintainer key"), "{detail}");
        assert!(
            detail.contains("47A50AEF4B65AAC2"),
            "names the pin: {detail}"
        );
    }

    #[test]
    fn wrong_key_stamp_fails_by_name() {
        let (ok, detail) = commit_signature_verdict("wrong-key:AAAA47A50AEF4B65AAC2", "cafeba1");
        assert!(!ok, "{detail}");
        assert!(detail.contains("NOT the pinned maintainer key"), "{detail}");
    }

    #[test]
    fn no_key_stamp_fails_naming_the_signer() {
        let (ok, detail) = commit_signature_verdict("no-key:FFFF47A50AEF4B65AAC2", "cafeba1");
        assert!(!ok, "{detail}");
        assert!(detail.contains("does not cover"), "{detail}");
        assert!(
            detail.contains("signed by FFFF47A50AEF4B65AAC2"),
            "{detail}"
        );
        let (ok, _) = commit_signature_verdict("no-key", "cafeba1");
        assert!(!ok, "evidenceless no-key also fails");
    }

    #[test]
    fn bad_signature_stamp_fails() {
        let (ok, detail) = commit_signature_verdict("bad-signature", "cafeba1");
        assert!(!ok, "{detail}");
        assert!(detail.contains("FAILED the cryptographic check"), "{detail}");
    }

    #[test]
    fn unsigned_stamp_fails_with_the_binding_directive() {
        // The most common honest state on main: HEAD unsigned (agent
        // pushes, rebases) — the row FAILS and says what to do.
        let (ok, detail) = commit_signature_verdict("unsigned", "6d71e6c");
        assert!(!ok, "{detail}");
        assert!(detail.contains("NO GPG signature"), "{detail}");
        assert!(detail.contains("bind releases to signed"), "{detail}");
    }

    #[test]
    fn no_vcs_and_unavailable_stamps_fail_honestly() {
        let (ok, detail) = commit_signature_verdict("no-vcs", "unknown");
        assert!(!ok, "{detail}");
        assert!(detail.contains("without VCS evidence"), "{detail}");
        let (ok, detail) = commit_signature_verdict("unavailable", "unknown");
        assert!(!ok, "{detail}");
        assert!(detail.contains("could not verify"), "{detail}");
    }

    #[test]
    fn unstamped_and_unknown_states_fail_closed() {
        // The default when build.rs never stamped (a stale cache, a
        // foreign build path) and any future grammar this binary does
        // not know: never a silent pass.
        for stamp in ["unstamped", "future-state:whatever", ""] {
            let (ok, detail) = commit_signature_verdict(stamp, "abc1234");
            assert!(!ok, "stamp {stamp:?} must fail closed: {detail}");
        }
    }

    #[test]
    fn short_fpr_shapes_fingerprints_honestly() {
        assert_eq!(short_fpr(PINNED), "47A50AEF4B65AAC2");
        assert_eq!(short_fpr("short"), "short");
        assert_eq!(short_fpr(""), "");
    }
}
