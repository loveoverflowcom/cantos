# Local pixel comparison

`cargo xtask visual-compare` compares two existing PNGs offline and writes fresh, ignored review
artifacts. It does not capture an app, update a baseline, upload data, approve a design, or add a
CI merge threshold. The [spike record](../../docs/evidence/visual-compare.md) and
[proposed decision](../../docs/decisions/0029-local-visual-comparison.md) explain the local MVP choice.

## Optional QA dependency

Python 3.9+ and **Pillow 11.3.0** are required for PNG I/O. No application depends on Pillow.
An already configured interpreter can be selected with `--python /path/to/python`.
Missing or different Pillow is `UNSUPPORTED`; comparison and ordinary verification never install
packages or access the network. Core unit tests use the standard library; adapter tests explicitly
skip when the optional pinned Pillow is absent. A skip supplies no PNG adapter evidence.

To prepare an isolated local environment explicitly:

```sh
python3 -m venv artifacts/visual-compare-venv
artifacts/visual-compare-venv/bin/python -m pip install -r tools/visual-compare/requirements.txt
cargo xtask visual-compare --python artifacts/visual-compare-venv/bin/python --self-test
```

Pillow 11.3.0 supports the repository's observed Python 3.9.6 host; upgrading this QA pin requires
the alpha/threshold regressions and calibration to run again. It is not an app or release dependency.

## Compare a pair

```sh
cargo xtask visual-compare \
  --reference /path/to/approved.png --actual /path/to/captured.png \
  --scenario theatre-player-light-vi-v1 --metadata /path/to/pair.json
```

`--output artifacts/ui-evidence/<run-id>/comparison` selects a **new** directory under the
evidence root. Existing directories are refused. Otherwise a unique UTC run directory is created.
Both source files are copied from the exact bytes hashed and decoded. Inputs are single-frame
PNGs with at most 8-bit channels, at most 64 MiB and 16 million pixels; the tile grid is bounded
to 262144 cells. Sixteen-bit input is refused rather than silently quantized to RGBA8.

The default `--threshold 0` counts exact changes. A pixel differs when the largest absolute
**premultiplied RGBA** channel delta is greater than the selected integer threshold (0–255).
Hidden RGB at alpha zero is ignored; alpha changes are measured. Encoded channel values are
compared without ICC/gamma conversion, resampling, cropping, automatic font normalization or
anti-alias suppression. Any nonzero threshold is an explicit review policy, not a universal safe
noise floor. `--tile-size 16` groups adjacent occupied tiles (including diagonals); pixel bounds
inside a group remain exact. Nearby distinct controls can share a region.

Equal or different pixels both produce tool `PASS`: the operation completed. This never means
UX passed. Different dimensions or explicitly conflicting pairing metadata produce `FAIL` with
`dimension_mismatch` or `scenario_mismatch`, original images and a report **without a score or
resized overlay**. Missing attribution remains `unknown`; raw comparison is useful but supplies
no design-fidelity claim.

## Pair metadata (schema v1)

```json
{
  "baseline": {
    "id": "theatre-player-light-vi", "revision": "v1",
    "sha256": "<exact reference PNG SHA-256>",
    "approval": {"status": "approved", "owner": "<named owner>", "record": "<review reference>"}
  },
  "reference": {
    "source_sha": "<design revision>", "renderer": "<design HTML browser and version>",
    "viewport": {"width": 390, "height": 844, "unit": "physical px"},
    "pixel_ratio": 1, "font": "<family/version/hash>", "theme": "light", "locale": "vi-VN",
    "text_scale": 1, "fixture": "mot-loi-hen-v1", "state": "paused", "captured_at": "<UTC>"
  },
  "actual": {
    "source_sha": "<rendered build SHA and dirty description>", "renderer": "<actual renderer/version>",
    "viewport": {"width": 390, "height": 844, "unit": "physical px"},
    "pixel_ratio": 1, "font": "<family/version/hash>", "theme": "light", "locale": "vi-VN",
    "text_scale": 1, "fixture": "mot-loi-hen-v1", "state": "paused", "captured_at": "<UTC>"
  },
  "masks": []
}
```

