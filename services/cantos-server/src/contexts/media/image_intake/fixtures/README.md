# Synthetic image fixtures

The three original PNG/JPEG positive fixtures were converted to actual AVIF under #607.
`conversions.json` records old/new names, independently computed SHA256, sizes and encoder.
All are 2×3 synthetic images, not real artwork. RGB/RGBA16 originals were independently
constructed with Python struct/zlib; the JPEG used Pillow. Original RGBA16 is opaque solid
red, so RGB encoding retains its visible values and opacity. Converted files use Pillow12.3.0,
libavif1.4.2, quality100, 4:4:4, speed4. No source digest is reused for changed bytes.

`ffmpeg-yuv420p-2x3.avif` is the independent FFmpeg/libaom fixture from #608 (322 bytes,
SHA256 `7c2306b2967d74b97c08809e1aa73b4923ab830a71cdecdf7acf2f0ba6873257`).
The production inspector now fully decodes AV1 pixels with pinned dav1d after framing,
orientation, dimensions, bit-depth and allocation checks. No encoder participates in admission.

`legacy-rejection.json` contains the exact original synthetic encoded bytes as hex, exclusively
for negative format/rename/admission tests. It is never a positive fixture, resource or served
image. No legacy image files or checksum-waiver entries remain. Historical inventory records
retain original source hashes without making those images admissible again.
