#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
"""Provenance word generator for the depth-matrix signature
(night-signature-advanced-2 masked layers; night-signature-advanced-3
blake3 anchor).

Why this exists: the ownership claim must NOT exist as plaintext in the
compiled binary, because a thief can run strings(1) or rg over the
artifact, find the wording, and patch it out. This tool converts the two
provenance truths (the Signature manifest and the Copyright authors
string) into XOR-masked 64-bit words. The masks come from a splitmix64
stream keyed by per-layer seeds, so the constants in
src/diagnostics/signature.rs look like opaque checksum material ("nobody
knows what this is") and no contiguous ASCII fragment of the identity
survives into .rodata. At runtime the binary unmasks the words and
materializes the plaintext on the -V/--version surface only.

night-signature-advanced-3 adds the cryptographic anchor: a BLAKE3
digest over the framed clear payload of both layers (domain tag +
length-prefixed manifest + length-prefixed authors). The binary embeds
the anchor as an opaque [u8; 32] and recomputes the digest from the
SAME materialized bytes it displays (single-pass invariant), so any
patched word, patched anchor, or incoherent layer pair breaks
status=verified. BLAKE3 is implemented here in pure Python (spec-exact,
no external dependencies -- this script must run anywhere python3
runs) and self-checked against pinned official test vectors before the
anchor is emitted; the Rust side verifies the same bytes through the
blake3 crate, and the two implementations were cross-validated against
each other at integration time.

This file is the committed truth source for those constants: the identity
appears here ONCE by design, exactly like NOTICE and TRADEMARK.md carry
it as the legal layer. Rotating the identity (or the seeds) is one edit
plus one paste of the emitted Rust block.

The masking/unmasking algorithm is mirrored byte-exactly in
src/diagnostics/signature.rs (const fn splitmix64_mix + mask_for). If you
change one side, change both, and re-run this tool. The emitted block is
self-checked below: it unmasks its own words and asserts the round-trip
equals the input, so a paste-mistake or an algorithm drift fails here
before it can ship.

Usage:
    python3 scripts/release/generate-provenance-words.py
"""

# The provenance truths. Byte-level truth: UTF-8 (the em dash is three
# bytes, E2 80 94, and every layer below operates on bytes, not chars).
MANIFEST = "Cosmic Dragon — Official Build by rezky_nightky (oxyzenQ) 2026"
AUTHORS = "rezky_nightky (oxyzenQ) <with dot rezky at gmail dot com>"

# Per-layer seeds. Decoys with no external meaning; rotating them is a
# supported operation (re-run this tool, paste the new block). Both must
# differ so the two layers never produce identical .rodata bytes.
SEED_MANIFEST = 0x9D3A_7C1E_44B2_6F08
SEED_AUTHORS = 0x1F4E_6B8A_C27D_53A1

MASK64 = (1 << 64) - 1
GOLDEN = 0x9E3779B97F4A7C15
FNV_OFFSET = 0xCBF29CE484222325
FNV_PRIME = 0x100000001B3

# Domain-separation tag framing the anchor payload. Changing this string
# (or the framing below) changes every anchor value; rotate it together
# with a re-run of this tool and a fresh paste of the Rust block.
ANCHOR_DOMAIN = b"cosmostrix/depth-matrix/v1"


def splitmix64_mix(z: int) -> int:
    """Byte-exact mirror of signature.rs splitmix64_mix."""
    a = (z + GOLDEN) & MASK64
    b = ((a ^ (a >> 30)) * 0xBF58476D1CE4E5B9) & MASK64
    c = ((b ^ (b >> 27)) * 0x94D049BB133111EB) & MASK64
    return (c ^ (c >> 31)) & MASK64


def mask_for(seed: int, index: int) -> int:
    """Mask stream for word `index`: mix(seed + (index+1) * GOLDEN)."""
    return splitmix64_mix((seed + (index + 1) * GOLDEN) & MASK64)


def chunk_words(data: bytes) -> list:
    """Split bytes into little-endian u64 words, zero-padded."""
    words = []
    for i in range(0, len(data), 8):
        chunk = data[i : i + 8]
        words.append(int.from_bytes(chunk, "little"))
    return words


