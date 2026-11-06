# UI evidence bundle — version 1

This is the capture/uploader/inspector handoff format, maintained by `cantos-ui-capture`.
It is an agent-authored JSON contract, not a new application API, runner or enforced schema.
Use UTF-8; unknown facts are `null` with a reason, never example values asserted as observations.

## Files and identity

Use a unique `run_id`, for example `2026-11-04T143000+0700_develop_<sha8>_<suffix>`.
Store full branch/SHA and timezone (`Asia/Ho_Chi_Minh` by default) in the manifest. With no source
checkout use an `unknown-source` slug and explain it. Read the previous screen index/registry
when available; keep semantic `screen_id`/`component_id` stable across runs, with surface/variant
separate. Reuse repo catalog/scenario IDs verbatim; record aliases when an ID changes meaning.

```text
artifacts/ui-evidence/<run-id>/
  manifest.json
  REPORT.md
  navigation.mmd
  navigation-adjacency.json
  theatre-web/                 # product browser captures
  theatre-cmp/android/          # actual native runtime only
  theatre-cmp/ios/
  studio-web/
  previews/                    # named mock/component/IDE hosts, separate coverage
  metadata/                    # permitted original sidecars, semantics and source inventory
  upload-receipt.json           # local recovery ledger, not an immutable run artifact
```

Create only directories containing outputs. Artifact paths are relative, unique and confined
to this bundle: no absolute paths, `..`, symlinks or files outside the selected root. A useful
PNG filename is `<surface>__<screen>__<state>__<theme>__<size-unit>__<capture-id>.png`.
Keep original capture names/paths in provenance when copying; never overwrite source images.

## Manifest fields

All top-level fields below are required; empty collections need a reason in `scope`.
Every target/capture/artifact/navigation reference resolves to an existing ID.

| Field | Required content |
| --- | --- |
| `format_version` | `1` |
| `run` | `run_id`, repository, requested ref, full checkout `git_sha` or null with reason, dirty state and digest of relevant dirty/untracked build inputs, started/finished ISO-8601 timestamps with offset, timezone, tools/versions and exact safe commands |
| `scope` | requested surfaces, roles, variants, budget, planned capture IDs, selection/exclusion reasons; retain original expected set and record later additions explicitly |
| `targets[]` | `target_id`, surface, `evidence_kind`, renderer/host/version, environment, sanitized origin/API label, data mode, `build` {status `VERIFIED` or `UNKNOWN`, full runtime `git_sha` or null, build/deployment ID, artifact SHA-256, variant, binding evidence and unknown reasons}; backend identity separate from frontend identity |
| `screens[]` | stable `screen_id`, label, surface, route pattern or native destination, source owner, component/catalog/scenario IDs, observed vs source-only inventory |
| `captures[]` | `capture_id`, screen/target/component/scenario IDs, planned variant, status, reason/prerequisite, sanitized route/deep link, click path, fixture, account alias/role, auth mode, state, navigation/overlay state, requested/effective theme, locale, orientation, viewport {width,height,unit}, image pixel dimensions, density/DPR, zoom/font scale, captured-at timestamp, readiness/motion, artifact ID, opened-image record, review status |
| `artifacts[]` | `artifact_id`, relative path, kind, MIME, byte size, SHA-256; PNG pixel width/height; original sidecar/source and derivative/redaction lineage where applicable |
| `navigation` | graph and adjacency artifact IDs; nodes for screens/overlays and edges {from,to,action,pre/post state,guard,`observed` or `source-only`,capture IDs/source}; no credentials in paths |
| `coverage` | counts derived from captures by status, opened count, breakdown by surface/evidence kind/renderer/theme/role/state, plus excluded cells and reasons; `COMPLETE` or `PARTIAL` with gaps |
| `review` | `NOT_REVIEWED` by default; optional pinned reference IDs/revisions/authority and separately attributed inspector report artifacts when requested |

`evidence_kind` distinguishes `product-runtime`, `mock-app`, `component-preview`, `host-preview`
and `design-reference`. A target can be an unknown deployed build and still supply real product
pixels. Mark its SHA unknown; never substitute the inspection checkout's SHA. A locally built
target needs the build command/input fingerprint and the serving/installed artifact binding;
a dirty build is identified by SHA **plus** input fingerprint, not SHA alone.

Capture status is independent of gate/test status:

| Status | Meaning |
| --- | --- |
| `CAPTURED` | actual screenshot file exists, measured and hashed; visible error/loading can be the intended state |
| `BLOCKED` | implemented/unknown state cannot be reached due to a named prerequisite, e.g. test account/fixture |
| `NOT_IMPLEMENTED` | current source establishes this screen/state is absent; cite evidence |
| `UNSUPPORTED` | required tool, renderer or runtime is unavailable; name it |
| `NOT_RUN` | planned cell not executed, e.g. budget/cancellation; keep a continuation step |
| `OUT_OF_SCOPE` | explicitly excluded with rationale; never used to hide a failed planned cell |

