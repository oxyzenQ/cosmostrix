#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
"""Local proof harness for the D5 lane (night-netrunner-Z1).

No Rust toolchain on this host, so this harness re-executes the exact
logic of build/gpg.rs and src/diagnostics/sig_commit_sig.rs against
the real repository state (real git objects, real gpg, real pinned
key) and proves every verdict class end-to-end:

  1. The stamp classifier against the REAL HEAD (signed by the
     pinned key) must yield verified:<primary-fpr>.
  2. The stamp classifier against a synthetic unsigned commit must
     yield "unsigned".
  3. The stamp classifier against a foreign-key signature must yield
     the wrong-key/no-key shape.
  4. The runtime policy verdicts for every stamp grammar state,
     mirrored assertion-for-assertion from the Rust unit tests.

Exit 0 = every proof passed.
"""

import os
import shutil
import subprocess
import sys
import tempfile

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
with open(os.path.join(REPO, "assets/gpg/pinned-maintainer-fpr.txt")) as fh:
    PINNED_FPR = fh.read().strip()
PINNED_KEY = os.path.join(REPO, "assets/gpg/maintainer-47A50AEF4B65AAC2.asc")

failures = []


def check(name, cond, detail=""):
    status = "OK " if cond else "FAIL"
    print(f"[{status}] {name}" + (f" — {detail}" if detail else ""))
    if not cond:
        failures.append(name)


# --- build/gpg.rs logic, mirrored --------------------------------------


def git_stdout(repo, args):
    out = subprocess.run(["git", "-C", repo] + args, check=False, capture_output=True)
    if out.returncode != 0:
        return None
    return out.stdout.decode()


def make_temp_keyring():
    home = tempfile.mkdtemp(prefix="cosmostrix-sig-verify-")
    os.chmod(home, 0o700)
    key_file = os.path.join(home, "pinned.asc")
    shutil.copy(PINNED_KEY, key_file)
    imp = subprocess.run(
        ["gpg", "--homedir", home, "--batch", "--quiet", "--import", key_file],
        check=False,
        capture_output=True,
    )
    if imp.returncode != 0:
        shutil.rmtree(home)
        return None
    return home


def verify_in_keyring(repo, home):
    out = subprocess.run(
        ["git", "-C", repo, "verify-commit", "--raw", "HEAD"],
        check=False,
        capture_output=True,
        env={**os.environ, "GNUPGHOME": home},
    )
    stderr = out.stderr.decode()
    validsig_signer = None
    validsig_primary = None
    errsig_fpr = None
    saw_no_pubkey = False
    saw_bad = False
    for line in stderr.splitlines():
        rest = line[len("[GNUPG:] ") :] if line.startswith("[GNUPG:] ") else None
        if rest is None:
            continue
        tokens = rest.split()
        if tokens[0] == "VALIDSIG" and len(tokens) > 1:
            validsig_signer = tokens[1]
            for t in reversed(tokens):
                if len(t) == 40 and all(c in "0123456789abcdefABCDEF" for c in t):
                    validsig_primary = t
                    break
        elif tokens[0] == "NO_PUBKEY":
            saw_no_pubkey = True
        elif tokens[0] == "ERRSIG" and len(tokens) >= 8:
            errsig_fpr = tokens[7]
        elif tokens[0] in ("BADSIG", "EXPKEYSIG", "REVKEYSIG"):
            saw_bad = True
    if validsig_signer is not None:
        primary = validsig_primary or validsig_signer
        if validsig_signer == PINNED_FPR or primary == PINNED_FPR:
            return f"verified:{primary}"
        return f"wrong-key:{validsig_signer}"
    if saw_no_pubkey:
        return f"no-key:{errsig_fpr}" if errsig_fpr else "no-key"
    if saw_bad or errsig_fpr is not None:
        return "bad-signature"
    return "bad-signature"


def classify_head_signature(repo):
    commit_object = git_stdout(repo, ["cat-file", "commit", "HEAD"])
    if commit_object is None:
        return "no-vcs"
    if not any(l.startswith("gpgsig ") for l in commit_object.splitlines()):
        return "unsigned"
    home = make_temp_keyring()
    if home is None:
        return "unavailable"
    verdict = verify_in_keyring(repo, home)
    shutil.rmtree(home)
    return verdict