def mask_words(words: list, seed: int) -> list:
    return [w ^ mask_for(seed, i) for i, w in enumerate(words)]


def unmask_words(words: list, seed: int, byte_len: int) -> bytes:
    out = bytearray()
    for i, w in enumerate(words):
        out += ((w ^ mask_for(seed, i)) & MASK64).to_bytes(8, "little")
    return bytes(out[:byte_len])


def fnv1a64(data: bytes) -> int:
    h = FNV_OFFSET
    for byte in data:
        h ^= byte
        h = (h * FNV_PRIME) & MASK64
    return h


# --- BLAKE3 (pure Python, spec-exact) --------------------------------
#
# The anchor digest must be computable by this script with zero external
# dependencies, so BLAKE3 is implemented here from the specification
# (O'Connor, BLAKE3 spec, hash mode, 32-byte output). The Rust side uses
# the blake3 crate over the SAME framed payload; the two implementations
# were cross-validated byte-for-byte at integration time (see
# scripts/release/verify-release-build.sh history and the pinned test
# vectors in selftest_blake3, which assert before anything is emitted).

BLAKE3_OUT_LEN = 32
BLAKE3_BLOCK_LEN = 64
BLAKE3_CHUNK_LEN = 1024
BLAKE3_MASK32 = (1 << 32) - 1

# The BLAKE3 initial state is the SHA-256 IV.
BLAKE3_IV = [
    0x6A09E667,
    0xBB67AE85,
    0x3C6EF372,
    0xA54FF53A,
    0x510E527F,
    0x9B05688C,
    0x1F83D9AB,
    0x5BE0CD19,
]

BLAKE3_MSG_PERMUTATION = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8]

BLAKE3_CHUNK_START = 1 << 0
BLAKE3_CHUNK_END = 1 << 1
BLAKE3_PARENT = 1 << 2
BLAKE3_ROOT = 1 << 3


def _rotr32(x: int, n: int) -> int:
    return ((x >> n) | (x << (32 - n))) & BLAKE3_MASK32


def _g(state: list, a: int, b: int, c: int, d: int, mx: int, my: int) -> None:
    state[a] = (state[a] + state[b] + mx) & BLAKE3_MASK32
    state[d] = _rotr32(state[d] ^ state[a], 16)
    state[c] = (state[c] + state[d]) & BLAKE3_MASK32
    state[b] = _rotr32(state[b] ^ state[c], 12)
    state[a] = (state[a] + state[b] + my) & BLAKE3_MASK32
    state[d] = _rotr32(state[d] ^ state[a], 8)
    state[c] = (state[c] + state[d]) & BLAKE3_MASK32
    state[b] = _rotr32(state[b] ^ state[c], 7)


def _round(state: list, m: list) -> None:
    _g(state, 0, 4, 8, 12, m[0], m[1])
    _g(state, 1, 5, 9, 13, m[2], m[3])
    _g(state, 2, 6, 10, 14, m[4], m[5])
    _g(state, 3, 7, 11, 15, m[6], m[7])
    _g(state, 0, 5, 10, 15, m[8], m[9])
    _g(state, 1, 6, 11, 12, m[10], m[11])
    _g(state, 2, 7, 8, 13, m[12], m[13])
    _g(state, 3, 4, 9, 14, m[14], m[15])


def _compress(
    cv: list, block_words: list, counter: int, block_len: int, flags: int
) -> list:
    state = [
        cv[0],
        cv[1],
        cv[2],
        cv[3],
        cv[4],
        cv[5],
        cv[6],
        cv[7],
        BLAKE3_IV[0],
        BLAKE3_IV[1],
        BLAKE3_IV[2],
        BLAKE3_IV[3],
        counter & BLAKE3_MASK32,
        (counter >> 32) & BLAKE3_MASK32,
        block_len,
        flags,
    ]
    block = list(block_words)
    for i in range(7):
        _round(state, block)
        if i < 6:
            block = [block[BLAKE3_MSG_PERMUTATION[j]] for j in range(16)]
    for i in range(8):
        state[i] ^= state[i + 8]
        state[i + 8] ^= cv[i]
    return state


