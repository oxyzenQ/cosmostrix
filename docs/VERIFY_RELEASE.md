<!-- SPDX-License-Identifier: GPL-3.0-only -->
<!-- Copyright (C) 2026 rezky_nightky (oxyzenQ) -->

# Verifying Release Artifacts

Every release ships **three** checksum files per archive, covering
classical + post-quantum algorithms. Verify at least one before
trusting a downloaded binary.

## Checksum files

| File | Algorithm | Family | Quantum-safe? |
|------|-----------|--------|---------------|
| `*.sha512sum` | SHA-512 | SHA-2 | 256-bit (borderline) |
| `*.b2sum` | BLAKE2b-512 | BLAKE2 | 256-bit |
| `*.shake256` | SHAKE256 | SHA-3 XOF (NIST PQ) | 256-bit |

## How to verify

All three commands print `<filename>: OK` on success (or `FAILED` on
mismatch). No manual hash comparison needed.

```bash
# Classical (universal, every Linux has this)
sha512sum -c project-vX.Y.Z-linux-amd64-gnu.tar.gz.sha512sum

# Quantum-resistant — BLAKE2b (fastest, in coreutils)
b2sum -c project-vX.Y.Z-linux-amd64-gnu.tar.gz.b2sum

# Quantum-resistant — SHAKE256 (NIST PQ standard, via Python)
# openssl's -shake256 default output length varies by version/distro;
# Python hashlib.shake_256 is consistent (64 bytes = 128 hex chars)
COMPUTED=$(python3 -c "import hashlib; print(hashlib.shake_256(open('project-vX.Y.Z-linux-amd64-gnu.tar.gz','rb').read()).hexdigest(64))")
EXPECTED=$(awk '{print $1}' project-vX.Y.Z-linux-amd64-gnu.tar.gz.shake256)
[ "$COMPUTED" = "$EXPECTED" ] && echo "project-vX.Y.Z-linux-amd64-gnu.tar.gz: OK" || echo "FAILED"
```

Replace `project-vX.Y.Z-linux-amd64-gnu` with the actual archive name
(e.g. `cosmostrix-v10.0.1-linux-amd64-musl` or
`cosmostrix-v10.0.1-linux-amd64-v3-gnu`).

## Why three algorithms

Defense in depth across three independent hash families
(SHA-2, BLAKE2, SHA-3 XOF). A future cryptanalytic break of any
single family does not invalidate verification via the other two.

## Verification tools required

- `sha512sum` — GNU coreutils (preinstalled on every Linux)
- `b2sum` — GNU coreutils ≥ 8.x (preinstalled on modern distros)
- `openssl` — OpenSSL 1.1.1+ (preinstalled on virtually every Linux)

All three tools ship with Arch Linux, Debian, Ubuntu, Fedora, Alpine,
and macOS by default — no extra install needed.
