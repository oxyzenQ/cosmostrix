<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->

# asset signature checksums

night-dragon-maintenance-X1 (2026-11-01): every image under `assets/`
(the two logo plates, the 4-scene capture, the three donation qr
codes, and the animated webp demo loop) carries a recorded BLAKE3
digest, so a future swap, duplicate, or re-encoded fake of the logo
is detectable against the committed bytes. The digests were produced
with `b3sum 1.8.7` and cross-verified against an independent BLAKE3
implementation before landing. The png table landed first; the webp
row was added in a same-day follow-up pass after the owner flagged
the demo loop as in-scope too.

night-netrunner-Z1 (2026-11-23): the signed set grew the two D5 GPG
trust anchors under `assets/gpg/` — the maintainer public key
(`maintainer-47A50AEF4B65AAC2.asc`, the armored export
`build/gpg.rs` imports into an isolated throwaway keyring) and the
pinned primary fingerprint (`pinned-maintainer-fpr.txt`, the single
source of truth both the build-time verifier and the runtime
`sig_commit_sig.rs` policy read through `include_str!`). Not images,
but the estate law (zelynic's `signature-assets.md`) counts trust
anchors as artifacts: a swapped or re-keyed key file must fail by
name before it can vouch for anything, so their bytes belong in the
record. The digests match the zelynic sidecar byte-for-byte — the
pin is estate-wide, one key, one identity.

Tool: `cargo install b3sum` (the BLAKE3 team's canonical CLI). On a
host without b3sum, the project's own regenerator reproduces the
sidecar exactly (its pure-Python BLAKE3 mirror is vector-pinned to
the crate and proven byte-identical to b3sum against the tracked
sidecar):

```bash
python3 scripts/release/regenerate-signature-b3sum.py
```

Verify (the whole protocol — every line must print OK):

```bash
b3sum -c assets/signature.b3sum
```

Regenerate and compare against the table below — any mismatch means
the file on disk no longer matches the signed asset:

```bash
cd assets
b3sum $(find . \( -name '*.png' -o -name '*.webp' \) -o -path './gpg/*' | sort)
```

| file | bytes | blake3 |
|------|-------|--------|
| cosmostrix-4-scene.png | 184,815 | `65cd77593207b365f6c8f3fb00b70ffac77803918356b49f7873eec2fa6d93ed` |
| cosmostrix-original-logo-2026-old.png | 1,255,879 | `98b3862b7b7e0ed4dd286cd8dd6d63acec4ff1589c660ebf334a5722d3f135cd` |
| cosmostrix-original-logo-2026.png | 1,288,029 | `dd99531e39307c0ea0766ab28651bb64bb7f1f9ecbc18bb8746cc38c4795e4ab` |
| cosmostrix-video.webp | 9,217,878 | `554e86d755351c77d91c7e1d9a8fdd4c88760e174bc02b9bebd4eca24342751f` |
| gpg/maintainer-47A50AEF4B65AAC2.asc | 4,918 | `639b5d94603e14fa9022cad8e26efff7711bdaedd92f08723ffa96c0b2d90d77` |
| gpg/pinned-maintainer-fpr.txt | 40 | `dc6108d809afdbbb41756c37ef3ce3eec4b674ed1c8c225678c0103c1e4cfa3b` |
| qr/bitcoin.png | 575 | `09ec09fcdc4815e23373cb266bb120aa4e1af140fb9d598e117d05478a791b47` |
| qr/ethereum.png | 494 | `f0f3bf92d1730fe304f57f591ba1ed036449bb13731624e2e5b83c5c9896750b` |
| qr/solana.png | 595 | `bca56b0a454daf06ff2d57fe401719553e28733e73479de2652a0740a9e87d1a` |

The animated webp (`cosmostrix-video.webp`, the README demo loop) is
pinned on the same terms: it is as much a brand asset as the logo
plates, so a re-encoded or substituted loop is detectable against the
committed bytes too. One caveat travels with the key: re-exporting
the maintainer key with a new UID or a pushed-out expiry changes the
armored bytes and moves the `.asc` digest — a legitimate change, but
the commit message must say so, same as a re-encoded logo.

When an asset legitimately changes, refresh the matching row in the
same commit that replaces the file. The toolkit commands that
produced these assets live in [tools.md](tools.md).

<!-- COSMOSTRIX-DISCLAIMER -->
<!--
  Documentation Disclaimer — read before relying on any data point.

  This document may contain stale data, hardcoded counts, or outdated
  file paths and symbol names. Maintainers update source code but may
  forget to sync every doc — the project ships 80+ .md files and
  perfect sync is a known maintenance burden with diminishing returns.

  Source code (`src/**/*.rs`) is the single source of truth.
  Always cross-check against the actual `.rs` files before relying on
  any specific number (test count, LOC, FPS, ms timeout), file path,
  function name, or config key.

  If you find a discrepancy, please open a PR — the doc is wrong, not
  the source.
-->
