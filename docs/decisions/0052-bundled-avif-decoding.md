# ADR 0052: Decode bundled AVIF artwork at renderer adapters

- Status: proposed; source implementation in the #607 PR, native qualification pending
- Date: 2026-10-29
- Decision owner: Cantos owner
- Related issue or PR: [#607](https://github.com/loveoverflowcom/cantos/issues/607)
- Supersedes / superseded by: N/A

## Context

The [image policy](../design/runtime-images.md) selects AVIF for raster resources. Compose
1.9.0 does not generate an AVIF drawable accessor, and the installed Skiko 0.9.22.2 decoder
rejects the converted paintings in the real Desktop Preview resource test. Simply replacing
WebP/PNG paths breaks those images. The native hosts retain Android minSdk23 and iOS15.

## Decision

Store bundled AVIFs in Compose raw `files/artwork` resources and decode off the UI thread
through one multiplatform adapter. Keep decorative semantics, aspect ratio and the existing
native-vector recovery. This adapter is for **trusted bundled artwork only**, never a new
Server decoder or an alternate HTTP/public-cover admission path.

The desktop implementation pins `media.kamel:avif-jvm:0.0.1` in `desktopMain` only. Its
[published source](https://github.com/qdsfdhvh/avif-kmp) wraps libavif/dav1d and packages native
libraries for supported JVM hosts. It copies its own native JAR resource to a temporary file
and loads it; it does not download a codec at runtime. Close the decoder and intermediate
bitmap on every exit, bound input/dimensions, require one frame and preserve vector recovery
on missing native linkage. The dependency is Apache-2.0 as declared by its published POM;
the dependency graph and licences remain part of artifact review.

Android uses its platform AVIF decoder from API31 first, then the official AOMedia
`org.aomedia.avif.android:avif:1.3.0.841110fd` JNI implementation when native decoding is
unavailable. The exact published AAR declares minSdk21, with 16KB PT_LOAD alignment in both
64-bit ELF payloads. It covers the app's API23 floor without a minimum-SDK change; this is
packaging evidence, not device proof. The KMP candidate's Android AAR requires API24 and is
not used. A lazy 8MiB direct buffer and one synchronized software decode avoid repeated
DirectByteBuffer allocations and concurrent full YUV frames. Header dimensions, one frame,
16MP and a 1536px output plan are checked before decode; the native decoder is released on
all exits. Software decode can still allocate a full YUV frame before scaling; no total-RSS
or hardware-acceleration guarantee is claimed. iOS uses ImageIO to obtain pixels for Compose,
returning vector recovery when the OS cannot decode; it adds no native codec dependency.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| Existing Compose/Skia decode only | No new dependency | Converted AVIF paintings return null on the pinned Desktop Preview | Rejected by the exact resource regression |
| Desktop-only pinned AVIF adapter + OS adapters | Preserves Desktop Preview artwork; keeps platform dependencies local | Pinned native JVM and Android dependencies; browser/native/runtime acceptance stays separate | Proposed for this bounded static migration |
| New cross-platform native codec or higher minimum OS | Could provide raster artwork on older devices | New ABI/packaging/security/support decision; the inspected Android AAR misses API23 | Deferred for owner review and device qualification |
| Legacy raster fallback or embedding PNG in SVG | Retains old decoding | Violates the selected source policy | Rejected |

## Consequences

- Product impact: capable platforms retain the same artwork; unavailable codecs show the
  existing vector placeholder. Named OS/device visual acceptance is pending.
- Compatibility and migrations: no Server schema, receipt, public-cover DTO or database changes.
  The [Media transition](../design/runtime-images.md#media-contracts-and-existing-data) remains open.
- Operations/cost: no provider, network, object-storage or deployment action; desktop native
  package size and resolved dependency versions are included in the audit.
- Recovery: retain source hashes/provenance; reverting static code is independent of persisted
  media. Do not silently restore source PNG/WebP as a policy workaround.
- Deferred: API23–35 native runtime qualification, old iOS support, other JVM architectures, native app/device
  qualification and production Media decoder choice.

## Validation and revisit conditions

The exact paintings must decode in `cargo xtask theatre-preview verify`; the shared Library
fixture must retain 1374 × 1145 dimensions and its transparent margin. The policy guard catches
legacy or mislabeled source. Android release packaging and iOS framework/app/device evidence
are independent. Revisit this candidate when Compose/Skia supplies a qualified codec or a
reviewed native codec can cover the declared minimum OS versions without widening the boundary.
