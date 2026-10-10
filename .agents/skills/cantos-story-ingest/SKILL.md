---
name: cantos-story-ingest
description: >-
  Acquire authorized story chapters from a supplied website through computer use, capture
  browser HTML or download TXT/Markdown, extract with Trafilatura, and use OCR only for image
  sources. Resolve the story's snake_case folder in the Cantos Drive workspace, preserve raw
  provenance, update indexes and recover offline through local files and a GitHub issue.
  Use for story crawling, scraping, website-to-text and raw manuscript intake, not code changes.
---

# Cantos story ingestion

This is an artifact workflow entrypoint. Compose [engineering](../cantos-engineering/SKILL.md)
for evidence and [Script IR](../cantos-script-ir/SKILL.md) for source/import boundaries; load
[publication](../cantos-publication/SKILL.md) when rights need evaluating. Do not mistake creating
raw files for a saved backend script revision.

## Shared handoff

Read [story workspace](../../../docs/production/story-workspace.md) for the configured Drive root,
identity resolution, folder/index contract, revision rules, synchronization and offline recovery.
Those rules are shared by all four artifact skills; this skill does not redefine them.
Read [capture and tools](references/capture-and-tools.md) for installation or browser extraction.
For text/files already supplied by the user, preserve those inputs as `user_supplied_text` or
`user_supplied_file` and skip browser acquisition; do not claim the referenced website was
captured or verified. Resolve the story from supplied metadata and the existing catalog.

1. Read the existing project/story/stage indexes and pending sync records. Establish requested
   story, source and chapter range. A chapter URL, pagination number and chapter number are
   different facts. If no range can be inferred, inspect the index while requesting the missing
   scope; do not start an entire-book crawl by default.
2. Use computer use to open the supplied page, inspect visible title/author and observed links,
   and resolve the work identity and `raw/` destination. Use the connected Drive plugin for Drive
   operations when available; follow its own skill and discover current schemas.
3. Reuse an existing exact story mapping. For a new story, use
   [prepare_story_workspace.py](../../../scripts/prepare_story_workspace.py) after resolving its
   canonical work URL. New content belongs outside Git; the helper only initializes local folders.
4. Prefer the page's actual TXT/HTML download if it contains the requested scope. Otherwise capture
   the rendered chapter content through the documented browser export/read-only DOM APIs. Extract
   from those captured bytes; do not silently substitute a fresh HTTP fetch while calling it a
   browser capture. Follow observed next links and verify chapter identity after navigation.
5. Extract text without literary changes. Trafilatura is HTML extraction, not OCR. For raster-only
   pages use the OCR branch, retain source-page mapping and mark uncertain characters. Never repair
   censored letters or missing passages from imagination. Save inert HTML, TXT or Markdown in `raw`.
6. Compare title/range, paragraph ordering and beginning/middle/end against the rendered page.
   Record missing/duplicate chapters and extraction losses. Ads, navigation and hidden SEO are
   excluded; original attribution and translator notes remain. Empty output or a block page is
   failure, not a successful short chapter.
7. Save immutable revisions and provenance, then update indexes and synchronize per the shared
   contract. Hand off exact raw filenames/checksums and coverage to `cantos-radio-adapt`.
8. Clean only task-created temporary captures/services/tabs after durable local output is checked.
   Pending sync retains local source and operation records. For blocked access/network, execute
   the shared GitHub issue-or-local-draft recovery procedure.

## Report

Use the foundation's evidence vocabulary and name: work identity/folder, requested and acquired
range, source/capture method and tool version, raw files/checksums, extraction spot checks,
Drive IDs/readback or pending operations, recovery issue/draft, cleanup and remaining gaps.
A length count alone does not establish chapter completeness.
