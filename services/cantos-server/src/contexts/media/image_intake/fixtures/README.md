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

## Alpha and ImageGrid fixtures

`pillow-alpha-64x48.avif` (single primary + `auxl` alpha item), `grid-2x2-120x100.avif`
(2×2 grid of 64×64 tiles cropped to 120×100) and `grid-alpha-2x2-120x100.avif` (color grid +
alpha grid) are synthetic. `build_alpha_grid_fixtures.py OUT_DIR` (Python 3.9, Pillow 11.3.0 with
libavif 1.3.0; 4:4:4, quality 100, speed 6) encodes the tiles with Pillow and assembles the grid
containers by hand as `avifenc --grid` lays them out. The script never decodes AV1: it decodes
each result with libavif and compares it with the source (maximum channel error 2/255, alpha
exact). Hashes: pillow-alpha `2a204a34…6482`, grid `b7689baf…e296`, grid-alpha `e4d69222…9dc`.
The same bytes are copied to the Theatre desktop test resources; each consumer owns its copy.
Negative cases are same-length patches of these files in `image_intake/tests.rs`.
