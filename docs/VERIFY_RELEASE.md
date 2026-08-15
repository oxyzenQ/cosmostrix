<!-- SPDX-License-Identifier: GPL-3.0-only -->
<!-- Copyright (C) 2026 rezky_nightky (oxyzenQ) -->

# Verifying Release Artifacts

Every release ships **three checksum files** + **one GPG signature** per archive.
Verify at least one checksum + the GPG signature before trusting a downloaded binary.

## 1. GPG Signature Verification (recommended)

### Import the maintainer's public key

The key is available on two public keyservers:

```bash
# Ubuntu keyserver (primary)
gpg --keyserver keyserver.ubuntu.com --recv-keys 47A50AEF4B65AAC2

# openpgp.org (mirror)
gpg --keyserver keys.openpgp.org --recv-keys 47A50AEF4B65AAC2
```

**Key fingerprint**: `F532 4E09 67F1 04D5 8CE0  25F3 47A5 0AEF 4B65 AAC2`

**Key details**:
- Master key (ed25519): `47A50AEF4B65AAC2` — Certify + Sign, never expires
- Signing subkey (ed25519): `F8E50CFF84765C30` — Sign only, expires 2029-08-09
- UID: `Rezky Cahya Sahputra (cosmic dragon) <130107241+oxyzenQ@users.noreply.github.com>`

### Signing key expiry policy

The signing subkey (`F8E50CFF84765C30`) has a **1-year expiry cycle** for security hygiene — the master key generates a fresh subkey before the current one expires, and the new subkey is published to the keyservers.

**What happens when the signing subkey expires:**

1. **Signatures made before expiry remain cryptographically valid forever.** GPG verifies them against the public key at the time of signing — the expiry only prevents *new* signatures, not validation of old ones.
2. GPG may print `WARNING: signature key expired` alongside the `Good signature` line. **This is expected and safe.** The signature is still valid; the warning just means the subkey has passed its expiry date.
3. Import the latest public key from the keyserver to suppress the warning:

```bash
gpg --keyserver keys.openpgp.org --recv-keys 47A50AEF4B65AAC2
```

**If GPG refuses to verify at all** (rare, only with very old GPG versions):

Use checksum verification (section 2) as a fallback — it does not depend on GPG key state. All three hash families (SHA-512, BLAKE2b, SHAKE256) provide independent integrity verification.

**Renewal timeline**: the maintainer rotates the signing subkey at least 30 days before expiry. The updated public key appears on keyservers within minutes of renewal. CI monitors key expiry automatically (see `maintenance.yml`).

### Verify the signature

```bash
# Download the archive + its .asc signature from the GitHub Release page
# e.g. cosmostrix-v50.0.0-alpha.2-linux-amd64-v3-gnu.tar.gz
#      cosmostrix-v50.0.0-alpha.2-linux-amd64-v3-gnu.tar.gz.asc

gpg --verify cosmostrix-v50.0.0-alpha.2-linux-amd64-v3-gnu.tar.gz.asc
```

Expected output:

```
gpg: Good signature from "Rezky Cahya Sahputra (cosmic dragon) <130107241+oxyzenQ@users.noreply.github.com>"
gpg:                 aka "Rezky Cahya Sahputra (rezky_nightky) <with.rezky@gmail.com>"
```

A `Good signature` line confirms authenticity. The `WARNING: This key is not certified with a trusted signature` notice is normal for first-time imports — verify the fingerprint matches `F532 4E09 67F1 04D5 8CE0 25F3 47A5 0AEF 4B65 AAC2`.

## 2. Checksum Verification

Every archive ships three checksum files covering classical + post-quantum algorithms:

| File | Algorithm | Family | Quantum-safe? |
|------|-----------|--------|---------------|
| `*.sha512sum` | SHA-512 | SHA-2 | 256-bit (borderline) |
| `*.b2sum` | BLAKE2b-512 | BLAKE2 | 256-bit |
| `*.shake256` | SHAKE256 | SHA-3 XOF (NIST PQ) | 256-bit |

### How to verify

All three commands print `<filename>: OK` on success (or `FAILED` on mismatch):

```bash
# Classical (universal, every Linux has this)
sha512sum -c cosmostrix-v50.0.0-alpha.2-linux-amd64-v3-gnu.tar.gz.sha512sum

# Quantum-resistant — BLAKE2b (fastest, in coreutils)
b2sum -c cosmostrix-v50.0.0-alpha.2-linux-amd64-v3-gnu.tar.gz.b2sum

# Quantum-resistant — SHAKE256 (NIST PQ standard, via Python)
# openssl's -shake256 default output length varies by version/distro;
# Python hashlib.shake_256 is consistent (64 bytes = 128 hex chars)
COMPUTED=$(python3 -c "import hashlib; print(hashlib.shake_256(open('cosmostrix-v50.0.0-alpha.2-linux-amd64-v3-gnu.tar.gz','rb').read()).hexdigest(64))")
EXPECTED=$(awk '{print $1}' cosmostrix-v50.0.0-alpha.2-linux-amd64-v3-gnu.tar.gz.shake256)
[ "$COMPUTED" = "$EXPECTED" ] && echo "cosmostrix-v50.0.0-alpha.2-linux-amd64-v3-gnu.tar.gz: OK" || echo "FAILED"
```

Replace `v50.0.0-alpha.2` and `linux-amd64-v3-gnu` with the actual version and platform from the release page.

## 3. Why both GPG + checksums

- **GPG signature** proves the artifact was produced by the maintainer (authenticity).
- **Checksums** prove the artifact was not corrupted during download (integrity).
- **Three hash families** (SHA-2, BLAKE2, SHA-3) provide defense in depth — a future cryptanalytic break of any single family does not invalidate verification via the other two.

## Verification tools required

- `gpg` — GnuPG 2.x (preinstalled on virtually every Linux; on macOS: `brew install gnupg`)
- `sha512sum` — GNU coreutils (preinstalled on every Linux)
- `b2sum` — GNU coreutils ≥ 8.x (preinstalled on modern distros)
- `python3` — Python 3.6+ (for SHAKE256 verification; preinstalled on most systems)

All tools ship with Arch Linux, Debian, Ubuntu, Fedora, Alpine, and macOS (via Homebrew) by default — no extra install needed on most systems.
