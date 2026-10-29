# ADR 0053: Fully decode AVIF before Media admission

- Status: proposed decoder integration; AVIF-only policy authorized by the owner in the #607 follow-up
- Date: 2026-10-29
- Related: [runtime image policy](../design/runtime-images.md), #607, #608

## Context and decision

The owner selects AVIF-only active admission, superseding the additive legacy admission in
#608. Keep legacy enum numbers, historical documents, immutable receipts and stored bytes
readable without granting new admission, selection or delivery. Migration30 adds future-row
guards; it does not rebuild tables or rewrite existing evidence. Public legacy covers become
fallback; private legacy byte previews are refused. A new AVIF replacement requires new
identity, explicit owner rebind and disclosure; no automatic data conversion is authorized.

The existing ISOBMFF/AV1 framing parser cannot prove pixel decoding. Keep its early bounds,
orientation and single-image checks, then use pinned `dav1d=0.11.1` to decode the primary frame
with native limits set before input: one thread, one-frame delay, strict compliance and
16,777,216 pixels. Verify output dimensions/bit depth and no second picture. The existing
blocking admission lane retains bytes and native work across cancellation. Admit one
bounded alpha/ImageGrid profile, decoding every AV1 item the same way and one picture
at a time, and reject every other auxiliary, derived or extra image item. Preserve the
8MiB encoded and 64MiB decoded-layout caps; neither cap claims total native RSS or timeout.

The [wrapper settings](https://docs.rs/dav1d/0.11.1/dav1d/struct.Settings.html) expose the
pre-input native limit. An ImageDecoder constructor that decodes before applying limits is
not substituted here. No ffmpeg subprocess, network codec download or paid provider is used.

## Packaging and alternatives

libdav1d>=1.3 is an explicit build/runtime dependency. Local evidence uses installed
Homebrew1.5.4. The Server OCI build adds pkg-config/libdav1d-dev; its runtime adds libdav1d7.
Debian trixie provides [libdav1d7 1.5.1](https://packages.debian.org/trixie/libdav1d7).
Rust CI provisions development headers. Locking the Rust wrapper does not pin a host's shared
library; exact OCI/package qualification and security patch policy remain operational gates.

Structural parsing alone was rejected because corrupt entropy tiles could receive a witness.
An external decoder command was rejected because it adds a subprocess/runtime boundary.
Overlays, other auxiliary planes, nested grids and production storage conversion are separate
work. The alpha/grid profile needs no new codec: dav1d already decodes each AV1 item, and
alpha/grid are ISOBMFF structure. libavif (Pillow), macOS ImageIO, Chromium and the pinned
desktop libavif adapter independently decode the same fixtures; Android and iOS devices are not
qualified.

## Validation and recovery

Real independent FFmpeg and converted Pillow AVIFs must decode. Negative tests cover legacy,
corrupt/truncated/appended/animated/rotated/oversized input and admission cancellation.
Real file-backed Turso and private HTTP/storage tests retain owner, hash, retry, revocation and
immutability checks. Migration evidence must preserve old rows while refusing new legacy rows.
Source fixture conversion never changes a persisted receipt. Revisit when every auxiliary plane
has a reviewed bounded decoder or the host native codec/ABI policy changes. No deployed,
old-device, iOS or full six-cover source-package acceptance follows from source tests.