This example needs real values; it is not an approved baseline. `approval.status` may instead be
`calibration` for original test fixtures or `unreviewed`. Approved/calibration declarations require
an ID, revision, named owner, review record and matching reference hash. Approval is caller
attribution, not authenticated by the tool. Optional capture `sha256` pins are checked too.
Capture unknowns are retained in `provenance_gaps`. Pairing checks cover viewport, pixel ratio,
font, theme, locale, text scale, fixture and state; renderer and source revision may intentionally
differ. Preserve the logical CSS px/dp/pt context alongside the physical viewport in review notes.

Masks are never inferred. Each rectangle needs `x`, `y`, `width`, `height` in physical pixels,
`reason`, `reviewed_by` and `review_record`. Coordinates must lie inside the image; overlapping
masks count their union once; masking every pixel is refused. Cyan outlines show exclusions.
Duplicate metadata keys and non-finite JSON numbers are refused.

## Output contract (schema v1)

| Artifact or field | Meaning |
| --- | --- |
| `reference.png`, `actual.png` | Exact input bytes, never overwritten |
| `diff.png`, `mask.png`, `heatmap.png` | Highlighted changes, binary unmasked change mask and channel-delta heatmap |
| `overlay.png` | 50/50 context image, numbered red region bounds and cyan reviewed exclusions |
| `region-<id>-{reference,actual,overlay}.png` | Detail crops with 8 px context for the three largest regions |
| `report.json`, `report.md` | Metrics/provenance and a local review index |
| `diff_pixels`, `total_pixels`, `diff_ratio` | Unmasked changed count / **whole-image** pixel count; never a percentage of design quality |
| `compared_pixels`, `excluded_pixels`, `unmasked_diff_ratio` | Reviewed mask accounting and changed count / compared count |
| `regions` | Descending pixel count, then y/x; `{id,x,y,width,height,pixels,local_ratio,severity_hint,images?}` |
| `local_ratio` | Region changed pixels / its bounding-box area |
| `severity_hint` | Area hint only (`review-large-area` at 5% image area); no P0–P3 or UX decision |
| `regions_omitted` | Count beyond the 256 reported regions; all changes remain in counts/mask |
| `artifact_sha256`, `provenance`, `comparator` | Image identities, baseline/render context and implementation/configuration hashes |
| `recheck_policy_sha256` | Pins scenario, reference bytes, pairing, masks, threshold, tile size and comparator implementation/version; allows a new actual build/capture |
| `images_inspected` | Always false; inspection is a separate attributed record after opening images |

Coordinates start at top-left; right/bottom are exclusive (`x + width`, `y + height`). A box is a
spatial group, not a proven component or root cause. No SSIM is computed. Read the
[visual review method](../../.agents/skills/cantos-ui-design/references/visual-review.md) before
turning metrics into findings. The inspector report consumes this contract; no duplicate skill
or separate evidence vocabulary is introduced.

## Repeat the optional backend spike

The isolated npm lock is only for research, with no default verification/app consumer. Explicitly
prepare it with `npm ci --ignore-scripts --prefix tools/visual-compare/spike`. Node and npm are
operator supplied. Lifecycle scripts are disabled; odiff uses its pinned optional platform binary.

Create a JSON array of `{ "id": "color", "reference": "reference.png", "actual": "color.png" }`
pairs. Paths resolve from the manifest directory. Use original/authorized real PNG captures and
independently known controlled mutations; do not label synthesized pixels as an actual UI fix.

```sh
cargo xtask visual-compare --spike-pairs artifacts/ui-evidence/<run-id>/pairs.json
```

The spike records one warm-up and five samples per pair for A/B/C/D, median decode + comparison +
diff/mask encode time, tool parameters, hashes and host platform. It excludes fixture capture,
Node startup and MVP region/crop/report work. Looks-same shortcuts identical bytes and emits no
diff there. Timings are exploratory, not statistically robust or evidence of equivalent output.
