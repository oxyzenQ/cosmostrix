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

```bash
# Classical (universal, every Linux has this)
sha512sum -c project-vX.Y.Z-linux-amd64-gnu.tar.gz.sha512sum

# Quantum-resistant — BLAKE2b (fastest, in coreutils)
b2sum -c project-vX.Y.Z-linux-amd64-gnu.tar.gz.b2sum

# Quantum-resistant — SHAKE256 (NIST PQ standard, via openssl)
openssl dgst -shake256 project-vX.Y.Z-linux-amd64-gnu.tar.gz
# Compare hash with: cat project-vX.Y.Z-linux-amd64-gnu.tar.gz.shake256
```

Replace `project-vX.Y.Z-linux-amd64-gnu` with the actual archive name
(e.g. `cosmostrix-bin-v10.0.1-linux-amd64-v3-gnu` or
`cosmostrix-bin-v10.0.1-linux-amd64-musl`).

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