def _words64(block: bytes) -> list:
    return [int.from_bytes(block[i * 4 : (i + 1) * 4], "little") for i in range(16)]


def _chunk_cv(chunk: bytes, chunk_counter: int) -> list:
    """Chaining value of one chunk (ROOT flag NOT applied)."""
    cv = BLAKE3_IV[:]
    if not chunk:
        # Empty chunk (only reachable for the all-empty input case).
        # The block_len word is 0 for the zero-byte block, per spec.
        out = _compress(
            cv,
            [0] * 16,
            chunk_counter,
            0,
            BLAKE3_CHUNK_START | BLAKE3_CHUNK_END,
        )
        return out[:8]
    for start in range(0, len(chunk), BLAKE3_BLOCK_LEN):
        block = chunk[start : start + BLAKE3_BLOCK_LEN]
        flags = BLAKE3_CHUNK_START if start == 0 else 0
        if start + BLAKE3_BLOCK_LEN >= len(chunk):
            flags |= BLAKE3_CHUNK_END
        # block_len is the ACTUAL byte count of this block (64 for full
        # blocks, less for the final partial block) -- the pinned
        # vectors caught a hardcoded-64 drift here during integration.
        out = _compress(cv, _words64(block), chunk_counter, len(block), flags)
        cv = out[:8]
    return cv


def blake3(data: bytes) -> bytes:
    """BLAKE3 hash-mode digest (first 32 bytes), spec-exact.

    This mirrors the OFFICIAL reference implementation's Hasher
    semantics (reference_impl.rs, BLAKE3-team/BLAKE3), which the pinned
    vectors enforce:

    - a full block is compressed only when MORE INPUT FOLLOWS (a block
      that fills exactly at the end of input stays buffered and is
      compressed as the output block);
    - a completed chunk is folded into the CV stack only when more
      input follows (a chunk that completes exactly at the end of input
      stays in the chunk state and becomes the root output);
    - parent nodes ALWAYS use counter 0 and block_len 64 (the
      reference's parent_output comment: "Always 0 for parent nodes");
    - the merge law is the trailing-zeros rule of the running chunk
      count (add_chunk_chaining_value), not a size-equality stack rule;
    - ROOT rides ONLY the final compression: intermediate chunk/parent
      chaining values are compressed without it, and the last fold
      adds it.
    """
    cv_stack = []  # completed subtree chaining values
    chunk_cv = BLAKE3_IV[:]
    chunk_counter = 0
    block_buf = bytearray()
    blocks_compressed = 0

    def chunk_len() -> int:
        return BLAKE3_BLOCK_LEN * blocks_compressed + len(block_buf)

    def compress_chunk_block(root: bool) -> list:
        flags = BLAKE3_CHUNK_START if blocks_compressed == 0 else 0
        flags |= BLAKE3_CHUNK_END
        if root:
            flags |= BLAKE3_ROOT
        out = _compress(
            chunk_cv, _words64(bytes(block_buf)), chunk_counter, len(block_buf), flags
        )
        return out[:8]

    pos = 0
    n = len(data)
    while pos < n:
        # Chunk complete and more input follows: fold it into the CV
        # stack (never ROOT here), using the trailing-zeros merge law.
        if chunk_len() == BLAKE3_CHUNK_LEN:
            new_cv = compress_chunk_block(root=False)
            total = chunk_counter + 1
            while total & 1 == 0:
                new_cv = _compress(
                    BLAKE3_IV[:],
                    list(cv_stack.pop()) + list(new_cv),
                    0,
                    BLAKE3_BLOCK_LEN,
                    BLAKE3_PARENT,
                )[:8]
                total >>= 1
            cv_stack.append(new_cv)
            chunk_cv = BLAKE3_IV[:]
            chunk_counter += 1
            block_buf.clear()
            blocks_compressed = 0
        # Block full and more input follows: compress without CHUNK_END.
        if len(block_buf) == BLAKE3_BLOCK_LEN:
            flags = BLAKE3_CHUNK_START if blocks_compressed == 0 else 0
            out = _compress(
                chunk_cv,
                _words64(bytes(block_buf)),
                chunk_counter,
                BLAKE3_BLOCK_LEN,
                flags,
            )
            chunk_cv = out[:8]
            blocks_compressed += 1
            block_buf.clear()
        take = min(BLAKE3_BLOCK_LEN - len(block_buf), n - pos)
        block_buf += data[pos : pos + take]
        pos += take

    # Finalize: the chunk state's output (empty input included -- it
    # compresses the zero block with block_len 0) becomes the right
    # child of the stack fold. ROOT rides the last compression only.
    cv = compress_chunk_block(root=not cv_stack)
    while len(cv_stack) > 1:
        cv = _compress(
            BLAKE3_IV[:],
            list(cv_stack.pop()) + list(cv),
            0,
            BLAKE3_BLOCK_LEN,
            BLAKE3_PARENT,
        )[:8]
    if cv_stack:
        cv = _compress(
            BLAKE3_IV[:],
            list(cv_stack.pop()) + list(cv),
            0,
            BLAKE3_BLOCK_LEN,
            BLAKE3_PARENT | BLAKE3_ROOT,
        )[:8]
    return b"".join(v.to_bytes(4, "little") for v in cv)