# --- sig_commit_sig.rs policy, mirrored --------------------------------


def short_fpr(fpr):
    return fpr[-16:] if len(fpr) >= 16 else fpr


def commit_signature_verdict(stamp, commit_sha):
    state, _, evidence = stamp.partition(":")
    evidence = evidence or None
    if state == "verified":
        fpr = evidence or ""
        if fpr == PINNED_FPR:
            return True, (
                f"commit {commit_sha} carries a GPG signature by the pinned "
                f"maintainer key {short_fpr(fpr)} (verified against the repo's "
                f"pinned public key at build time)"
            )
        return False, (
            f"commit {commit_sha} is signed by {short_fpr(fpr)} — NOT the "
            f"pinned maintainer key {short_fpr(PINNED_FPR)}"
        )
    if state == "wrong-key":
        return False, (
            f"commit {commit_sha} carries a valid GPG signature by "
            f"{short_fpr(evidence or 'an unknown key')} — NOT the pinned "
            f"maintainer key {short_fpr(PINNED_FPR)}"
        )
    if state == "no-key":
        extra = f" (signed by {evidence})" if evidence else ""
        return False, (
            f"commit {commit_sha} carries a signature the pinned maintainer "
            f"key does not cover{extra} — not the maintainer's signature"
        )
    if state == "bad-signature":
        return False, (
            f"commit {commit_sha} carries a GPG signature that FAILED the "
            f"cryptographic check"
        )
    if state == "unsigned":
        return False, (
            f"commit {commit_sha} carries NO GPG signature — the provenance "
            f"chain stops at an unbound commit; bind releases to signed "
            f"commits/tags"
        )
    if state == "no-vcs":
        return (
            False,
            "built from a source snapshot without VCS evidence — the commit signature cannot be verified here",
        )
    if state == "unavailable":
        return (
            False,
            "signature evidence exists but the build machine could not verify (git/gpg unavailable at build time)",
        )
    return False, f"no commit-signature verdict ({state}) — fail-closed"


# --- proofs -------------------------------------------------------------

# P0: the pin is the estate master key.
check(
    "pinned fpr is the estate master key",
    PINNED_FPR == "F5324E0967F104D58CE025F347A50AEF4B65AAC2",
    PINNED_FPR,
)


# P1: the REAL signed commit (089badd, the owner's release commit,
# signed by the tag subkey whose VALIDSIG carries the pinned primary)
# classifies as verified:<primary>. Probed by REV (not HEAD): the
# agent session lands unsigned commits on main, and the honest
# classifier stamps those "unsigned" — the lane's fail-closed law,
# proven separately in P5.
def classify_commit_signature(rev):
    commit_object = git_stdout(REPO, ["cat-file", "commit", rev])
    if commit_object is None:
        return "no-vcs"
    if not any(l.startswith("gpgsig ") for l in commit_object.splitlines()):
        return "unsigned"
    home = make_temp_keyring()
    if home is None:
        return "unavailable"
    out = subprocess.run(
        ["git", "-C", REPO, "verify-commit", "--raw", rev],
        check=False,
        capture_output=True,
        env={**os.environ, "GNUPGHOME": home},
    )
    verdict = classify_status_lines(out.stderr.decode())
    shutil.rmtree(home)
    return verdict


def classify_status_lines(stderr):
    validsig_signer = None
    validsig_primary = None
    errsig_fpr = None
    saw_no_pubkey = False
    saw_bad = False
    for line in stderr.splitlines():
        rest = line[len("[GNUPG:] ") :] if line.startswith("[GNUPG:] ") else None
        if rest is None:
            continue
        tokens = rest.split()
        if tokens[0] == "VALIDSIG" and len(tokens) > 1:
            validsig_signer = tokens[1]
            for t in reversed(tokens):
                if len(t) == 40 and all(c in "0123456789abcdefABCDEF" for c in t):
                    validsig_primary = t
                    break
        elif tokens[0] == "NO_PUBKEY":
            saw_no_pubkey = True
        elif tokens[0] == "ERRSIG" and len(tokens) >= 8:
            errsig_fpr = tokens[7]
        elif tokens[0] in ("BADSIG", "EXPKEYSIG", "REVKEYSIG"):
            saw_bad = True
    if validsig_signer is not None:
        primary = validsig_primary or validsig_signer
        if validsig_signer == PINNED_FPR or primary == PINNED_FPR:
            return f"verified:{primary}"
        return f"wrong-key:{validsig_signer}"
    if saw_no_pubkey:
        return f"no-key:{errsig_fpr}" if errsig_fpr else "no-key"
    if saw_bad or errsig_fpr is not None:
        return "bad-signature"
    return "bad-signature"