Non-captured records have no image artifact and keep reason/prerequisite. Opened-image records
contain tool/reviewer, timestamp, SHA-256 of opened bytes and visible observations, or null with
reason. Opening confirms content, not a P0–P3 audit. A capture run is `COMPLETE` only if every
requested in-scope cell is captured and its cited images opened; native gaps remain gaps even
if previews exist. Scope and deployment identity gaps stay explicit beside coverage.

## Component and token records

Add these sections when their subjects are in scope. A version 1 bundle without these sections
remains readable. An omitted section in an older bundle has no recorded evidence for that subject.

| Field | Content |
| --- | --- |
| `components[]` | `component_id`, name, source/API owner, catalog and scenario IDs when registered, and usages with `screen_id`, `target_id`, variant/state, source and capture IDs. Mark the usage `observed` or `source-only` and state any uncertain mapping. |
| `tokens.sources[]` | Token or catalog path, version if declared, SHA-256, source/build identity and optional artifact ID for a permitted snapshot in `metadata/`. Identify checkout lookup bytes separately from runtime-reported bytes. |
| `tokens.observations[]` | Semantic token key, `component_id`, `target_id`, `capture_id` when available, rendered element/scope, effective theme, role mapping or consuming property, source value when known, effective value and unit/format, applicable override, method, evidence status and artifact ID if measurement output was saved. |

A token observation is `MEASURED` when it identifies the capture context and shows how the
active component uses the value. A value seen only in source or generated mappings is
`SOURCE_ONLY`. A role selected for the run with unavailable evidence is `NOT_MEASURED` with a
reason. Leave the effective value null in the latter two cases. Preview measurements retain
their preview target IDs. The measurement statuses do not affect screenshot totals or indicate
contrast, design fidelity or product acceptance.

## Report, validation and sealing

`REPORT.md` names scope/source/runtime provenance, exact reproduction path and fixtures,
surface-by-surface expected/captured/opened/gap counts, screen/component inventory, artifact
links, navigation and blockers with next prerequisites. State `NOT_REVIEWED` unless an inspector
actually ran; do not manufacture severity from unavailable screens. A still image supplies no
native playback, durability, focus traversal, motion or screen-reader acceptance.

For a component or token report, link screen → component → catalog/scenario → capture where
available, and component → semantic role → renderer observation. Use `Component | Screen or
specimen | State | Source | Capture or gap` and `Token role | Component and target | Source
value | Effective value and unit | Method or gap` as report columns. Group measurements and gaps
by renderer and theme. Show source-only and unmeasured rows even if the screenshots are complete.

Keep `navigation.mmd` and adjacency data from the same edge records. Use safe Mermaid aliases
for node syntax and visible labels for stable screen IDs; label source-only edges explicitly.
A design-reference link is not an app route. Empty navigation records explain that no edges
were observed. Local report links are relative; verified Drive links belong in the upload receipt
and screen index. This preserves the captured bundle's hashes across upload/resume.

Before handoff validate file presence/MIME/size/hash and PNG dimensions against actual bytes,
IDs/paths, totals, status reasons and image-open records. Do not count preview/product together.
When component or token records are present, check their links and confirm that each measured
row points to the recorded target and capture context. Check that source-only and unmeasured
rows leave effective values null.
The manifest does **not** list/hash itself or `upload-receipt.json`; the external receipt holds
the final manifest's digest/remote ID. Finish and freeze the captured files before upload.
Resume upload of identical bytes; a later capture or correction creates a new run referencing
the prior run, rather than rewriting published evidence. Preserve original runner sidecars.

## Local upload receipt

Owned by the [Drive protocol](../../cantos-ui-evidence-drive/references/drive-protocol.md).
Keep `run_id`, manifest SHA-256, verified folder IDs/URLs and per-file {relative path, local
SHA-256/size, parent ID, remote file ID/observed URL, status, call time, readback, remote checksum
algorithm/value, integrity level, error/reconciliation}. Write planned operation identity before
each remote write and record its result immediately afterwards, including folder creation.

Statuses: `NOT_REQUESTED`, `PENDING`, `UPLOADED`, `VERIFIED`, `FAILED`, `UNCERTAIN`.
`VERIFIED` means metadata was read back and matched; integrity is independently
`content-verified`, `metadata-only` or `unavailable`. Compare like hash algorithms only; local
SHA-256 is never compared directly with Drive MD5. A receipt also records report/manifest/index
verification and upload completeness separately from capture completeness. It contains no secrets.