def anchor_payload(manifest_bytes: bytes, authors_bytes: bytes) -> bytes:
    """The framed clear payload the anchor binds.

    Framing contract (mirrored byte-exactly in src/diagnostics/
    signature.rs): domain tag, then each layer's clear bytes with a
    64-bit little-endian length prefix. The length prefix keeps the
    layer boundaries unambiguous; the domain tag keeps this payload
    distinct from any other blake3 use in the tree.
    """
    out = ANCHOR_DOMAIN
    out += len(manifest_bytes).to_bytes(8, "little") + manifest_bytes
    out += len(authors_bytes).to_bytes(8, "little") + authors_bytes
    return out


def depth_anchor(manifest_bytes: bytes, authors_bytes: bytes) -> bytes:
    """The 32-byte anchor digest over the framed payload."""
    return blake3(anchor_payload(manifest_bytes, authors_bytes))


def selftest_blake3() -> None:
    """Assert pinned BLAKE3 vectors before anything is emitted.

    The vectors were cross-validated byte-for-byte against the Rust
    blake3 crate (1.8.7, pure-Rust path) at integration time. They pin
    the empty input, a short input, the 64-byte block edge, the exact
    chunk boundary (1024), the two-chunk tree shape (1025), and a
    three-chunk left-spine fold (3000) -- every structural case the
    tree traversal can take. A drift in this mirror fails here, before
    it can ship an anchor the Rust side would reject.
    """
    pattern = bytes(i % 251 for i in range(3000))
    vectors = [
        (b"", "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"),
        (b"abc", "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"),
        (
            bytes((i % 251) for i in range(3)),
            "e1be4d7a8ab5560aa4199eea339849ba8e293d55ca0a81006726d184519e647f",
        ),
        (
            pattern[:64],
            "4eed7141ea4a5cd4b788606bd23f46e212af9cacebacdc7d1f4c6dc7f2511b98",
        ),
        (
            pattern[:1024],
            "42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af7",
        ),
        (
            pattern[:1025],
            "d00278ae47eb27b34faecf67b4fe263f82d5412916c1ffd97c8cb7fb814b8444",
        ),
        (
            pattern[:3000],
            "5fade288bf27444bee55ba2babb98c3c922c1e84c2e445e7d1f6da24756f5060",
        ),
    ]
    for data, expected in vectors:
        got = blake3(data).hex()
        assert got == expected, (
            f"blake3 self-test mismatch on {len(data)}-byte input: "
            f"got {got}, expected {expected}"
        )


