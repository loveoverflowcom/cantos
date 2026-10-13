# Cantos Forge

Producer-side CLI for Cantos content production, proposed in
[ADR 0022](../../docs/decisions/0022-cantos-forge-producer-cli.md) from the
[Forge vision issue](https://github.com/loveoverflowcom/cantos/issues/82). Forge runs in the
producer's environment, **outside the Cantos API runtime**: it will manage local drafts,
artifact revisions, evaluation evidence and handoff bundles for AI hosts and human producers.
The API, Studio and Theatre never execute generation; Forge never publishes.

Cantos Forge is unrelated to the external *Narrative Forge* project. The rule that Cantos code
never imports Narrative Forge internals applies to Forge too.

## Status

| Part | State |
| --- | --- |
| Crate, workspace membership, `cargo xtask build cantos-forge` | implemented |
| Pure domain core (below) | implemented, example- and property-tested; no command uses it yet |
| `cantos-forge capabilities [--json]` | implemented: reports exactly the commands this build has |
| Workspace, artifact registration, instruction bundles, evaluation, handoff export | proposed (next foundation slice of #82) |
| Provider/tool adapters, budget and attempt recovery | proposed (#82 PR 2); no provider is configured or called |
| Authorized handoff to the Cantos API | proposed (#82 PR 3); no HTTP client exists |
| MCP or any daemon/server mode | not planned until a host needs it |

Nothing here creates a server identity, a Production Brief, a submission, an admission or a
release. `capabilities` reports both workflows and the handoff as `unsupported`.

## Layout

```text
src/
  main.rs            imperative shell: writes the CLI response and maps the exit status
  lib.rs
  domain/            pure core; no I/O, clock, randomness, process, CLI or serde
    identity.rs      LocalId<K>: ArtifactId, EvidenceId; kinds cannot be mixed
    digest.rs        Sha256Digest of exact bytes
    workspace_path.rs WorkspacePath: lexically cannot leave the workspace root
    label.rs         bounded labels (producer, claim reference, check, model, reviewer)
    artifact.rs      immutable ArtifactRevision, type-specific payloads, provenance,
                     rights claims, input pins and the idempotent registration decision
    lineage.rs       RevisionHistory and pin freshness: current, stale or violation
    evaluation.rs    evidence and three independent layers: deterministic, AI, human
  interfaces/
    cli.rs           clap parsing, the JSON envelope and exit statuses; performs no I/O
tests/
  architecture.rs    the domain imports no I/O, framework or outer layer (lexical check)
  cli.rs             the built binary's exit codes and stdout/stderr split
```

`application/` (use cases and ports) and `adapters/` (filesystem, API client, provider
processes) are added with their first consumer, not as empty modules. The crate depends only on
pinned workspace dependencies (`clap`, `serde`, `serde_json`, `sha2`; `proptest` for tests).
It never depends on `services/` or another app; `cargo test -p xtask` checks every workspace
dependency edge points inward.

## Core invariants

| Invariant | Barrier | Evidence |
| --- | --- | --- |
| an artifact identity is never used as an evidence identity | `LocalId<K>` phantom kind and text prefix | type-enforced (compile-fail doctest), example-tested |
| a digest names exact bytes; NFC and NFD text differ | `Sha256Digest` with one canonical form | example-tested (NIST vectors), property-tested round trip |
| a workspace path cannot name anything outside the root | `WorkspacePath::parse` | example-tested per refusal, property-tested over generated paths |
| a revision is written once; a change is a successor; a replay creates nothing | private fields, `decide_registration` | example-tested |
| kind and manuscript origin never change across revisions; a revision never pins itself | `RevisionError` variants | example-tested |
| a new input revision marks dependants stale; unrelated revisions do not | `lineage::assess` | property-tested |
| a pin contradicting history fails closed with every violation | `PinViolation` | example- and property-tested |
| evaluation layers are independent and stay `NotRun` without current evidence | `evaluation::summarize` | example-tested |

These are local, lexical and in-memory guarantees. Symlinks, hard links and case- or
Unicode-normalization-insensitive file systems, concurrent writers and persisted history are
the future filesystem adapter's acceptance, not claims of this crate.

## Command-line contract

Non-interactive. Results go to stdout, diagnostics to stderr. With `--json`, stdout carries one
envelope per invocation:

```json
{"schema":"cantos-forge.cli/v1","command":"capabilities","outcome":"ok","data":{"forge_version":"0.1.0","commands":["capabilities"],"workflows":{"host_assisted":"unsupported","provider_managed":"unsupported"},"providers":[],"handoff":"unsupported"}}
```

A usage error with `--json` in the arguments is
`{"schema":…,"command":null,"outcome":"usage_error","error":{"code":"usage","message":…}}`.

| Exit | Meaning |
| --- | --- |
| 0 | success, including `--help` and `--version` |
| 1 | the command failed or its result could not be written |
| 2 | usage error; nothing ran |

A typed "blocked / action required" outcome (missing workspace, credential or permission) is
proposed for the first command that can be blocked; it will not prompt.

## Build and verify

```sh
cargo xtask build cantos-forge          # cargo build --locked -p cantos-forge --bins
cargo xtask verify rust                 # workspace fmt, clippy and tests, including Forge
cargo test -p cantos-forge --locked     # Forge only
```

xtask builds Forge but never runs it inside build, test, preflight or CI, and Forge never
starts xtask. Running the binary directly (`target/debug/cantos-forge capabilities --json`) is
local use, not a repository check.