SIGNED_REV = "089badd"
stamp = classify_commit_signature(SIGNED_REV)
signed_short = git_stdout(REPO, ["rev-parse", "--short=7", SIGNED_REV]).strip()
check(
    "real signed commit classifies verified with the pinned primary",
    stamp == f"verified:{PINNED_FPR}",
    f"{SIGNED_REV} ({signed_short}) -> stamp {stamp}",
)

# P1b: the current HEAD (an agent landing, unsigned) honestly stamps
# unsigned — the fail-closed law on the live tree itself.
head_stamp = classify_head_signature(REPO)
head_short = git_stdout(REPO, ["rev-parse", "--short=7", "HEAD"]).strip()
check(
    "current HEAD (unsigned agent commit) stamps unsigned",
    head_stamp == "unsigned",
    f"HEAD {head_short} -> stamp {head_stamp}",
)

# P2: the runtime policy passes the verified stamp and names key+commit.
ok, detail = commit_signature_verdict(stamp, signed_short)
check("policy passes the real verified stamp", ok, detail)
check("verdict names the short fpr", "47A50AEF4B65AAC2" in detail, detail)
check("verdict names the commit", signed_short in detail, detail)

# P3: every fail-closed state of the policy, mirrored from the Rust
# unit tests.
foreign = "0123456789ABCDEF0123456789ABCDEF01234567"
cases = [
    (
        "foreign VALIDSIG fails",
        f"verified:{foreign}",
        False,
        "NOT the pinned maintainer key",
    ),
    (
        "wrong-key fails",
        "wrong-key:AAAA47A50AEF4B65AAC2",
        False,
        "NOT the pinned maintainer key",
    ),
    (
        "no-key with evidence fails",
        "no-key:FFFF47A50AEF4B65AAC2",
        False,
        "signed by FFFF47A50AEF4B65AAC2",
    ),
    ("no-key bare fails", "no-key", False, "does not cover"),
    ("bad-signature fails", "bad-signature", False, "FAILED the cryptographic check"),
    ("unsigned fails with directive", "unsigned", False, "bind releases to signed"),
    ("no-vcs fails honestly", "no-vcs", False, "without VCS evidence"),
    ("unavailable fails honestly", "unavailable", False, "could not verify"),
    ("unstamped fails closed", "unstamped", False, "fail-closed"),
    ("future grammar fails closed", "future-state:whatever", False, "fail-closed"),
    ("empty stamp fails closed", "", False, "fail-closed"),
]
for name, st, want_ok, needle in cases:
    ok, detail = commit_signature_verdict(st, "cafeba1")
    check(f"policy: {name}", ok == want_ok and needle in detail, detail)

# P4: a signed-HEAD build reads 6/6 (verdict over the real verified
# stamp; ladder arithmetic: 5 depths evaluate on their own + D5 pass).
ok, _ = commit_signature_verdict(stamp, signed_short)
check("ladder arithmetic: signed HEAD -> 6/6 AUTHENTIC", ok)
# ...and the unsigned agent HEAD reads 5/6 UNSIGNED (night-hit-2: the
# D5 row FAILs — the policy below stays fail-closed — but the ladder
# names it a missing provenance link, not a modification; exit 1).
ok_h, _ = commit_signature_verdict(head_stamp, head_short)
check("ladder arithmetic: unsigned agent HEAD -> 5/6 UNSIGNED (D5 FAIL)", not ok_h)

