# Drive evidence storage, recovery and index

This protocol owns agent I/O and recovery for the
[capture bundle](../../cantos-ui-capture/references/evidence-bundle.md). It is not a Cantos
backend integration or a guarantee that a connector supports atomic upload/index operations.

## Ground the destination

The user-selected root is
[CantosDesign](https://drive.google.com/drive/folders/1GVOA3v-TGRRo23dFdeHW2lg32WI9DPZ4).
A read-only listing on 2026-11-04 found Theatre, Studio and shared UI design material, with
no `03_IMPLEMENTATION` child. This observation created nothing and grants no write authority.
Re-read metadata and children each time; the location can change. A user-selected alternative
destination takes precedence. Do not use Vink's IDs or infer a current approved design baseline
from this root's folder names.

Within a requested upload to this root, resolve/create this layout:

```text
CantosDesign/
  03_IMPLEMENTATION — UI Captures/
    00_SCREEN_INDEX.md
    runs/
      <run-id>/
        manifest.json
        REPORT.md
        navigation.mmd
        navigation-adjacency.json
        theatre-web/
        theatre-cmp/{android,ios}/
        studio-web/
        previews/                         # only when captured, never native coverage
        metadata/
```

Resolve by verified parent and file ID, using names only as discovery hints. Reuse a unique
existing implementation/runs folder, including a clearly matching pre-existing implementation
name; do not rename it for cosmetic consistency. Verify folder MIME, parents and non-trashed
state. Create missing folders as part of authorized upload, recording intent/result/readback.
If duplicate roots/runs or inconsistent parents cannot be reconciled with the receipt, report
ambiguity and retain local evidence; do not choose arbitrarily or create another root.

Do not modify/rename/move design folders, prototypes or other users' runs. Keep sharing unchanged;
a folder's inherited access is not a reason to add public permissions. No whole-repository,
credential, private story-workspace or unrelated log upload is implied.

## Connected tools

Prefer connected Google Drive. Discover available capability names/descriptions and read the
current schemas; the following shapes were available when this skill was authored:

| Operation | Shape / detail |
| --- | --- |
| Metadata readback | `get_file_metadata(fileId, fields)`; request id/name/mimeType/parents/trashed/size/webViewLink/modifiedTime and supported checksums/image metadata |
| Children | `list_folder(url, top_k)` takes a folder URL; it can be bounded. To establish absence, use `search` with an exact parent/name/trashed filter and follow `next_page_token` using `page_token` until exhausted; do not mistake the first 100 children for completeness |
| Create folder | `create_folder(name, parent_folder)` takes the grounded parent ID or URL; read back the resulting ID/parents |
| New raw file | `upload_file(file_uri=<absolute local path>, file_name, mime_type, parent_folder_id=<raw folder ID>)` |
| Replace canonical index bytes | `update_file(fileId, file_uri=<absolute local path>, mime_type="text/markdown")`; preserves its ID; no native Docs conversion |
| Content read | `fetch(url)` for readable JSON/Markdown; stored PNGs and exact-byte verification use supported raw-file fetch/download after metadata confirms MIME |

Keep PNG `image/png`, manifest/adjacency `application/json` and report/index `text/markdown`.
Do not import these as Google Docs or recompress images. If a connector cannot access a local
path, use an available supported file-transfer mechanism; never create an OAuth token, public
upload URL or a speculative CLI workaround. Missing capability is a named upload blocker.

## Upload and reconciliation

1. Validate the frozen manifest digest and resume receipt. Find the exact run in its verified
   `runs` parent. One matching folder resumes; multiple matches require a verified receipt ID
   or ambiguity resolution. No match after a complete search permits creation if no prior write
   remains uncertain. A folder-create timeout has the same reconciliation requirement as a file.
2. Persist operation identity (run, path, local digest/size, intended parent) before each write;
   record returned ID, time and result immediately after. On resume inspect remote IDs and
   search the correct parent before creating anything. Reuse only a file with matching content,
   using a like-algorithm checksum or a downloaded byte/hash comparison, not name/size alone.
3. Upload frozen images/sidecars and graph, then report, then manifest. After each write read
   metadata back: exact ID, parent, name, MIME, byte size, non-trashed state, observed URL and
   image dimensions if exposed. Record omitted metadata as unavailable, never guessed. A
   required parent/name/type/size check missing from the connector leaves metadata verification
   incomplete, even if upload returned success.
4. Compare remote SHA-256 with local SHA-256 when supplied. If only MD5 exists, compute local
   MD5 for that comparison while retaining canonical local SHA-256. Otherwise download raw
   bytes and compare hashes when supported. Without a content check report `metadata-only` or
   `unavailable`, never byte integrity. Cited-image opening and remote integrity are separate.
5. A timeout/disconnection after write is `UNCERTAIN`, not failed-with-no-effects. Search/read
   back before any retry; matching content reconciles success, differing content is a conflict.
   Never overwrite a same-name historical file. If absence or identity cannot be established,
   stop that artifact and keep it uncertain. After a confirmed temporary failure and resolved
   outcome, allow one retry; a repeated failure remains `FAILED`/`UNCERTAIN`. Continue independent
   files within the task budget, preserving all attempt records.
6. Upload the frozen manifest last, fetch its readable content and check run ID, source/runtime
   identities, IDs/counts and references against local bytes/records. Record its local digest,
   remote ID/URL/readback and final per-file results in the receipt, **outside** the manifest.
   Do not edit sealed manifest/report bytes to add their own remote checksum or upload status.

An interrupted upload resumes the same immutable run. Captures or corrections made later form
a new run with a predecessor link. No native/coverage gap changes because upload succeeded.
Return verified links even for a partial upload, with precise missing files and recovery steps.

## Canonical screen index

Resolve exactly one raw `00_SCREEN_INDEX.md` under the implementation root; read ID, parent,
MIME, modified time and full content. Create it only after a complete search establishes absence
and no unresolved prior create exists. Duplicate indexes are ambiguous, not permission to add
another. Only the canonical index is updated in place; run files remain immutable.

Preserve all prior run history and semantic screen IDs. Include:

- run ID, capture time, requested ref and actual runtime build/source identities;
- links to verified run folder, manifest, report and navigation/adjacency;
- `Screen ID | Surface/renderer | Route/scenario/state/theme/role | Capture link | Review/gap`,
  plus component/catalog IDs and exact design reference/version/authority only when known;
- capture coverage and upload/content-integrity counts separately, blockers and prerequisites;
- `NOT_REVIEWED` unless an attributed inspector report exists; its P0–P3 findings stay separate;
- latest attempted and latest complete **per comparable scope/renderer**, derived from capture
  time, not upload completion. An older resumed run or narrow partial run never claims broader
  current coverage. A run's own manifest remains the source of its exact scope and provenance.

Re-read modified time/content immediately before update; if changed, merge from the new copy by
run ID and preserve its history. The current connector has no guaranteed compare-and-swap.
Avoid concurrent writers; on repeated conflict leave index update pending while preserving
the uploaded run. Fetch content after update and verify old history plus new links/counts.
Never claim an atomic index write or erase another writer's entries to make a retry pass.

## Retrieval for inspection

Resolve a requested deployment/source/surface through the index, then pin the exact manifest
file ID and digest. Never equate “latest upload” with latest deployed or requested `develop`.
Fetch necessary artifacts using the receipt/index links or verified run-relative parents;
validate actual bytes against the manifest. Store retrieved bytes under an ignored local
directory preserving provenance. If hashes conflict, withhold that artifact's findings and
report the conflict; continue independently verified images. Preserve unknown deployment SHA,
preview/native classification and coverage gaps. Missing raw-download capability limits integrity;
it does not authorize relabelling a preview, inventing bytes or launching the app to replace them.
