<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-diner-9 — sha2 0.11.0 takes the last Unchanged line: one function rewritten, identity proven three ways, and the wall-of-Unchanged reaches zero

Owner directive (2026-09-30): "gue approve ini sampai audit semua
deps done: kandidat berikutnya tinggal sha2 0.11 (sambil
berkeliling sekali per triwulan) — tinggal bilang kalau mau gue
eksekusi."

The directive is the quarterly-revisit clause from the NIGHT-dinner-5
audit policy firing for the first time: sha2 0.11 was the only HOLD
left on the board, held because the audit priced it as a
security-critical-path rework. This record is the execution, and it
lands under the same dragon boundary as every NIGHT migration since:
the skin may be upgraded, the heart is never edited — sha2 never
touches the frame path (it hashes config bytes only), so the proof
burden here is output identity of the hashes themselves, carried all
the way to GNU coreutils.

## Task state at start (2026-09-30)

| Item | Status at task start | Est. effort (audit table) |
|------|---------------------|---------------------------|
| notify 8 | DONE 2026-09-29 | — |
| signal-hook 0.4 | DONE 2026-09-30 (NIGHT-dinner-8) | — |
| rand 0.10 | DONE 2026-09-30 (NIGHT-dinner-8) | — |
| sha2 0.11 | HOLD (quarterly revisit) — this task | "Medium — 4 call sites, API rework, high risk" |

The Unchanged list at start: exactly two lines, both by design —
generic-array 0.14.7 (upstream exact-pin via crypto-common, leaves
only with sha2 0.11) and sha2 0.10.9 itself.

## The claim versus the compiler — the fifth corrected estimate

The 2026-09-02 audit table priced this migration as: "sha2 0.11 is a
major API rework: Digest trait restructured, `Sha512::new()` →
`Sha512::new_with_prefix()`, output API changed. Medium — update 4
call sites. High risk — a subtle hashing change could cause
live-reload to miss config changes."

Every particular of that is wrong, and the wrongness is now compiler
evidence, not opinion. cosmostrix's entire sha2 surface is five
sites in three files (two src, three test), all using the same
`Sha512::new()` → `update()` → `finalize()` shape:

- `src/config/configfile/configfile_dump.rs::sha512_hex` — the
  `--dump-config` / `--testconf` fingerprint; formats the digest
  with `format!("{:0128x}", finalize())`.
- `src/config/live_config_poll/mod.rs::hash_file_prefix` —
  live-reload change detection (750 ms poll on the watcher thread);
  converts the digest with `let [u8; 64] = finalize().into()`.
- `test/config/live_config_poll/tests.rs` — three sites, all on the
  `.into()` conversion path, including the hardcoded NIST FIPS 180-4
  vectors.

A twin probe crate (`sha2 = "=0.10.9"` vs `"=0.11.0"`, session-side
at `scripts/sha2-ab-check/`) compiling the exact repo call patterns
gave the verdict:

- `Sha512::new()` — survives (0.11 makes it a `const fn`).
- `Digest::update` / `Digest::finalize` — survive.
- `Into<[u8; 64]>` on the output — survives (hybrid-array keeps the
  impl generic-array had).
- `format!("{:0128x}", finalize())` — **the one real break**: digest
  0.11's `Output<Sha512>` is hybrid-array's `Array<u8, U64>`, and it
  does not implement `LowerHex` (generic-array did). E0277, twice,
  both on the formatting path.

So `new_with_prefix()` (which exists since 0.10.3 and replaces
nothing here), the "restructured Digest trait", the "four call
sites", and the "medium effort" were all phantom weight. The real
migration is one function.

## The migration — one function, one comment

`sha512_hex` now converts the digest to `[u8; 64]` first and
hex-encodes by hand (a 16-entry byte table, `String::with_capacity(128)`).
Output format is unchanged by construction: a SHA-512 digest is
exactly 64 bytes, so the old `{:0128x}` zero-padding never triggered
and the new path always emits exactly 128 lowercase hex chars — the
same string, byte for byte. `hash_file_prefix` needed zero source
changes; its only edit is the comment that said "GenericArray to
[u8; 64]" now saying hybrid-array, so the code reference doesn't rot.

The Cargo.toml pin moved `0.10` → `0.11` with the comment block
extended to record the 0.11 rationale (digest 0.11 / hybrid-array
stack, the LowerHex loss, the parity-proven output identity).

MSRV: sha2 0.11 requires 1.85 (edition 2024); the repo toolchain is
1.98.1 and rand 0.10 already carries the same floor, so no change.

## Identity proven three ways

"It's just SHA-512, the output is the standard" is an assumption,
and assumptions are what produced five consecutive overestimates in
this audit's history. The proof is layered:

