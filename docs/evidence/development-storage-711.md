# Developer storage audit — issue 711

> **Source-bound macOS observation, 4 November 2026.** This initial inventory is not a
> cleanup authorization, a production result or a portable storage budget. Part of
> [#711](https://github.com/loveoverflowcom/cantos/issues/711).

## Boundary and provenance

Fetched `origin/develop` at `58259714b50ad07209496ced19403786fa448c6c`; isolated branch
`codex/711-dev-storage-audit`. `$REPO` denotes the original `Projects/Mine/cantos`
checkout, `$TASK` its sibling `cantos-711`, and `$HOME` the operator's home directory.
The original branch was `codex/theatre-account-design-polish` at `d0988e3e9e2b65813df8fcbb3ed00bf1516257b1`
with eleven dirty paths. No original path was edited or deleted by this task. Git worktree
registration and fetch necessarily update the shared Git metadata.

Local full records are under `$TASK/artifacts/agent-runs/2026-11-04-711-storage/`.
They remain ignored; only this sanitized summary is tracked. `inventory.py` is one-off
metadata collection, not another repository build/test/cleanup command. It calls `lstat`,
uses `st_blocks * 512` for allocated bytes and `st_size` for apparent bytes (including
metadata entries), deduplicates `(device, inode)` within each root, skips other devices
and never follows symlinks. Each JSON row carries root UID/GID/device, entry count,
oldest/newest mtime, links and collection errors. All measured roots completed without
collection errors. Root ownership does not prove descendant/task ownership.

The volume is APFS `/dev/disk3s5`, mounted at `/System/Volumes/Data`; root device
`16777231`, root UID `517203268` for the measured paths. At the initial `df -k .`
observation only 1,884,748 KiB was available. Concurrent tasks consume storage: a later
`df` difference cannot be attributed to cleanup. Allocated bytes are **not exclusive
physical APFS bytes**: compression, clones, snapshots and hardlinks can change actual
recovery. Parent/child rows overlap and must not be added. A multiply-linked-file count
means distinct observed inodes with `nlink > 1`, not recoverable copies.

## Initial inventory and classification

Times below are newest mtimes in UTC, `MM-DD HH:MM`. Descendant link counts are
`symlinks / multiply-linked files`; no measured root itself was a symlink.

| Exact root (aliases above) | Allocated bytes | Apparent bytes | Links | Newest UTC | Decision |
| --- | ---: | ---: | ---: | --- | --- |
| `$REPO/.git` | 113,225,728 | 109,818,395 | 0 / 0 | 11-04T05:49 | protected |
| `$REPO/target` | 35,939,016,704 | 35,801,222,157 | 0 / 1101 | 11-04T05:07 | shared/unknown |
| `$REPO/target/debug` | 28,764,684,288 | 28,657,215,788 | 0 / 550 | 11-04T05:07 | shared/unknown |
| `$REPO/target/wasm32-unknown-unknown` | 5,861,883,904 | 5,837,791,226 | 0 / 379 | 11-04T05:07 | shared/unknown |
| `$REPO/artifacts` | 639,815,680 | 625,939,284 | 14 / 0 | 11-04T05:15 | retained-for-evidence |
| `$REPO/docs/evidence` | 11,915,264 | 10,361,536 | 0 / 0 | 11-04T03:48 | retained-for-evidence |
| `$REPO/tools/behavior/node_modules` | 42,967,040 | 28,890,825 | 4 / 0 | 11-04T03:03 | shared/unknown |
| `$REPO/apps/cantos-theatre/mobile/.gradle` | 2,703,360 | 2,667,349 | 0 / 0 | 11-04T05:10 | shared/unknown |
| `$REPO/apps/cantos-theatre/mobile/.kotlin` | 0 | 160 | 0 / 0 | 11-04T05:10 | shared/unknown |
| `$REPO/apps/cantos-theatre/mobile/shared/build` | 41,648,128 | 35,715,477 | 0 / 0 | 11-04T05:10 | shared/unknown |
| `$REPO/apps/cantos-theatre/mobile/androidApp/build` | 0 | 96 | 0 / 0 | 11-04T04:58 | shared/unknown |
| `$REPO/apps/cantos-theatre/mobile/androidApp/build-audit` | 181,575,680 | 180,356,269 | 0 / 0 | 10-30T06:26 | shared/unknown |
| `$REPO/packages/ui-system/cmp/build` | 29,126,656 | 26,014,202 | 0 / 0 | 11-04T05:08 | shared/unknown |
| `$HOME/.gradle` | 26,728,644,608 | 26,210,702,203 | 0 / 0 | 11-04T05:48 | shared/unknown |
| `$HOME/.konan` | 3,768,074,240 | 3,574,020,444 | 8 / 0 | 11-03T10:01 | shared/unknown |
| `$HOME/Library/Developer/Xcode/DerivedData` | 1,306,624 | 154,110 | 0 / 0 | 11-02T04:57 | shared/unknown |
| `$HOME/.cargo/registry` | 2,344,644,608 | 2,129,975,612 | 0 / 0 | 11-04T05:10 | shared/unknown |
| `$HOME/.cargo/git` | 3,866,624 | 2,343,841 | 0 / 2 | 09-14T02:19 | shared/unknown |
| `$HOME/.rustup/toolchains` | 1,312,276,480 | 1,305,746,580 | 0 / 0 | 11-04T02:51 | shared/unknown |
| `$HOME/.npm` | 234,606,592 | 227,481,372 | 4 / 0 | 11-04T05:48 | shared/unknown |
| `$HOME/.local/cantos-gradle` | 152,731,648 | 152,113,161 | 0 / 0 | 10-22T09:44 | shared/unknown |
| `$TASK` | 56,389,632 | 48,641,377 | 22 / 0 | 11-04T05:50 | task source/evidence |
| `$REPO/target/debug/deps` | 16,868,564,992 | 16,853,936,751 | 0 / 40 | 11-04T05:03 | shared/unknown |
| `$REPO/target/debug/incremental` | 11,543,777,280 | 11,498,568,284 | 0 / 550 | 11-04T05:03 | shared/unknown |
| `$REPO/target/protobuf-oracle` | 188,395,520 | 186,055,123 | 0 / 87 | 11-04T04:41 | shared/unknown |
| `$HOME/.gradle/caches` | 24,661,958,656 | 24,161,238,087 | 0 / 0 | 11-04T05:48 | shared/unknown |
| `$HOME/.gradle/caches/8.14.3/transforms` | 2,147,209,216 | 2,040,575,249 | 0 / 0 | 11-04T05:09 | shared/unknown |
| `$REPO/apps/cantos-theatre/mobile/preview-sandbox/build` | 18,223,104 | 16,013,468 | 0 / 0 | 11-04T05:10 | shared/unknown |
| `$REPO/packages/ui-system/cmp/.gradle` | 532,480 | 496,192 | 0 / 0 | 11-04T05:08 | shared/unknown |
| `$HOME/.codex/worktrees` | 3,482,275,840 | 3,272,648,265 | 45 / 6 | 10-29T09:06 | shared/unknown |
| `$REPO/apps/cantos-theatre/web/dist` | 144,719,872 | 144,596,229 | 0 / 0 | 11-04T05:07 | shared/unknown |
| `$REPO/apps/cantos-studio/web/dist` | 404,611,072 | 404,591,981 | 0 / 0 | 11-04T05:06 | shared/unknown |
| `$REPO/packages/ui-system/cmp/.kotlin` | 0 | 160 | 0 / 0 | 11-04T05:08 | shared/unknown |

Absent roots: workspace `target/release`, `target/qualification`, standalone
`tools/proto-codegen/target`, both `scripts/protobuf-oracle/target` and the actual
`scripts/protobuf-oracle/rust/target`, Forge Firebase and Cloudflare Wrangler
`node_modules`, visual-comparator/root `.venv`, CMP Android-preview `.gradle`/`.kotlin`,
and Theatre iOS `iosApp/DerivedData`. The protobuf gate already passes explicit
`--target-dir target/protobuf-oracle` and `target/proto-codegen`; nested defaults remain
possible when a developer invokes either isolated crate directly.

`CARGO_TARGET_DIR`, `GRADLE_USER_HOME` and `KONAN_DATA_DIR` were unset in this task's
environment. The fresh worktree initially had no compiler target; default Cargo targets
are per-worktree, while Cargo downloads and Gradle/Konan caches are host-shared. The
33-root dry-run retention list (`cleanup-plan.json`) has **zero eligible pre-existing
paths**. Some roots are repeated in the inventory to show contained costs.

## Liveness, cleanup and preservation

Initial `ps` showed Cargo/rustc and Java activity outside this task. Subsequent `lsof`
process-cwd snapshots observed another repository (`vink-pr-integration`) as well as this
task's own build; absence from one snapshot never proves exclusive ownership. Gradle
cache mtimes were current. Per-file locks and all consumers of shared targets/caches were
not established, so they are **shared/unknown**, even where bytes are rebuildable.
The 11.54 GB incremental subtree also has 550 multiply-linked files. No shared-cache,
other-worktree, toolchain, release, DB/WAL, media, secret or immutable-evidence deletion
is eligible. The same UID or an old mtime supplies no deletion authority.

Pre-existing artifacts reclaimed: **0 bytes**. Deletion command/exit code: **none**;
no eligible pre-existing deletion was attempted. This does not satisfy a claim that old
caches were cleaned. `cleanup-receipt.json` records that initial decision. Any removal
of this task's own newly generated verifier target is accounted separately in the PR
and an exact-path receipt; it must not be presented as recovery from old caches.

Before any later operator-approved removal, retain an exact-path dry-run receipt with
owner/task/ref, reproduction command, inode/device/link state, allocation and liveness.
Recheck against concurrent use immediately before effects; refuse unknown locks/owners,
external links, changed identities and protected state. Inspect incoming evidence/run
references before archiving; an age cutoff alone is insufficient. Remove only enumerated
owned disposable files, then compare source/evidence identities and actual before/after
allocation. Keep rollback and referenced receipts with their original owner. Existing
[run-record retention](../../.agents/skills/cantos-diagnostics/references/run-record.md#6-cleanup-and-retention)
and [credential custody](../deployment/google-secret-manager.md) continue to apply.

## Assets and dependency graph

Git blob identity found 30 duplicate groups, 3,692,095 bytes of additional **working-tree
copies**, not additional Git object storage. The four Noto TTF pairs account for
3,123,288 bytes; the Library AVIF pair for 464,587 bytes. Those files remain intact.

- Shared [font provenance](../../packages/ui-system/shared/assets/noto-fonts/provenance.json)
  pins Theatre CMP source bytes and OFL attribution. CMP uses `Res.font` from its own
  `composeResources`; shared desktop preview uses `syncPreviewFonts` from the shared
  asset directory, while Leptos preview copies that directory through Trunk. Removing
  a packaged copy without a generated-resource replacement changes those consumers.
- Library artwork is read by CMP `TheatreEmptyArtwork.kt` through
  `Res.readBytes("files/artwork/library_expressive.avif")` and by Theatre Web under
  `assets/empty/library-expressive.avif`, including an exact-path HTML regression.
  They are distinct packaging inputs despite identical source bytes.
- Consumer-owned DTO copies, licences, contract goldens and independent fixtures are
  retained. Blob equality supplies no authority to merge contract ownership.

Rust 1.99.1 `cargo tree --locked --offline -d` succeeded, as did Server and Identity
`--features service` feature graphs. `Cargo.lock` has 552 entries and 34 multi-version
names. For example, `base64 0.23.1` is reached through Lettre/email-encoding while
`0.22.1` remains reached through Server, Leptos and Turso consumers. `syn 3.0.7` is
reached by current proc macros alongside `syn 2`. A repeated name is not an unused
package or a measured disk saving. No dependency or lockfile was changed.

Installed tools observed: Rust 1.99.1 plus retained 1.96, Gradle 8.14.3,
Node 26.12.0 and Xcode 26.6. The shell's Python 3.9 lacks `tomllib`; tests use the already
installed Homebrew Python 3.14 by putting `/opt/homebrew/bin` first on `PATH`. No tool
was installed. Resolved Gradle dependency graphs are **NOT_RUN**: low free space and
shared active cache ownership make a new Gradle configuration/cache write undesirable.
Gradle manifest inspection is not resolved graph or APK acceptance; #245 owns that work.

## Changes and three follow-up choices

1. **Adopted: ignore every Cargo `target/` and derive CircleCI cache namespaces from
   `rust-toolchain.toml` checksum.** No tracked path contains a `target` directory.
   The ignore regression checks isolated generator/oracle outputs and confirms their
   manifests, lockfile and source remain visible. Cache regressions check exact
   restore/save pairing, architecture/pin separation and source-download-only scopes.
   The v3 namespace causes a future cold cache, without changing compiler selection,
   build commands, cached paths or promotion/rollback policy. Reverting the small config
   change restores the former lookup; no remote cache was deleted. Disk savings are
   unmeasured; ignore prevents accidental binary inclusion, not compiler-output growth.
2. **Defer: owner-approved incremental/Gradle cleanup after current runs finish.**
   Measured candidates are 11,543,777,280 allocated bytes of Rust incremental and
   2,147,209,216 bytes of Gradle 8.14.3 transforms (both contained in larger rows).
   They are upper allocation observations, not promised exclusive recovery. Rebuild
   time, network need and APFS recovery are unmeasured. Next owner: the current build
   tasks and machine operator, with fresh process/lock and exact-path approval.
3. **Defer: generated canonical asset inputs.** Potential working-tree reduction is
   3,123,288 font bytes plus 464,587 artwork bytes; current Git storage savings are
   unknown and packaged app savings may be zero. Cost: shared UI/CMP/Web owners must
   preserve resource names, OFL/provenance, Vietnamese glyphs, independent consumers
   and Android/iOS/Web packaging, with clean-build parity before deleting copies.
   Do not add a generator merely to save 3.59 MB before reclaiming approved build output.

CI audit: build cache currently stores Cargo registry/git downloads, not `target`;
Trunk cache stores only its executable. Gradle installation and Android release-audit
keys remain scoped as before. GitHub upload retention remains 14/30 days; CircleCI
workspace/account retention is still operator-owned in the
[release runbook](../deployment/circleci-builds.md#operator-prerequisites-and-retention).
Historical CI/provider lanes remain: #280 cutover/rollback acceptance is separate,
as are #386's legacy-doc cleanup and #245's APK/runtime work. No new gate, installer,
cleanup utility, workflow or runtime dependency is introduced.

## Reproduction and evidence limits

Local evidence filenames are in the ignored run directory above. Reproduce the source
graph without compilation with `RUSTUP_TOOLCHAIN=1.99.1 cargo tree --locked --offline -d`,
and select `-p cantos-server -e features` or
`-p cantos-identity --features service -e features`. Read-only filesystem primitives were
`git worktree list --porcelain`, `git count-objects -v`, `df -k .`, `mount`, `ps`, `lsof`
and the `lstat` traversal described above. No historic measurement was reused as a
current measurement. Git reported 104,369 KiB packed objects and 2,140 KiB loose objects;
its physical `.git` row includes administrative metadata and is a different metric.

New tests first failed on the fetched base, then the three affected Python suites passed
(29 tests). The first attempt used Python 3.9 and also failed to import `tomllib`; it is
retained as an environment failure, not a code regression. Exact final preflight command,
independent gate statuses, checked diff identity and log hashes belong in the PR. Broad
runtime rebuilds are not storage-audit evidence and are deferred under the low-disk
boundary; no browser/device, provider, production, audio or release acceptance follows.

Behavior SSOT impact: catalog and closest `cantos.tooling.hosting-deploy` draft reviewed;
no catalog entry owns local ignore/cache lookup policy. Product states, authorization,
retry/recovery and deploy outcomes are unchanged; no model delta. The preflight
`impact:unknown` from CI configuration must receive an explicit scoped decision.

Raw observation identity (SHA-256; local files, not tracked attachments):

```text
23007f391e2a6be2dfe47d41a868df7a9657832700f15f0bc40ac83afd1e8344  inventory-before.jsonl
2b8099fc8ed9ced3b47e1af730a14a78e35e6868d4dcbfed11339a2785ff2c8d  inventory-detail.jsonl
482ce410a21e4a6b0903ab24f43aa9cecf141bc6a2eb924978d58bf8c9c7fb58  inventory-extra.jsonl
c35d58390d6c3df58aad9b3a0125d7ddaab149b2bb9834d840d621ee1a687546  inventory-other-generated.jsonl
95ba00f1b7925de1c924fe853c471e5dd7bb8f3305cf02b206275dce70a6193c  cleanup-plan.json
01da3ab77782efe562872e5860478428d875089b9d3b8a83e6c57c4c2746eba4  duplicate-assets.json
```