def longest_ascii_run(words: list) -> int:
    """Longest run of printable ASCII bytes across all masked words.

    The at-rest invisibility contract: strings(1) reports runs of four
    or more printable bytes. The emitted constants must stay below that
    floor everywhere, or the identity becomes greppable again.
    """
    longest = 0
    run = 0
    for w in words:
        for byte in (w & MASK64).to_bytes(8, "little"):
            if 0x20 <= byte <= 0x7E:
                run += 1
                longest = max(longest, run)
            else:
                run = 0
    return longest


def rust_u64(value: int) -> str:
    return f"0x{value:016X}"


def rust_array(name: str, words: list, comment: str) -> str:
    body = ", ".join(rust_u64(w) for w in words)
    return f"const {name}: [u64; {len(words)}] = [{body}]; // {comment}"


def main() -> None:
    manifest_bytes = MANIFEST.encode("utf-8")
    authors_bytes = AUTHORS.encode("utf-8")

    manifest_words = chunk_words(manifest_bytes)
    authors_words = chunk_words(authors_bytes)
    masked_manifest = mask_words(manifest_words, SEED_MANIFEST)
    masked_authors = mask_words(authors_words, SEED_AUTHORS)

    # Self-check: the block must round-trip, and it must stay invisible
    # to strings(1)-style scans (no printable run of length >= 4).
    assert (
        unmask_words(masked_manifest, SEED_MANIFEST, len(manifest_bytes))
        == manifest_bytes
    )
    assert (
        unmask_words(masked_authors, SEED_AUTHORS, len(authors_bytes)) == authors_bytes
    )
    assert longest_ascii_run(masked_manifest) < 4, "manifest words leak ASCII"
    assert longest_ascii_run(masked_authors) < 4, "authors words leak ASCII"

    print("// --- Generated by scripts/release/generate-provenance-words.py ---")
    print("// Paste into src/diagnostics/signature.rs (replacing the existing")
    print("// constants of the same names). Do not hand-edit the words.")
    print()
    print(f"pub(crate) const MANIFEST_SEED: u64 = {rust_u64(SEED_MANIFEST)};")
    print(
        f"pub(crate) const MANIFEST_BYTE_LEN: usize = {len(manifest_bytes)}; "
        f"// {len(manifest_words)} words"
    )
    print(rust_array("MASKED_MANIFEST_WORDS", masked_manifest, "layer D0"))
    print()
    print(f"pub(crate) const AUTHORS_SEED: u64 = {rust_u64(SEED_AUTHORS)};")
    print(
        f"pub(crate) const AUTHORS_BYTE_LEN: usize = {len(authors_bytes)}; "
        f"// {len(authors_words)} words"
    )
    print(rust_array("MASKED_AUTHORS_WORDS", masked_authors, "layer D1"))
    print()
    # night-signature-advanced-3: the blake3 anchor over the framed
    # clear payload. Self-test first: a drifted mirror must fail here,
    # before it can emit an anchor the Rust side would reject.
    selftest_blake3()
    anchor = depth_anchor(manifest_bytes, authors_bytes)
    anchor_hex = anchor.hex()

    print("// night-signature-advanced-3: blake3 anchor over the framed clear")
    print("// payload (ANCHOR_DOMAIN + len64le(manifest) + manifest +")
    print("// len64le(authors) + authors). Verified at runtime by the blake3")
    print("// crate over the SAME materialized bytes the layers display.")
    anchor_words = ", ".join(f"0x{b:02X}" for b in anchor)
    print(
        f"pub(crate) const DEPTH_ANCHOR: [u8; 32] = [{anchor_words}]; "
        f"// b3:{anchor_hex}"
    )
    print()
    print("// Cross-check values (informational; the Rust const fingerprints")
    print("// are computed independently from the words at compile time):")
    print(f"// manifest fingerprint: 0x{fnv1a64(manifest_bytes):016X}")
    print(f"// authors fingerprint:  0x{fnv1a64(authors_bytes):016X}")
    print(f"// depth anchor (blake3): b3:{anchor_hex}")
    print(
        f"// longest printable run across masked words: "
        f"{max(longest_ascii_run(masked_manifest), longest_ascii_run(masked_authors))}"
    )


if __name__ == "__main__":
    main()