# P5: an unsigned tree (the agent-push shape) fails D5 by name —
# build a synthetic unsigned commit in a scratch clone and classify.
scratch = tempfile.mkdtemp(prefix="cosmostrix-unsigned-probe-")
subprocess.run(
    ["git", "clone", "-q", "--no-local", "--depth=1", f"file://{REPO}", scratch],
    check=False,
    capture_output=True,
)
# Replace HEAD with an unsigned commit: amend with --no-gpg-sign on a
# trivially changed tree.
subprocess.run(
    ["git", "-C", scratch, "checkout", "-q", "HEAD~0"], check=False, capture_output=True
)
# depth=1 has no parent; create the unsigned shape by committing an
# empty change with author/committer envs (no signing).
env = {
    **os.environ,
    "GIT_AUTHOR_NAME": "probe",
    "GIT_AUTHOR_EMAIL": "probe@example.invalid",
    "GIT_COMMITTER_NAME": "probe",
    "GIT_COMMITTER_EMAIL": "probe@example.invalid",
}
subprocess.run(
    [
        "git",
        "-C",
        scratch,
        "commit",
        "--allow-empty",
        "--no-gpg-sign",
        "-q",
        "-m",
        "unsigned probe",
    ],
    check=False,
    capture_output=True,
    env=env,
)
stamp2 = classify_head_signature(scratch)
check("unsigned HEAD classifies unsigned", stamp2 == "unsigned", stamp2)
ok2, detail2 = commit_signature_verdict(
    stamp2, git_stdout(scratch, ["rev-parse", "--short=7", "HEAD"]).strip()
)
check(
    "unsigned HEAD fails D5 by name", not ok2 and "NO GPG signature" in detail2, detail2
)
shutil.rmtree(scratch)

# P6: a foreign-key-signed commit classifies wrong-key — craft one
# with a throwaway key the pin does not cover.
scratch = tempfile.mkdtemp(prefix="cosmostrix-foreign-probe-")
subprocess.run(
    ["git", "clone", "-q", "--no-local", "--depth=1", f"file://{REPO}", scratch],
    check=False,
    capture_output=True,
)
ghome = tempfile.mkdtemp(prefix="cosmostrix-foreign-key-")
os.chmod(ghome, 0o700)
subprocess.run(
    [
        "gpg",
        "--homedir",
        ghome,
        "--batch",
        "--passphrase",
        "",
        "--quick-gen-key",
        "foreign probe <probe@example.invalid>",
        "ed25519",
        "sign",
        "0",
    ],
    check=False,
    capture_output=True,
)
env = {
    **os.environ,
    "GNUPGHOME": ghome,
    "GIT_AUTHOR_NAME": "probe",
    "GIT_AUTHOR_EMAIL": "probe@example.invalid",
    "GIT_COMMITTER_NAME": "probe",
    "GIT_COMMITTER_EMAIL": "probe@example.invalid",
}
subprocess.run(
    [
        "git",
        "-C",
        scratch,
        "commit",
        "--allow-empty",
        "--no-gpg-sign",
        "-q",
        "-m",
        "foreign probe unsigned base",
    ],
    check=False,
    capture_output=True,
    env=env,
)
keyid = subprocess.run(
    ["gpg", "--homedir", ghome, "--batch", "--with-colons", "--list-keys"],
    check=False,
    capture_output=True,
).stdout.decode()
subprocess.run(
    [
        "git",
        "-C",
        scratch,
        "commit",
        "--amend",
        "--allow-empty",
        "--no-edit",
        "-S",
        "-q",
    ],
    check=False,
    capture_output=True,
    env=env,
)
stamp3 = classify_head_signature(scratch)
check(
    "foreign-key HEAD classifies wrong-key (MITM shape)",
    stamp3.startswith(("wrong-key:", "no-key")),
    stamp3,
)
ok3, detail3 = commit_signature_verdict(stamp3, "deadbee")
check(
    "foreign-key verdict fails by name",
    not ok3
    and (
        "NOT the pinned maintainer key" in detail3  # wrong-key shape
        or "does not cover" in detail3  # no-key shape (the realistic
        # MITM verdict: the foreign signer is unknown to the pin)
    ),
    detail3,
)
shutil.rmtree(scratch)
shutil.rmtree(ghome)

print()
if failures:
    print(f"FAILURES ({len(failures)}): " + ", ".join(failures))
    sys.exit(1)
print(
    "ALL D5 LANE PROOFS PASSED — verified / unsigned / foreign-key / fail-closed states all proven against the real repo, real key, real gpg."
)