1. **Twin parity probe** — the repo's exact call patterns (both
   conversion paths) on 0.10.9-with-old-formatting vs
   0.11.0-with-new-path, 10 vectors: NIST FIPS 180-4 (empty, "abc",
   the 112-byte two-block vector), block-boundary edges at 127/128/129
   bytes, a config-shaped TOML input, the 1 MB million-'a' NIST
   stress vector, and a chunked 3,333-byte feed mimicking
   `hash_file_prefix` partial reads. Bit-for-bit identical, archived
   session-side (`out_010.txt` vs `out_011.txt`, empty diff).
2. **In-suite ground truth** — `sha512_known_vectors` hardcodes the
   NIST FIPS 180-4 digests and passes on 0.11.0; full suite
   2983/2983 (2 ignored, unchanged), clippy `-D warnings` clean,
   fmt clean.
3. **End-to-end against the OS** — the migrated release binary's
   `--testconf` prints `file-sha512: 7036fcdd...4c16` for a probe
   config and GNU coreutils `sha512sum` prints the same digest for
   the same file, byte-for-byte. The fingerprint contract ("matches
   `sha512sum` exactly", per the testconf docs) holds on the new
   line.

## Graph consequence — the wall reaches zero

`cargo update -p sha2` delta:

- Removed: generic-array 0.14.7, version_check 0.9.5, cpufeatures
  0.2.17 (the duplicate; unified with rand's 0.3.1).
- Added: hybrid-array 0.4.15, const-oid 0.10.2.
- Updated: sha2 0.10.9 → 0.11.0, digest 0.10.7 → 0.11.3,
  crypto-common 0.1.7 → 0.2.2 (no longer exact-pinning anything),
  block-buffer 0.10.4 → 0.12.1.

The generic-array upstream exact-pin — the thing the 2026-09-02
audit told the owner to "UPDATE NOW" via a command that could never
succeed — is retired with the same commit that was its only escape
hatch. `cargo update --verbose` now prints zero Unchanged lines:
eight (pre-dinner-5) → four (notify, 2026-09-29) → two (signal-hook
and rand, 2026-09-30) → zero (sha2, 2026-09-30). deny.toml needs no
changes — no new duplicate versions entered the graph, and one
duplicate (cpufeatures) left.

Quarterly-round note (the "berkeliling sekali per triwulan" clause):
with the wall at zero, this session's round is the audit itself —
`cargo update --verbose` confirms nothing actionable exists, the
weekly maintenance cron owns in-range flow, and the next quarterly
revisit has nothing on the board unless upstream ships new majors
(proptest's rand 0.9 pin remains the only duplicate cluster, dev-only
and documented in deny.toml with its leave-condition).

## Dragon-heart check

sha2 is not in the frame path — no engine file imports it, no
render code calls it. Its two runtime surfaces are the watcher
thread's 750 ms poll (~100 µs of hashing per cycle) and
startup/diagnostic fingerprints (`--dump-config`, `--testconf`).
The dragon's heart never sees a byte of it. The heart-adjacent risk
was never the algorithm (SHA-512 is standardized) but the plumbing —
a false-negative digest change would make live-reload miss config
edits — and the three-way identity proof covers exactly that.

## Gates

- `cargo check --locked --all-targets` — clean.
- `cargo fmt --check` — clean.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`
  — clean.
- `cargo test --locked --release` — 2983 passed / 0 failed /
  2 ignored (baseline-identical).
- PTY live-reload proof: covered by the existing
  `scripts/harness/cli_config_stresstest.sh` 47/47 config-reload
  suite in CI (the digest change is identity-proven, so the reload
  mechanism that compares digests is proof-covered by the parity
  above).
- gate-keepers: 21/21 (this session's run).
- Post-commit A/B benchmark: recorded in
  [benchmark/bench-labs/night_dinner9/AB_REPORT.md](../../benchmark/bench-labs/night_dinner9/AB_REPORT.md).

## References

- `docs/DEPENDENCY_AUDIT.md` — sha2 row (DONE), generic-array section
  (RESOLVED), zero-line current state.
- `docs/SUPPLY_CHAIN.md` — sha2 row (0.11).
- [NIGHT_DINNER_5_DEPENDENCY_STRICTNESS.md](NIGHT_DINNER_5_DEPENDENCY_STRICTNESS.md)
  — the relax policy, the dragon boundary, the fourth-relaxation
  section, and the one-glance table.
- `docs/research/NIGHT_DINNER_8_MAJOR_BUMP_BACKLOG.md` — the prior
  session (signal-hook 0.4 + rand 0.10) and the corrected-estimate
  pile this record extends.
- Session-side artifacts: `scripts/sha2-ab-check/` (twin probes,
  archived outputs).
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
